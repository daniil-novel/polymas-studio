use polymas_core::secrets::OsVault;
use polymas_desktop_host::{
    BridgeError,
    preferences::{CloseBehavior, Preferences},
    profile::NativeProfile,
};
use serde::Serialize;
use serde_json::Value;
use std::{path::PathBuf, sync::Arc};
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    phase: &'static str,
    error: Option<String>,
    close_behavior: Option<CloseBehavior>,
    close_requested: bool,
    warnings: Vec<String>,
}
struct Inner {
    status: Status,
    profile: Option<Arc<NativeProfile>>,
}
struct DesktopState {
    profile_dir: PathBuf,
    workspace: PathBuf,
    executable: PathBuf,
    preferences: Preferences,
    inner: Mutex<Inner>,
    lifecycle: Mutex<()>,
}
fn bridge_error(code: &str, message: impl Into<String>, status: u16) -> BridgeError {
    BridgeError {
        code: code.into(),
        message: message.into(),
        status,
    }
}
fn local_error(error: polymas_core::error::Error) -> BridgeError {
    bridge_error(
        "desktop",
        polymas_core::process::redact(&error.to_string()),
        503,
    )
}
fn verify_no_service(workspace: &std::path::Path) -> Result<(), BridgeError> {
    let lock = match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(workspace.join("workspace.lock"))
    {
        Ok(lock) => lock,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => {
            return Err(bridge_error(
                "disconnected",
                "Не удалось проверить состояние службы",
                503,
            ));
        }
    };
    fs2::FileExt::try_lock_exclusive(&lock).map_err(|_| {
        bridge_error(
            "disconnected",
            "Рабочая область занята службой. Восстановите подключение перед остановкой.",
            503,
        )
    })
}
async fn publish(app: &tauri::AppHandle) {
    let state = app.state::<DesktopState>();
    let status = state.inner.lock().await.status.clone();
    let _ = app.emit_to("main", "desktop-status-changed", status);
}
#[tauri::command]
async fn api_request(
    state: tauri::State<'_, DesktopState>,
    path: String,
    body: Option<Value>,
) -> Result<Value, BridgeError> {
    let profile = {
        let inner = state.inner.lock().await;
        if inner.status.phase != "ready" {
            return Err(bridge_error(
                "not_ready",
                inner
                    .status
                    .error
                    .clone()
                    .unwrap_or_else(|| "Служба подключается или останавливается".into()),
                503,
            ));
        }
        inner
            .profile
            .clone()
            .ok_or_else(|| bridge_error("not_ready", "Служба ещё не подключена", 503))?
    };
    profile.connection.request(&path, body).await
}
#[tauri::command]
async fn desktop_status(state: tauri::State<'_, DesktopState>) -> Result<Status, BridgeError> {
    Ok(state.inner.lock().await.status.clone())
}
async fn connect(app: &tauri::AppHandle) -> Result<(), BridgeError> {
    let state = app.state::<DesktopState>();
    let _guard = state
        .lifecycle
        .try_lock()
        .map_err(|_| bridge_error("conflict", "Другое действие службы ещё выполняется", 409))?;
    {
        let mut inner = state.inner.lock().await;
        inner.status.phase = "starting";
        inner.status.error = None;
    }
    publish(app).await;
    let result = match OsVault::new("desktop") {
        Ok(vault) => {
            NativeProfile::connect(
                &state.profile_dir,
                &state.workspace,
                &state.executable,
                1,
                &vault,
            )
            .await
        }
        Err(error) => Err(error),
    };
    let mut inner = state.inner.lock().await;
    let outcome = match result {
        Ok(profile) => {
            inner.status.phase = "ready";
            inner.status.warnings = profile.warnings.clone();
            inner.profile = Some(Arc::new(profile));
            Ok(())
        }
        Err(error) => {
            let error = local_error(error);
            inner.status.phase = "failed";
            inner.status.error = Some(error.message.clone());
            Err(error)
        }
    };
    drop(inner);
    publish(app).await;
    outcome
}
#[tauri::command]
async fn desktop_reconnect(app: tauri::AppHandle) -> Result<(), BridgeError> {
    connect(&app).await
}

