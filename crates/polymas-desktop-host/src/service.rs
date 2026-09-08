use crate::NativeConnection;
use polymas_core::{
    error::{Error, Result, require},
    process::{ProcessSpec, command_for},
    secrets::Secret,
};
use serde::Deserialize;
use serde_json::json;
use std::{path::Path, process::Stdio, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    process::Child,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Ready {
    r#type: String,
    port: u16,
    workspace_id: String,
}
/// The service is separate from the window. Drop deliberately detaches; only
/// explicit shutdown requests stop work. Failed startup always reaps its child.
pub struct RunningService {
    connection: Arc<NativeConnection>,
    child: Child,
    pub workspace_id: String,
}
impl RunningService {
    pub fn connection(&self) -> Arc<NativeConnection> {
        self.connection.clone()
    }
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }
    pub async fn launch(executable: &Path, workspace: &Path, capacity: usize) -> Result<Self> {
        require(
            executable.is_absolute()
                && executable.is_file()
                && workspace.is_absolute()
                && (1..=5000).contains(&capacity),
            "Нужны доверенный executable, абсолютный workspace и capacity 1–5000",
        )?;
        let token = Secret::new(format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        ))?;
        let spec = ProcessSpec {
            executable: executable.into(),
            args: vec![
                "--workspace".into(),
                workspace.to_string_lossy().into_owned(),
                "--port".into(),
                "0".into(),
                "--slots".into(),
                capacity.to_string(),
            ],
            workspace: executable
                .parent()
                .ok_or_else(|| Error::Validation("Нет папки executable".into()))?
                .into(),
            stdin: String::new(),
            timeout_seconds: 20,
            environment: [("POLYMAS_CONTROL_TOKEN".into(), token.expose().to_owned())].into(),
        };
        let mut command = command_for(&spec);
        command.stdin(Stdio::null()).kill_on_drop(false);
        let mut child = command.spawn()?;
        let mut stdout = BufReader::new(
            child
                .stdout
                .take()
                .ok_or_else(|| Error::Runtime("Нет канала готовности службы".into()))?
                .take(4097),
        );
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| Error::Runtime("Нет канала диагностики службы".into()))?;
        let diagnostics = Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
        let tail = diagnostics.clone();
        tokio::spawn(async move {
            let mut buffer = [0_u8; 2048];
            while let Ok(n) = stderr.read(&mut buffer).await {
                if n == 0 {
                    break;
                }
                if let Ok(mut bytes) = tail.lock() {
                    bytes.extend_from_slice(&buffer[..n]);
                    if bytes.len() > 16384 {
                        let excess = bytes.len() - 16384;
                        bytes.drain(..excess);
                    }
                }
            }
        });
        let mut line = String::new();
        let ready =
            tokio::time::timeout(Duration::from_secs(20), stdout.read_line(&mut line)).await;
        let parsed: Option<Ready> = match ready {
            Ok(Ok(n)) if n > 0 && n <= 4096 => serde_json::from_str(&line).ok(),
            _ => None,
        };
        let Some(ready) =
            parsed.filter(|r| r.r#type == "ready" && r.port > 0 && !r.workspace_id.is_empty())
        else {
            child.kill().await.ok();
            child.wait().await.ok();
            let message = diagnostics
                .lock()
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .unwrap_or_default();
            return Err(Error::Runtime(format!(
                "Служба не запустилась: {}",
                polymas_core::process::redact(&message.replace(token.expose(), "[REDACTED]"))
            )));
        };
        let connection = match NativeConnection::new(ready.port, token) {
            Ok(connection) => Arc::new(connection),
            Err(error) => {
                child.kill().await.ok();
                child.wait().await.ok();
                return Err(error);
            }
        };
        Ok(Self {
            connection,
            child,
            workspace_id: ready.workspace_id,
        })
    }
    pub async fn shutdown(mut self) -> Result<()> {
        self.connection
            .request("/api/control", Some(json!({"action":"shutdown"})))
            .await
            .map_err(|e| Error::Runtime(e.message))?;
        match tokio::time::timeout(Duration::from_secs(15), self.child.wait()).await {
            Ok(result) => {
                result?;
                Ok(())
            }
            Err(_) => {
                // This handle was created by this launcher, never recovered from a PID file.
                self.child.kill().await?;
                self.child.wait().await?;
                Err(Error::Timeout("Служба не завершилась после drain; процесс остановлен, исход активных задач требует сверки".into()))
            }
        }
    }
}
