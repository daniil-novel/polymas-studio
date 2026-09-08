//! Opt-in real provider check. Never run in ordinary CI.
use polymas_core::codex::{CodexRequest, probe_codex, run_codex};
use polymas_core::error::Result;
use tokio::sync::{mpsc, watch};

#[tokio::main]
async fn main() -> Result<()> {
    if !std::env::args().any(|arg| arg == "--allow-provider") {
        eprintln!("Pass --allow-provider to use your Codex account for two short turns.");
        std::process::exit(2);
    }
    let probe = probe_codex(None).await?;
    println!(
        "Codex {}: compatible={}, auth={}",
        probe.version, probe.compatible, probe.auth_method
    );
    if !probe.compatible || !probe.authenticated {
        return Err(polymas_core::error::Error::Runtime(
            "Compatibility/auth gate failed".into(),
        ));
    }
    let workspace = std::env::current_dir()?.join("tmp/real-codex-smoke/Агент один");
    std::fs::create_dir_all(&workspace)?;
    let (_cancel, stop) = watch::channel(false);
    let mut session = None;
    let mut pids = vec![];
    for prompt in [
        "Remember the marker POLYMAS-CHECK-42. Reply with that marker only. Do not use tools.",
        "What marker did I ask you to remember? Reply with that marker only. Do not use tools.",
    ] {
        let (tx, mut rx) = mpsc::channel(128);
        let log = tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                let event: polymas_core::process::ProcessEvent = event;
                println!("event: {}", event.kind);
            }
        });
        let result = run_codex(
            probe.path.clone(),
            CodexRequest {
                workspace: workspace.clone(),
                session_id: session.clone(),
                prompt: prompt.into(),
                timeout_seconds: 120,
                ..Default::default()
            },
            tx,
            stop.clone(),
        )
        .await?;
        log.await.expect("event logger");
        assert!(result.output.contains("POLYMAS-CHECK-42"));
        if let Some(previous) = &session {
            assert_eq!(previous, &result.session_id);
        }
        session = Some(result.session_id);
        pids.push(result.pid);
        println!("Verified marker, PID={}", result.pid);
    }
    assert_ne!(pids[0], pids[1]);
    println!("PASS: two separate Codex processes; one explicit resumed session; marker retained");
    Ok(())
}
