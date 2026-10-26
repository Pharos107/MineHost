//! WebSocket `/api/v1/ws` (spec P1 mục 5.8, topic `jobs` ở P2 mục 6.3).
//!
//! Client gửi `{ "subscribe": ["servers", "jobs", "server.<id>.console", "server.<id>.metrics",
//! "job.<id>"],
//! "since": { "server.<id>.console": 120 } }`. Server gửi `{ "topic", "seq", "data" }`.
//! `jobs` gửi `JobDto` mỗi khi một Job đổi trạng thái hoặc tiến độ, kể cả Job mới.
//! Với console, `seq` là `seq` của dòng: kết nối lại với `since` thì nhận tiếp từ bộ đệm.
//! `server.<id>.metrics` gửi từng mẫu mới; `since` không áp dụng cho topic này.
//! Origin đã được `guard` kiểm tra trước khi nâng cấp kết nối.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use mh_app::jobs::JobState;
use mh_app::{App, AppEvent, MetricsEvent, SupervisorEvent};
use mh_core::{ErrorCode, ServerId};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use crate::dto::{ConsoleLineDto, JobDto, MetricSampleDto, ServerDto};

const SERVERS: &str = "servers";
const JOBS: &str = "jobs";

#[derive(Debug, Default, Deserialize)]
struct ClientMessage {
    #[serde(default)]
    subscribe: Vec<String>,
    #[serde(default)]
    since: HashMap<String, u64>,
}

#[derive(Debug, Serialize)]
struct ServerMessage<'a> {
    topic: &'a str,
    seq: u64,
    data: Value,
}

/// Gửi tin cho client qua kênh chung của kết nối. `false` khi kết nối đã đóng.
#[derive(Debug, Clone)]
struct Outbox(mpsc::Sender<String>);

impl Outbox {
    async fn send(&self, topic: &str, seq: u64, data: Value) -> bool {
        let Ok(text) = serde_json::to_string(&ServerMessage { topic, seq, data }) else {
            return true;
        };
        self.0.send(text).await.is_ok()
    }

    async fn error(&self, topic: &str, code: ErrorCode) -> bool {
        let data = json!({ "code": code.as_str(), "params": { "topic": topic } });
        self.send("error", 0, data).await
    }
}

pub async fn upgrade(State(app): State<Arc<App>>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| run(app, socket))
}

enum Wake {
    Incoming(Option<Result<Message, axum::Error>>),
    Outgoing(Option<String>),
}

