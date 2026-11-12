//! Test WebSocket `/api/v1/ws` với server HTTP thật trên cổng ngẫu nhiên. "Java" là `fake-mc`.

// Hàm trợ giúp ngoài `#[test]` không được clippy coi là code test.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use mh_app::sources::{
    BoxFut, CancellationToken, CatalogVersion, Checksum, JavaRuntime, ProgressFn, ResolvedServer,
    ServerJar,
};
use mh_app::{App, AppDeps, AppError, CreateRequest, Created, JavaSource, JobState, ServerCatalog};
use mh_core::{LoaderKind, McVersion, ServerId, ServerName};
use mh_platform::DataDirs;
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{Message, http};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

const WAIT: Duration = Duration::from_secs(10);

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct FakeCatalog;

impl ServerCatalog for FakeCatalog {
    fn versions(&self, _: LoaderKind) -> BoxFut<'_, Result<Vec<CatalogVersion>, AppError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn resolve<'a>(
        &'a self,
        _: LoaderKind,
        minecraft: &'a McVersion,
    ) -> BoxFut<'a, Result<ResolvedServer, AppError>> {
        Box::pin(async move {
            Ok(ResolvedServer {
                minecraft: minecraft.clone(),
                build: Some(1),
                java_major: 21,
                jar: ServerJar {
                    url: "https://example.invalid/x.jar".to_owned(),
                    checksum: Checksum::sha256(&"0".repeat(64)).unwrap(),
                    size: 1,
                },
            })
        })
    }

    fn download<'a>(
        &'a self,
        _: &'a ServerJar,
        dest: &'a Path,
        _: ProgressFn<'a>,
        _: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(async move {
            std::fs::write(dest, b"jar").unwrap();
            Ok(())
        })
    }
}

struct FakeJava;

impl JavaSource for FakeJava {
    fn ensure<'a>(
        &'a self,
        major: u16,
        _: ProgressFn<'a>,
        _: &'a CancellationToken,
    ) -> BoxFut<'a, Result<JavaRuntime, AppError>> {
        Box::pin(async move {
            let bin = PathBuf::from(env!("CARGO_BIN_EXE_api-fake-mc"));
            Ok(JavaRuntime {
                major,
                home: bin.parent().unwrap().to_path_buf(),
                java_bin: bin,
                version: "21".to_owned(),
            })
        })
    }
}

struct Harness {
    _tmp: tempfile::TempDir,
    app: Arc<App>,
    port: u16,
}

async fn harness() -> Harness {
    build_harness(mh_app::MetricsConfig::default(), false).await
}

async fn harness_sampling() -> Harness {
    build_harness(
        mh_app::MetricsConfig {
            sample_every: Duration::from_millis(50),
            disk_every: Duration::from_millis(50),
        },
        true,
    )
    .await
}

