//! `run_to_completion` với `fake-mc --install` (spec P3, PR-02); không cần Java.

#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use mh_core::ConsoleStream;
use mh_runtime::{Completion, OutputLine, ProcessRuntime, ProcessSpec};

const LONG: Duration = Duration::from_secs(30);

fn installer(dir: &std::path::Path, args: &[&str]) -> ProcessSpec {
    let mut all = vec!["--install".to_owned()];
    all.extend(args.iter().map(|a| (*a).to_owned()));
    ProcessSpec {
        program: PathBuf::from(env!("CARGO_BIN_EXE_fake-mc")),
        args: all,
        work_dir: dir.to_path_buf(),
        limits: None,
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mh-complete-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

async fn run(spec: ProcessSpec, timeout: Duration) -> (Completion, Vec<OutputLine>) {
    let rt = ProcessRuntime::new().unwrap();
    let mut lines = Vec::new();
    let done = rt
        .run_to_completion(spec, timeout, std::future::pending(), |l| lines.push(l))
        .await
        .unwrap();
    (done, lines)
}

#[tokio::test]
async fn success_writes_files_and_keeps_every_line() {
    let dir = temp_dir("ok");
    let spec = installer(
        &dir,
        &["--write", "libraries/x/unix_args.txt", "--read-stdin"],
    );
    let (done, lines) = run(spec, LONG).await;
    assert_eq!(done, Completion::Exited { code: Some(0) });
    assert!(done.success());
    assert!(dir.join("libraries/x/unix_args.txt").is_file());
    let texts: Vec<_> = lines.iter().map(|l| l.text.as_str()).collect();
    assert!(texts.contains(&"stdin closed after 0 lines"), "{texts:?}");
    // Dòng cuối ra stderr ngay trước khi thoát vẫn phải tới tay người gọi.
    assert!(
        lines
            .iter()
            .any(|l| l.stream == ConsoleStream::Stderr && l.text.contains("finished with code 0")),
        "{texts:?}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn failure_reports_exit_code() {
    let dir = temp_dir("fail");
    let (done, _) = run(installer(&dir, &["--exit-code", "3"]), LONG).await;
    assert_eq!(done, Completion::Exited { code: Some(3) });
    assert!(!done.success());
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn timeout_kills_the_process() {
    let dir = temp_dir("timeout");
    let started = Instant::now();
    let (done, _) = run(
        installer(&dir, &["--sleep-ms", "60000"]),
        Duration::from_millis(500),
    )
    .await;
    assert_eq!(done, Completion::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(20));
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn cancel_kills_the_process() {
    let dir = temp_dir("cancel");
    let rt = ProcessRuntime::new().unwrap();
    let cancel = tokio::time::sleep(Duration::from_millis(300));
    let started = Instant::now();
    let done = rt
        .run_to_completion(
            installer(&dir, &["--sleep-ms", "60000"]),
            LONG,
            cancel,
            |_| {},
        )
        .await
        .unwrap();
    assert_eq!(done, Completion::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(20));
    let _ = std::fs::remove_dir_all(dir);
}
