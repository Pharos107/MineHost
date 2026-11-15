//! Luồng spawner: mọi process server được spawn từ một luồng `std::thread` sống suốt vòng đời
//! controller.
//!
//! Trên Linux, `PR_SET_PDEATHSIG` gửi tín hiệu khi **luồng** đã fork thoát. Spawn từ
//! `spawn_blocking` (luồng tự thoát sau 10 giây rảnh) hoặc từ một luồng tokio bất kỳ sẽ làm
//! server bị kill vô cớ. Trên Windows, luồng này giữ Job Object.

use std::io;
use std::process::Stdio;
use std::sync::mpsc;

use tokio::process::{Child, Command};
use tokio::runtime::Handle;
use tokio::sync::oneshot;

use crate::ProcessSpec;
use crate::os::{Guard, ProcessGuard};

type Spawned = (Child, ProcessGuard);

struct Request {
    spec: ProcessSpec,
    reply: oneshot::Sender<io::Result<Spawned>>,
}

#[derive(Debug)]
pub(crate) struct Spawner {
    tx: mpsc::Sender<Request>,
    limits_enforced: bool,
}

impl Spawner {
    /// Luồng dừng khi `Spawner` bị drop, kéo theo mọi process con: đây là ý đồ thiết kế,
    /// vì server dừng theo controller (ADR 0001).
    pub(crate) fn start(handle: Handle) -> io::Result<Self> {
        let (tx, rx) = mpsc::channel::<Request>();
        let (ready_tx, ready_rx) = mpsc::channel::<io::Result<bool>>();
        std::thread::Builder::new()
            .name("mh-spawner".to_owned())
            .spawn(move || {
                // `tokio::process` cần reactor của runtime để theo dõi process con.
                let _runtime = handle.enter();
                let guard = match Guard::new() {
                    Ok(guard) => guard,
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };
                let _ = ready_tx.send(Ok(guard.limits_enforced()));
                for req in rx {
                    let _ = req.reply.send(spawn_one(&guard, &req.spec));
                }
            })?;
        let limits_enforced = ready_rx
            .recv()
            .map_err(|_| io::Error::other("luồng spawner dừng trước khi sẵn sàng"))??;
        Ok(Self {
            tx,
            limits_enforced,
        })
    }

    pub(crate) fn limits_enforced(&self) -> bool {
        self.limits_enforced
    }

    pub(crate) async fn spawn(&self, spec: ProcessSpec) -> io::Result<Spawned> {
        let stopped = || io::Error::other("luồng spawner đã dừng");
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(Request { spec, reply })
            .map_err(|_| stopped())?;
        rx.await.map_err(|_| stopped())?
    }
}

fn spawn_one(guard: &Guard, spec: &ProcessSpec) -> io::Result<Spawned> {
    let mut cmd = Command::new(&spec.program);
    cmd.args(&spec.args)
        .current_dir(&spec.work_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(false);
    let pending = guard.prepare(&mut cmd, spec.limits);
    let mut child = cmd.spawn()?;
    let process_guard = match guard.adopt(&child, pending) {
        Ok(process_guard) => process_guard,
        Err(e) => {
            // Không gắn được thì process có thể sống sót sau controller: không chạy nó.
            let _ = child.start_kill();
            return Err(e);
        }
    };
    // Ghi log mọi lần spawn, để giải thích được với người dùng khi antivirus hỏi.
    tracing::info!(
        program = %spec.program.display(),
        dir = %spec.work_dir.display(),
        pid = child.id(),
        "đã chạy process server"
    );
    Ok((child, process_guard))
}