async fn build_harness(metrics: mh_app::MetricsConfig, sample: bool) -> Harness {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    dirs.ensure().unwrap();
    let deps = AppDeps {
        catalog: Arc::new(FakeCatalog),
        java: Arc::new(FakeJava),
        port_free: Arc::new(|_| true),
        free_space: Arc::new(|_| None),
        total_memory_mb: 8192,
        supervisor: mh_supervisor::SupervisorConfig::default(),
        save_flush_timeout: Duration::from_secs(60),
        metrics,
    };
    let app = Arc::new(App::with_deps(dirs, deps).unwrap());
    if sample {
        app.spawn_metrics();
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    // Cùng kiểu với `serve.rs`: đăng nhập đọc IP từ `ConnectInfo`.
    let router = mh_api::router(Arc::clone(&app), port)
        .into_make_service_with_connect_info::<std::net::SocketAddr>();
    tokio::spawn(async move { axum::serve(listener, router).await });
    Harness {
        _tmp: tmp,
        app,
        port,
    }
}

impl Harness {
    async fn connect(&self, origin: Option<&str>) -> Result<Ws, String> {
        let url = format!("ws://127.0.0.1:{}/api/v1/ws", self.port);
        let mut req = url.into_client_request().unwrap();
        if let Some(origin) = origin {
            req.headers_mut()
                .insert(http::header::ORIGIN, origin.parse().unwrap());
        }
        tokio_tungstenite::connect_async(req)
            .await
            .map(|(ws, _)| ws)
            .map_err(|e| e.to_string())
    }

    async fn subscribe(&self, msg: Value) -> Ws {
        let origin = format!("http://127.0.0.1:{}", self.port);
        let mut ws = self.connect(Some(&origin)).await.unwrap();
        ws.send(Message::Text(msg.to_string().into()))
            .await
            .unwrap();
        ws
    }

    /// Server đã cài và đồng ý EULA, chưa bật.
    async fn installed_server(&self) -> ServerId {
        let req = CreateRequest {
            name: ServerName::parse("WS").unwrap(),
            loader: LoaderKind::Paper,
            minecraft: McVersion::parse("1.21.4").unwrap(),
            memory_mb: None,
            port: None,
        };
        let created = self.app.create(req).unwrap();
        let mut rx = self.app.jobs().subscribe(created.job_id).unwrap();
        rx.wait_for(|s| s.state != JobState::Running).await.unwrap();
        self.app.accept_eula(created.server_id).unwrap();
        created.server_id
    }

    fn create_job(&self, name: &str) -> Created {
        self.app
            .create(CreateRequest {
                name: ServerName::parse(name).unwrap(),
                loader: LoaderKind::Paper,
                minecraft: McVersion::parse("1.21.4").unwrap(),
                memory_mb: None,
                port: None,
            })
            .unwrap()
    }
}

/// Đọc tin tới khi `pred` đúng; trả mọi tin đã đọc.
async fn read_until(ws: &mut Ws, pred: impl Fn(&Value) -> bool) -> Vec<Value> {
    let mut seen = Vec::new();
    let found = tokio::time::timeout(WAIT, async {
        while let Some(Ok(msg)) = ws.next().await {
            if let Message::Text(text) = msg {
                let value: Value = serde_json::from_str(&text).unwrap();
                let hit = pred(&value);
                seen.push(value);
                if hit {
                    return true;
                }
            }
        }
        false
    })
    .await;
    if found != Ok(true) {
        panic!("không thấy tin mong đợi; đã đọc {seen:#?}");
    }
    seen
}

fn text_contains(needle: &'static str) -> impl Fn(&Value) -> bool {
    move |v| {
        v["data"]["text"]
            .as_str()
            .is_some_and(|t| t.contains(needle))
    }
}

#[tokio::test]
async fn console_streams_new_lines_and_commands() {
    let h = harness().await;
    let id = h.installed_server().await;
    let topic = format!("server.{id}.console");
    let mut ws = h.subscribe(json!({ "subscribe": [topic] })).await;

    h.app.start(id).await.unwrap();
    let lines = read_until(&mut ws, text_contains("Done (")).await;
    assert!(lines.iter().all(|m| m["topic"] == topic));
    assert!(lines.iter().any(|m| {
        m["data"]["text"]
            .as_str()
            .is_some_and(|t| t.contains("Xin chào thế giới"))
    }));

    h.app
        .send_command(id, "say chào qua websocket")
        .await
        .unwrap();
    let lines = read_until(&mut ws, text_contains("[Server] chào qua websocket")).await;
    assert!(
        lines.iter().any(
            |m| m["data"]["stream"] == "stdin" && m["data"]["text"] == "say chào qua websocket"
        )
    );
    h.app.kill(id).await.unwrap();
}

#[tokio::test]
async fn reconnect_with_since_gets_missed_lines() {
    let h = harness().await;
    let id = h.installed_server().await;
    let topic = format!("server.{id}.console");
    let mut ws = h.subscribe(json!({ "subscribe": [topic] })).await;
    h.app.start(id).await.unwrap();
    let seen = read_until(&mut ws, text_contains("Done (")).await;
    let last_seq = seen.last().unwrap()["seq"].as_u64().unwrap();
    ws.close(None).await.unwrap();

    // Trong lúc không có ai nghe, server vẫn in log.
    h.app.send_command(id, "say lúc vắng mặt").await.unwrap();
    let handle = h.app.handle(id).unwrap();
    tokio::time::timeout(WAIT, async {
        while !handle
            .console()
            .after(Some(last_seq))
            .iter()
            .any(|l| l.text.contains("[Server] lúc vắng mặt"))
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();

    let msg = json!({ "subscribe": [topic], "since": { topic.clone(): last_seq } });
    let mut ws = h.subscribe(msg).await;
    let replay = read_until(&mut ws, text_contains("[Server] lúc vắng mặt")).await;
    let seqs: Vec<u64> = replay.iter().map(|m| m["seq"].as_u64().unwrap()).collect();
    assert!(seqs.iter().all(|s| *s > last_seq), "{seqs:?}");
    assert!(
        seqs.windows(2).all(|w| w[1] > w[0]),
        "không được trùng hay đảo: {seqs:?}"
    );
    assert!(
        replay
            .iter()
            .any(|m| m["data"]["text"] == "say lúc vắng mặt")
    );
    h.app.kill(id).await.unwrap();
}

#[tokio::test]
async fn foreign_origin_is_refused() {
    let h = harness().await;
    let err = h.connect(Some("https://evil.example")).await.unwrap_err();
    assert!(err.contains("403"), "{err}");
    // Không có Origin (và không có `X-MineHost`) cũng bị từ chối.
    let err = h.connect(None).await.unwrap_err();
    assert!(err.contains("403"), "{err}");
}

#[tokio::test]
async fn job_topic_reports_until_finished() {
    let h = harness().await;
    let req = CreateRequest {
        name: ServerName::parse("Job").unwrap(),
        loader: LoaderKind::Paper,
        minecraft: McVersion::parse("1.21.4").unwrap(),
        memory_mb: None,
        port: None,
    };
    let created = h.app.create(req).unwrap();
    let topic = format!("job.{}", created.job_id);
    let mut ws = h.subscribe(json!({ "subscribe": [topic] })).await;
    let msgs = read_until(&mut ws, |m| m["data"]["state"] != "running").await;
    let last = msgs.last().unwrap();
    assert_eq!(last["topic"], topic);
    assert_eq!(last["data"]["state"], "succeeded");
    assert_eq!(last["data"]["server_id"], created.server_id.to_string());
}

#[tokio::test]
async fn jobs_topic_receives_a_new_job() {
    let h = harness().await;
    let mut ws = h.subscribe(json!({ "subscribe": ["jobs"] })).await;
    // Thấy Job đầu thì feed đã đăng ký kênh; Job sau chỉ tới nếu broadcast chạy.
    let first = h.create_job("Một");
    read_until(&mut ws, |m| {
        m["topic"] == "jobs" && m["data"]["id"].as_u64() == Some(first.job_id)
    })
    .await;
    let second = h.create_job("Hai");
    let msgs = read_until(&mut ws, |m| {
        m["topic"] == "jobs" && m["data"]["id"].as_u64() == Some(second.job_id)
    })
    .await;
    let hit = msgs.last().unwrap();
    assert_eq!(hit["data"]["kind"], "create_server");
    assert_eq!(hit["data"]["server_id"], second.server_id.to_string());
    assert!(
        hit["data"]["started_at"]
            .as_str()
            .is_some_and(|s| s.contains('T'))
    );
    assert!(hit["data"].get("finished_at").is_some());
}

#[tokio::test]
async fn servers_topic_sends_list_then_changes() {
    let h = harness().await;
    let id = h.installed_server().await;
    let mut ws = h.subscribe(json!({ "subscribe": ["servers"] })).await;
    let first = read_until(&mut ws, |m| m["data"]["id"] == id.to_string()).await;
    assert_eq!(first.last().unwrap()["data"]["state"], "stopped");

    h.app.start(id).await.unwrap();
    read_until(&mut ws, |m| m["data"]["state"] == "running").await;
    h.app.kill(id).await.unwrap();
    read_until(&mut ws, |m| m["data"]["state"] == "stopped").await;
    h.app.trash(id, "WS").unwrap();
    read_until(&mut ws, |m| m["data"]["removed"] == true).await;
}

#[tokio::test]
async fn metrics_topic_receives_a_sample() {
    let h = harness_sampling().await;
    let id = h.installed_server().await;
    let topic = format!("server.{id}.metrics");
    let mut ws = h.subscribe(json!({ "subscribe": [topic] })).await;
    h.app.start(id).await.unwrap();
    let msgs = read_until(&mut ws, |m| {
        m["topic"] == topic && m["data"]["memory_bytes"].as_u64().is_some_and(|n| n > 0)
    })
    .await;
    let data = &msgs.last().unwrap()["data"];
    assert!(data["cpu_percent"].is_number());
    assert!(data["players"].is_null());
    assert!(data["at"].as_str().unwrap().contains('T'));
    h.app.kill(id).await.unwrap();
}

#[tokio::test]
async fn bad_topic_gets_an_error_message() {
    let h = harness().await;
    let mut ws = h
        .subscribe(json!({ "subscribe": ["server.not-an-id.console", "job.999"] }))
        .await;
    let msgs = read_until(&mut ws, |m| m["data"]["code"] == "NOT_FOUND").await;
    assert!(msgs.iter().any(|m| m["topic"] == "error"
        && m["data"]["code"] == "INVALID_INPUT"
        && m["data"]["params"]["topic"] == "server.not-an-id.console"));
}
