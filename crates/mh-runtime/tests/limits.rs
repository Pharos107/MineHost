//! Giới hạn tài nguyên của process (spec P3, mục 3.8) với `fake-mc`; không cần Java.

#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::time::Duration;

use mh_runtime::{Exit, ProcessRuntime, ProcessSpec, ResourceLimits};

const WAIT: Duration = Duration::from_secs(60);

fn fake_mc(args: &[&str], limits: Option<ResourceLimits>) -> ProcessSpec {
    ProcessSpec {
        program: PathBuf::from(env!("CARGO_BIN_EXE_fake-mc")),
        args: args.iter().map(|a| (*a).to_owned()).collect(),
        work_dir: std::env::temp_dir(),
        limits,
    }
}

async fn exit_of(spec: ProcessSpec) -> Exit {
    let rt = ProcessRuntime::new().unwrap();
    let proc = rt.spawn(spec).await.unwrap();
    // Ngừng đọc log cũng không sao: kênh không giới hạn, pipe vẫn được đọc.
    drop(proc.control);
    tokio::time::timeout(WAIT, proc.exit).await.unwrap().unwrap()
}

#[tokio::test]
async fn normal_exit_is_not_a_memory_limit() {
    let limits = ResourceLimits::for_heap_mb(512);
    let exit = exit_of(fake_mc(&["--install", "--exit-code", "0"], Some(limits))).await;
    assert_eq!(exit.code, Some(0));
    assert!(!exit.memory_limit_hit);
}

#[test]
fn limits_follow_heap_formula() {
    let limits = ResourceLimits::for_heap_mb(512);
    assert_eq!(limits.memory_bytes, 1792 * 1024 * 1024);
    assert_eq!(limits.max_processes, ResourceLimits::MAX_PROCESSES);
}

/// Job Object chặn commit vượt giới hạn; `fake-mc` không cấp phát được thì thoát.
#[cfg(windows)]
#[tokio::test]
async fn windows_job_stops_process_over_memory_limit() {
    let limits = ResourceLimits {
        memory_bytes: 64 * 1024 * 1024,
        max_processes: 4,
    };
    let exit = exit_of(fake_mc(&["--alloc-mb", "512"], Some(limits))).await;
    assert!(exit.memory_limit_hit, "{exit:?}");
    assert_ne!(exit.code, Some(0));
}