#[tauri::command]
async fn desktop_cancel_close(app: tauri::AppHandle) {
    app.state::<DesktopState>()
        .inner
        .lock()
        .await
        .status
        .close_requested = false;
    publish(&app).await;
}
#[tauri::command]
async fn desktop_reset_close(app: tauri::AppHandle) -> Result<(), BridgeError> {
    let state = app.state::<DesktopState>();
    state
        .preferences
        .set_close_behavior(None)
        .map_err(local_error)?;
    state.inner.lock().await.status.close_behavior = None;
    publish(&app).await;
    Ok(())
}
#[tauri::command]
async fn desktop_close(
    app: tauri::AppHandle,
    behavior: CloseBehavior,
    remember: bool,
) -> Result<(), BridgeError> {
    let state = app.state::<DesktopState>();
    let _guard = state.lifecycle.try_lock().map_err(|_| {
        bridge_error(
            "conflict",
            "Дождитесь завершения подключения или остановки",
            409,
        )
    })?;
    if remember {
        state
            .preferences
            .set_close_behavior(Some(behavior))
            .map_err(local_error)?;
        state.inner.lock().await.status.close_behavior = Some(behavior);
    }
    if behavior == CloseBehavior::Background {
        if let Some(window) = app.get_webview_window("main") {
            window
                .hide()
                .map_err(|_| bridge_error("window", "Не удалось скрыть окно", 500))?;
        }
        state.inner.lock().await.status.close_requested = false;
        publish(&app).await;
        return Ok(());
    }
    let profile = {
        let mut inner = state.inner.lock().await;
        inner.status.phase = "stopping";
        inner.status.error = None;
        inner.profile.clone()
    };
    publish(&app).await;
    let outcome = match profile {
        Some(profile) => profile.stop().await.map_err(local_error),
        None => verify_no_service(&state.workspace),
    };
    if let Err(error) = outcome {
        let mut inner = state.inner.lock().await;
        inner.status.phase = "failed";
        inner.status.error = Some(error.message.clone());
        inner.status.close_requested = true;
        drop(inner);
        show(&app);
        publish(&app).await;
        return Err(error);
    }
    app.exit(0);
    Ok(())
}
fn show(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
async fn request_close(app: tauri::AppHandle) {
    let state = app.state::<DesktopState>();
    let choice = {
        let inner = state.inner.lock().await;
        if matches!(inner.status.phase, "starting" | "stopping") {
            None
        } else {
            inner.status.close_behavior
        }
    };
    if let Some(behavior) = choice
        && desktop_close(app.clone(), behavior, false).await.is_ok()
    {
        return;
    }
    state.inner.lock().await.status.close_requested = true;
    show(&app);
    let _ = app.emit_to("main", "desktop-close-requested", ());
    publish(&app).await;
}
pub fn run() -> tauri::Result<()> {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show(app)))
        .invoke_handler(tauri::generate_handler![
            api_request,
            desktop_status,
            desktop_reconnect,
            desktop_close,
            desktop_cancel_close,
            desktop_reset_close
        ])
        .setup(|app| {
            let profile_dir = app.path().app_local_data_dir()?;
            let preferences = Preferences::open(&profile_dir.join("desktop.sqlite3"))?;
            let close_behavior = preferences.close_behavior()?;
            let executable = std::env::current_exe()?
                .parent()
                .ok_or("Executable has no parent")?
                .join("polymas-daemon.exe");
            app.manage(DesktopState {
                workspace: profile_dir.join("workspaces/default"),
                profile_dir,
                executable,
                preferences,
                inner: Mutex::new(Inner {
                    status: Status {
                        phase: "starting",
                        error: None,
                        close_behavior,
                        close_requested: false,
                        warnings: vec![],
                    },
                    profile: None,
                }),
                lifecycle: Mutex::new(()),
            });
            let window = tauri::WebviewWindowBuilder::from_config(
                app.handle(),
                &app.config().app.windows[0],
            )?
            .on_navigation(|url| crate::allowed_navigation(url, cfg!(debug_assertions)))
            .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
            .build()?;
            let handle = app.handle().clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    tauri::async_runtime::spawn(request_close(handle.clone()));
                }
            });
            use tauri::{
                menu::{Menu, MenuItem},
                tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
            };
            let open =
                MenuItem::with_id(app, "open", "Открыть PolyMAS Studio", true, None::<&str>)?;
            let close =
                MenuItem::with_id(app, "stop", "Остановить задачи и выйти", true, None::<&str>)?;
            let reset =
                MenuItem::with_id(app, "reset", "Спрашивать при закрытии", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &reset, &close])?;
            let icon = app.default_window_icon().ok_or("Missing app icon")?.clone();
            TrayIconBuilder::with_id("polymas")
                .icon(icon)
                .tooltip("PolyMAS Studio — редактор и управление службой")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        }
                    ) {
                        show(tray.app_handle());
                    }
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show(app),
                    "reset" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = desktop_reset_close(app).await;
                        });
                    }
                    "stop" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            if desktop_close(app.clone(), CloseBehavior::Stop, false)
                                .await
                                .is_err()
                            {
                                show(&app);
                            }
                        });
                    }
                    _ => {}
                })
                .build(app)?;
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let _ = connect(&handle).await;
            });
            Ok(())
        })
        .run(tauri::generate_context!())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_unconnected_window_cannot_claim_a_live_workspace_is_stopped() {
        let directory = tempfile::tempdir().unwrap();
        let workspace = polymas_core::workspace::Workspace::open(directory.path()).unwrap();
        assert!(super::verify_no_service(directory.path()).is_err());
        drop(workspace);
        assert!(super::verify_no_service(directory.path()).is_ok());
        assert!(super::verify_no_service(&directory.path().join("never-started")).is_ok());
    }
    #[test]
    fn ipc_respects_window_origin_and_preserves_not_ready_errors() {
        let directory = tempfile::tempdir().unwrap();
        let app = tauri::test::mock_builder()
            .manage(DesktopState {
                profile_dir: directory.path().into(),
                workspace: directory.path().join("workspace"),
                executable: directory.path().join("absent.exe"),
                preferences: Preferences::open(&directory.path().join("desktop.sqlite3")).unwrap(),
                inner: Mutex::new(Inner {
                    status: Status {
                        phase: "starting",
                        error: None,
                        close_behavior: None,
                        close_requested: false,
                        warnings: vec![],
                    },
                    profile: None,
                }),
                lifecycle: Mutex::new(()),
            })
            .invoke_handler(tauri::generate_handler![api_request, desktop_status])
            .build(tauri::generate_context!())
            .unwrap();
        let main = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        let other = tauri::WebviewWindowBuilder::new(&app, "untrusted", Default::default())
            .build()
            .unwrap();
        let request = |command: &str, origin: &str, body: Value| tauri::webview::InvokeRequest {
            cmd: command.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: origin.parse().unwrap(),
            body: tauri::ipc::InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.to_string(),
        };
        let local = "http://tauri.localhost";
        let status = tauri::test::get_ipc_response(
            &main,
            request("desktop_status", local, serde_json::json!({})),
        )
        .unwrap()
        .deserialize::<Value>()
        .unwrap();
        assert_eq!(status["phase"], "starting");
        let error = tauri::test::get_ipc_response(
            &main,
            request(
                "api_request",
                local,
                serde_json::json!({"path":"/api/control","body":{"action":"pause"}}),
            ),
        )
        .unwrap_err();
        assert_eq!(error["code"], "not_ready");
        assert_eq!(error["status"], 503);
        assert!(
            tauri::test::get_ipc_response(
                &other,
                request("desktop_status", local, serde_json::json!({}))
            )
            .is_err()
        );
        assert!(
            tauri::test::get_ipc_response(
                &main,
                request(
                    "desktop_status",
                    "https://example.com",
                    serde_json::json!({})
                )
            )
            .is_err()
        );
    }
}
