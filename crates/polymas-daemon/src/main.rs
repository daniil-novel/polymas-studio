use polymas_core::{
    engine::Engine,
    error::{Error, Result},
};
use polymas_daemon::{AppState, router, run_api, runtime};
use std::{io::Write, path::PathBuf, time::Duration};

#[tokio::main]
async fn main() {
    if let Err(error) = serve().await {
        eprintln!("{}", polymas_core::process::redact(&error.to_string()));
        std::process::exit(1);
    }
}
async fn serve() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--gateway-port") {
        polymas_core::error::require(args.len() == 2, "Gateway принимает только локальный порт")?;
        let port = args[1]
            .parse()
            .map_err(|_| Error::Validation("Некорректный gateway port".into()))?;
        return polymas_daemon::bridge::serve(port).await;
    }
    if args.iter().any(|a| a == "--help") {
        println!(
            "PolyMAS service\n--workspace ABSOLUTE_PATH --port PORT [--slots N] [--codex ABSOLUTE_EXE] [--dev]\nPOLYMAS_CONTROL_TOKEN is supplied by the desktop launcher. Listens on 127.0.0.1 only."
        );
        return Ok(());
    }
    let mut root = None;
    let mut port = 8787_u16;
    let mut capacity = 1_usize;
    let mut codex = None;
    let mut dev = false;
    let mut adoption_timeout = None;
    let mut index = 0;
    while index < args.len() {
        let key = &args[index];
        if key == "--dev" {
            dev = true;
            index += 1;
            continue;
        }
        let value = args
            .get(index + 1)
            .ok_or_else(|| Error::Validation(format!("Нет значения {key}")))?;
        match key.as_str() {
            "--workspace" => root = Some(PathBuf::from(value)),
            "--port" => {
                port = value
                    .parse()
                    .map_err(|_| Error::Validation("Некорректный порт".into()))?
            }
            "--slots" => {
                capacity = value
                    .parse()
                    .map_err(|_| Error::Validation("Некорректная capacity".into()))?
            }
            "--adoption-timeout" => {
                let seconds = value
                    .parse::<u64>()
                    .map_err(|_| Error::Validation("Некорректный срок adoption".into()))?;
                polymas_core::error::require(
                    (1..=60).contains(&seconds),
                    "Adoption timeout: 1–60 секунд",
                )?;
                adoption_timeout = Some(seconds);
            }
            "--codex" => codex = Some(PathBuf::from(value)),
            _ => return Err(Error::Validation(format!("Неизвестный параметр {key}"))),
        }
        index += 2;
    }
    let root = root.ok_or_else(|| Error::Validation("Укажите --workspace".into()))?;
    polymas_core::error::require(
        root.is_absolute() && (1..=5000).contains(&capacity),
        "Нужны абсолютный workspace и capacity 1–5000",
    )?;
    let token = std::env::var("POLYMAS_CONTROL_TOKEN").map_err(|_| {
        Error::Denied("Запустите службу через PolyMAS Desktop или локальный dev launcher".into())
    })?;
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    let actual_port = listener.local_addr()?.port();
    let mut origins = vec![
        format!("http://127.0.0.1:{actual_port}"),
        "tauri://localhost".into(),
        "http://tauri.localhost".into(),
    ];
    if dev {
        origins.push("http://127.0.0.1:1420".into());
    }
    let state = AppState::new(Engine::open(&root)?, token, origins)?;
    if let Some(seconds) = adoption_timeout {
        state.require_adoption();
        let pending = state.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(seconds)).await;
            if pending.expire_adoption() {
                pending.shutdown.notify_one();
            }
        });
    }
    state
        .configure_gateway(actual_port, std::env::current_exe()?)
        .await?;
    println!(
        "{}",
        serde_json::json!({"type":"ready","port":actual_port,"workspaceId":state.engine.lock().await.workspace.config().value.id})
    );
    std::io::stdout().flush()?;
    let background = state.clone();
    let worker = tokio::spawn(async move {
        let _ = run_api::configure_runtime(&background, codex, capacity).await;
        let mut turns = tokio::task::JoinSet::new();
        let mut interval = tokio::time::interval(Duration::from_millis(250));
        loop {
            tokio::select! {
                _=interval.tick()=>{
                    run_api::signal_stopping(&background).await;
                    let options=background.runtime_options.read().await.clone();
                    if let Some(options)=options {
                        match runtime::drive_once(&background,&options).await {
                            Ok(handles)=>for handle in handles {turns.spawn(handle);},
                            Err(error)=>{background.runtime_status.write().await.error=Some(error.to_string());}
                        }
                    }
                }
                Some(result)=turns.join_next(),if !turns.is_empty()=>{
                    let error=match result {
                        Ok(Ok(Ok(())))=>None,
                        Ok(Ok(Err(error)))=>Some(error.to_string()),
                        Ok(Err(error))|Err(error)=>Some(format!("Supervisor interrupted: {error}")),
                    };
                    if let Some(error)=error {background.runtime_status.write().await.error=Some(polymas_core::process::redact(&error));}
                }
            }
        }
    });
    let shutdown_state = state.clone();
    let server = axum::serve(listener, router(state.clone())).with_graceful_shutdown(async move {
        tokio::select! {_=tokio::signal::ctrl_c()=>{},_=shutdown_state.shutdown.notified()=>{}}
        if shutdown_state.is_adopted() {
            let _ = run_api::stop_all(&shutdown_state).await;
        }
        for _ in 0..100 {
            if shutdown_state.active.lock().await.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
    let result = server.await;
    worker.abort();
    let _ = worker.await;
    result?;
    Ok(())
}
