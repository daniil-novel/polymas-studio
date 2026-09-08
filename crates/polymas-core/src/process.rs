use crate::codex::JsonlDecoder;
use crate::error::{Error, Result, require};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::{mpsc, watch};

pub struct ProcessSpec {
    pub executable: PathBuf,
    pub args: Vec<String>,
    pub workspace: PathBuf,
    pub stdin: String,
    pub timeout_seconds: u64,
    pub environment: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessEvent {
    pub kind: String,
    pub body: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessOutcome {
    pub pid: u32,
    pub success: bool,
    pub cancelled: bool,
    pub timed_out: bool,
    pub exit_code: Option<i32>,
    pub stderr_tail: String,
}

pub fn redact(text: &str) -> String {
    static PATTERNS: OnceLock<Vec<regex::Regex>> = OnceLock::new();
    let patterns=PATTERNS.get_or_init(||[
        r"(?i)Bearer\s+[A-Za-z0-9._~+/=-]+",
        r"\b(?:sk-[A-Za-z0-9_-]{12,}|gh[pousr]_[A-Za-z0-9_]{12,}|github_pat_[A-Za-z0-9_]{12,})",
        r#"(?i)(?:api[_-]?key|access[_-]?token|refresh[_-]?token|password)\s*[=:]\s*[\"']?[^\s\"',}]+"#,
    ].iter().map(|p|regex::Regex::new(p).expect("static redaction pattern")).collect());
    patterns.iter().fold(text.to_string(), |value, pattern| {
        pattern.replace_all(&value, "[REDACTED]").into_owned()
    })
}

pub fn command_for(spec: &ProcessSpec) -> Command {
    let mut command = Command::new(&spec.executable);
    command
        .args(&spec.args)
        .current_dir(&spec.workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .env_clear();
    for key in [
        "PATH",
        "PATHEXT",
        "SystemRoot",
        "SYSTEMROOT",
        "WINDIR",
        "COMSPEC",
        "USERPROFILE",
        "HOME",
        "HOMEDRIVE",
        "HOMEPATH",
        "APPDATA",
        "LOCALAPPDATA",
        "PROGRAMDATA",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "TEMP",
        "TMP",
        "TMPDIR",
        "LANG",
        "LC_ALL",
        "CODEX_HOME",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "NO_PROXY",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command.envs(&spec.environment);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    #[cfg(unix)]
    command.process_group(0);
    command
}

async fn emit(sender: &mpsc::Sender<ProcessEvent>, kind: &str, body: Value) -> Result<()> {
    tokio::time::timeout(
        Duration::from_secs(5),
        sender.send(ProcessEvent {
            kind: kind.into(),
            body,
        }),
    )
    .await
    .map_err(|_| {
        Error::Runtime("Журнал не успевает сохранять события; попытка остановлена".into())
    })?
    .map_err(|_| Error::Runtime("Потеряна связь с журналом событий".into()))
}

pub async fn run_process(
    spec: ProcessSpec,
    sender: mpsc::Sender<ProcessEvent>,
    mut cancel: watch::Receiver<bool>,
) -> Result<ProcessOutcome> {
    require(!*cancel.borrow(), "Запуск уже отменён")?;
    require(
        spec.workspace.is_dir() && spec.workspace.is_absolute(),
        "Рабочая папка процесса отсутствует",
    )?;
    require(
        (1..=86_400).contains(&spec.timeout_seconds),
        "Некорректный таймаут процесса",
    )?;
    let mut child = command_for(&spec).spawn()?;
    let pid = child
        .id()
        .ok_or_else(|| Error::Runtime("Процесс не получил PID".into()))?;
    let containment = ProcessContainment::attach(&child, pid)?;
    emit(&sender, "runtime.started", json!({"pid":pid})).await?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| Error::Runtime("stdin недоступен".into()))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::Runtime("stdout недоступен".into()))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| Error::Runtime("stderr недоступен".into()))?;
    let input = spec.stdin.into_bytes();
    let input_task = tokio::spawn(async move {
        stdin.write_all(&input).await?;
        stdin.shutdown().await
    });
    let mut decoder = JsonlDecoder::default();
    let mut out = [0; 8192];
    let mut err = [0; 4096];
    let mut tail = Vec::<u8>::new();
    let mut stdout_done = false;
    let mut stderr_done = false;
    let mut cancelled = false;
    let mut timed_out = false;
    let mut exit_status = None;
    let deadline = tokio::time::sleep(Duration::from_secs(spec.timeout_seconds));
    tokio::pin!(deadline);
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
    heartbeat.tick().await;
    let result:Result<()>=async {
        loop {
            if stdout_done && stderr_done && exit_status.is_some() {break;}
            tokio::select! {
                biased;
                _=cancel.changed(), if !cancelled => {
                    if *cancel.borrow() {cancelled=true;containment.terminate();child.kill().await.ok();break;}
                    if cancel.has_changed().is_err() {cancelled=true;containment.terminate();child.kill().await.ok();break;}
                },
                _=&mut deadline=>{timed_out=true;containment.terminate();child.kill().await.ok();break;},
                status=child.wait(), if exit_status.is_none()=>{
                    exit_status=Some(status?);
                    containment.terminate();
                },
                read=stdout.read(&mut out),if !stdout_done=>{
                    let size=read?;
                    let events=if size==0 {stdout_done=true;decoder.finish()?} else {decoder.push(&out[..size])?};
                    for event in events {emit(&sender,"runtime.event",event).await?;}
                },
                read=stderr.read(&mut err),if !stderr_done=>{
                    let size=read?;if size==0 {stderr_done=true;} else {
                        tail.extend_from_slice(&err[..size]);if tail.len()>16_384 {tail.drain(..tail.len()-16_384);}
                    }
                },
                _=heartbeat.tick()=>{emit(&sender,"runtime.heartbeat",json!({"pid":pid})).await?;}
            }
        }Ok(())
    }.await;
    if result.is_err() {
        containment.terminate();
        child.kill().await.ok();
    }
    input_task.abort();
    let status = match exit_status {
        Some(status) => status,
        None => tokio::time::timeout(Duration::from_secs(5), child.wait())
            .await
            .map_err(|_| Error::Runtime("Не удалось подтвердить завершение процесса".into()))??,
    };
    result?;
    Ok(ProcessOutcome {
        pid,
        success: status.success() && !cancelled && !timed_out,
        cancelled,
        timed_out,
        exit_code: status.code(),
        stderr_tail: redact(&String::from_utf8_lossy(&tail)),
    })
}

#[cfg(windows)]
pub struct ProcessContainment {
    handle: windows_sys::Win32::Foundation::HANDLE,
}
#[cfg(windows)]
unsafe impl Send for ProcessContainment {}
#[cfg(windows)]
unsafe impl Sync for ProcessContainment {}
#[cfg(windows)]
impl ProcessContainment {
    pub fn attach(child: &tokio::process::Child, _pid: u32) -> Result<Self> {
        use windows_sys::Win32::System::JobObjects::*;
        // The owned Job handle stays alive through the entire attempt. Codex receives
        // its prompt only after assignment, and descendants inherit the Job.
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(std::io::Error::last_os_error().into());
            }
            let owned = Self { handle };
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                std::mem::size_of_val(&limits) as u32,
            ) == 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
            let process = child
                .raw_handle()
                .ok_or_else(|| Error::Runtime("Process handle недоступен".into()))?;
            if AssignProcessToJobObject(handle, process as _) == 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            Ok(owned)
        }
    }
    pub fn terminate(&self) {
        unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.handle, 1);
        }
    }
}
#[cfg(windows)]
impl Drop for ProcessContainment {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}
#[cfg(unix)]
pub struct ProcessContainment {
    pid: i32,
}
#[cfg(unix)]
impl ProcessContainment {
    pub fn attach(_child: &tokio::process::Child, pid: u32) -> Result<Self> {
        Ok(Self { pid: pid as i32 })
    }
    pub fn terminate(&self) {
        unsafe {
            libc::kill(-self.pid, libc::SIGKILL);
        }
    }
}
#[cfg(unix)]
impl Drop for ProcessContainment {
    fn drop(&mut self) {
        self.terminate();
    }
}
