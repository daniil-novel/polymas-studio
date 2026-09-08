use polymas_core::process::{ProcessSpec, run_process};
use std::time::Duration;
use tokio::sync::{mpsc, watch};

fn fixture(directory: &std::path::Path, script: &str) -> ProcessSpec {
    ProcessSpec {
        executable: if cfg!(windows) { "python" } else { "python3" }.into(),
        args: vec!["-u".into(), "-c".into(), script.into()],
        workspace: directory.into(),
        stdin: "fixture input".into(),
        timeout_seconds: 10,
        environment: Default::default(),
    }
}
#[tokio::test]
async fn independent_processes_return_distinct_pids_and_json_events() {
    let dir = tempfile::tempdir().unwrap();
    let script = "import json,sys,time; sys.stdin.read(); print(json.dumps({'type':'turn.completed','usage':{'input_tokens':1}})); time.sleep(0.05)";
    let (tx, mut rx) = mpsc::channel(32);
    let (_cancel, stop) = watch::channel(false);
    let (one, two) = tokio::join!(
        run_process(fixture(dir.path(), script), tx.clone(), stop.clone()),
        run_process(fixture(dir.path(), script), tx, stop)
    );
    let one = one.unwrap();
    let two = two.unwrap();
    assert_ne!(one.pid, two.pid);
    assert!(one.success && two.success);
    let mut count = 0;
    while let Some(e) = rx.recv().await {
        if e.kind == "runtime.event" {
            count += 1;
        }
    }
    assert_eq!(count, 2);
}
#[tokio::test]
async fn cancellation_interrupts_a_real_child_and_reaps_it() {
    let dir = tempfile::tempdir().unwrap();
    let (tx, mut rx) = mpsc::channel(32);
    let (cancel, stop) = watch::channel(false);
    let task = tokio::spawn(run_process(
        fixture(
            dir.path(),
            "import sys,time; sys.stdin.read(); time.sleep(30)",
        ),
        tx,
        stop,
    ));
    let started = rx.recv().await.unwrap();
    assert_eq!(started.kind, "runtime.started");
    cancel.send(true).unwrap();
    let result = tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(result.cancelled);
    assert!(!result.success);
}
#[tokio::test]
async fn protocol_corruption_fails_even_when_exit_code_is_zero() {
    let dir = tempfile::tempdir().unwrap();
    let (tx, _rx) = mpsc::channel(32);
    let (_cancel, stop) = watch::channel(false);
    let result = run_process(fixture(dir.path(), "print('not json')"), tx, stop).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn closing_pipes_does_not_disable_cancellation() {
    let dir = tempfile::tempdir().unwrap();
    let (tx, mut rx) = mpsc::channel(32);
    let (cancel, stop) = watch::channel(false);
    let task = tokio::spawn(run_process(
        fixture(
            dir.path(),
            "import os,time,sys; sys.stdin.read(); os.close(1); os.close(2); time.sleep(30)",
        ),
        tx,
        stop,
    ));
    rx.recv().await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    cancel.send(true).unwrap();
    let result = tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(result.cancelled);
}