async fn run(app: Arc<App>, mut socket: WebSocket) {
    let (tx, mut rx) = mpsc::channel(256);
    let outbox = Outbox(tx);
    // Drop `JoinSet` khi thoát vòng lặp là hủy mọi task theo dõi topic của kết nối này.
    let mut feeds = JoinSet::new();
    let mut subscribed = HashSet::new();
    loop {
        let wake = tokio::select! {
            msg = socket.recv() => Wake::Incoming(msg),
            out = rx.recv() => Wake::Outgoing(out),
        };
        match wake {
            Wake::Incoming(Some(Ok(Message::Text(text)))) => {
                let Ok(msg) = serde_json::from_str::<ClientMessage>(&text) else {
                    let _ = outbox.error("", ErrorCode::InvalidInput).await;
                    continue;
                };
                for topic in msg.subscribe {
                    if subscribed.insert(topic.clone()) {
                        let since = msg.since.get(&topic).copied();
                        feeds.spawn(feed(Arc::clone(&app), topic, since, outbox.clone()));
                    }
                }
            }
            Wake::Incoming(Some(Ok(Message::Close(_))) | Some(Err(_)) | None) => break,
            Wake::Incoming(Some(Ok(_))) => {}
            Wake::Outgoing(Some(text)) => {
                if socket.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
            Wake::Outgoing(None) => break,
        }
    }
}

async fn feed(app: Arc<App>, topic: String, since: Option<u64>, out: Outbox) {
    if topic == SERVERS {
        return servers_feed(&app, &out).await;
    }
    if topic == JOBS {
        return jobs_feed(&app, &out).await;
    }
    if let Some((id, kind)) = server_topic(&topic) {
        return server_feed(&app, &topic, id, kind, since, &out).await;
    }
    if let Some(id) = topic.strip_prefix("job.").and_then(|id| id.parse().ok()) {
        return job_feed(&app, &topic, id, &out).await;
    }
    let _ = out.error(&topic, ErrorCode::InvalidInput).await;
}

/// `server.<id>.console` hoặc `server.<id>.metrics`. Id không phải ULID thì `None`.
fn server_topic(topic: &str) -> Option<(ServerId, &str)> {
    let rest = topic.strip_prefix("server.")?;
    let (id, kind) = rest.rsplit_once('.')?;
    ServerId::parse(id).ok().map(|id| (id, kind))
}

async fn server_feed(
    app: &App,
    topic: &str,
    id: ServerId,
    kind: &str,
    since: Option<u64>,
    out: &Outbox,
) {
    match kind {
        "console" => console_feed(app, topic, id, since, out).await,
        "metrics" => metrics_feed(app, topic, id, out).await,
        _ => {
            let _ = out.error(topic, ErrorCode::InvalidInput).await;
        }
    }
}

async fn metrics_feed(app: &App, topic: &str, id: ServerId, out: &Outbox) {
    if let Err(err) = app.metrics(id, None) {
        let _ = out.error(topic, err.code).await;
        return;
    }
    // Đăng ký sau khi biết server tồn tại. Mẫu cũ lấy bằng REST; ở đây chỉ mẫu mới.
    let mut events = app.subscribe_metrics();
    let mut seq = 0u64;
    loop {
        let sample = match events.recv().await {
            Ok(MetricsEvent { id: got, sample }) if got == id => sample,
            Ok(_) | Err(RecvError::Lagged(_)) => continue,
            Err(RecvError::Closed) => return,
        };
        seq += 1;
        let data = serde_json::to_value(MetricSampleDto::from(sample)).unwrap_or_default();
        if !out.send(topic, seq, data).await {
            return;
        }
    }
}

/// Gửi toàn bộ danh sách lúc đăng ký, sau đó mỗi lần một server đổi thì gửi lại server đó.
/// Server bị xóa được gửi dạng `{ "id", "removed": true }`.
async fn servers_feed(app: &App, out: &Outbox) {
    let mut events = app.subscribe_events();
    let mut seq = 0;
    let initial = app.list().unwrap_or_default();
    for view in initial {
        seq += 1;
        if !out.send(SERVERS, seq, server_data(view.into())).await {
            return;
        }
    }
    loop {
        let id = match events.recv().await {
            Ok(AppEvent::ServerChanged(id)) => id,
            // Lỡ sự kiện thì gửi lại cả danh sách cho chắc.
            Err(RecvError::Lagged(_)) => {
                for view in app.list().unwrap_or_default() {
                    seq += 1;
                    if !out.send(SERVERS, seq, server_data(view.into())).await {
                        return;
                    }
                }
                continue;
            }
            Err(RecvError::Closed) => return,
        };
        let data = match app.get(id) {
            Ok(view) => server_data(view.into()),
            Err(_) => json!({ "id": id.to_string(), "removed": true }),
        };
        seq += 1;
        if !out.send(SERVERS, seq, data).await {
            return;
        }
    }
}

fn server_data(dto: ServerDto) -> Value {
    serde_json::to_value(dto).unwrap_or_default()
}

/// Gửi các Job đang giữ lúc đăng ký, rồi mỗi lần một Job đổi.
///
/// Đăng ký kênh trước khi đọc danh sách để không lỡ Job tạo ở giữa. Lỡ tin
/// (`Lagged`) thì gửi lại danh sách: bản mới nhất vẫn tới, tiến độ kẹt giữa thì mất.
async fn jobs_feed(app: &App, out: &Outbox) {
    let mut events = app.jobs().subscribe_all();
    let mut seq = 0;
    if !send_job_list(app, out, &mut seq).await {
        return;
    }
    loop {
        match events.recv().await {
            Ok(snapshot) => {
                seq += 1;
                if !send_job(out, seq, snapshot).await {
                    return;
                }
            }
            Err(RecvError::Lagged(_)) => {
                if !send_job_list(app, out, &mut seq).await {
                    return;
                }
            }
            Err(RecvError::Closed) => return,
        }
    }
}

async fn send_job_list(app: &App, out: &Outbox, seq: &mut u64) -> bool {
    for snapshot in app.jobs().list(None) {
        *seq += 1;
        if !send_job(out, *seq, snapshot).await {
            return false;
        }
    }
    true
}

async fn send_job(out: &Outbox, seq: u64, snapshot: mh_app::jobs::JobSnapshot) -> bool {
    let data = serde_json::to_value(JobDto::from(snapshot)).unwrap_or_default();
    out.send(JOBS, seq, data).await
}

async fn console_feed(app: &App, topic: &str, id: ServerId, since: Option<u64>, out: &Outbox) {
    let handle = match app.handle(id) {
        Ok(handle) => handle,
        Err(e) => {
            let _ = out.error(topic, e.code).await;
            return;
        }
    };
    // Đăng ký trước rồi mới đọc bộ đệm, để không lỡ dòng nào ở giữa; trùng thì lọc theo `seq`.
    let mut events = handle.subscribe();
    let mut last = since;
    loop {
        for line in handle.console().after(last) {
            last = Some(line.seq);
            if !send_line(out, topic, line).await {
                return;
            }
        }
        match events.recv().await {
            Ok(SupervisorEvent::Console(line)) if last.is_none_or(|l| line.seq > l) => {
                last = Some(line.seq);
                if !send_line(out, topic, line).await {
                    return;
                }
            }
            // Lỡ dòng thì vòng lặp sau đọc bù từ bộ đệm.
            Ok(_) | Err(RecvError::Lagged(_)) => {}
            Err(RecvError::Closed) => return,
        }
    }
}

async fn send_line(out: &Outbox, topic: &str, line: mh_core::ConsoleLine) -> bool {
    let seq = line.seq;
    let data = serde_json::to_value(ConsoleLineDto::from(line)).unwrap_or_default();
    out.send(topic, seq, data).await
}

async fn job_feed(app: &App, topic: &str, id: u64, out: &Outbox) {
    let Some(mut rx) = app.jobs().subscribe(id) else {
        let _ = out.error(topic, ErrorCode::NotFound).await;
        return;
    };
    let mut seq = 0;
    loop {
        let snapshot = rx.borrow_and_update().clone();
        let done = snapshot.state != JobState::Running;
        seq += 1;
        let data = serde_json::to_value(JobDto::from(snapshot)).unwrap_or_default();
        if !out.send(topic, seq, data).await || done || rx.changed().await.is_err() {
            return;
        }
    }
}
