//! Người chơi: sửa file khi server tắt, gửi lệnh khi đang chạy (spec P2, mục 6.7; kế hoạch PR-17).

#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use mh_app::sources::{BoxFut, CatalogVersion, ProgressFn, ResolvedServer};
use mh_app::{
    ActionDone, App, AppDeps, AppError, CreateRequest, JavaSource, JobState, MetricsConfig,
    PlayerDirectory, PlayerProfile, PlayerRequest, ServerCatalog,
};
use mh_core::{
    ErrorCode, LoaderKind, McVersion, PlayerAction, PlayerName, ServerId, ServerName, ServerState,
};
use mh_java::JavaRuntime;
use mh_net::{CancellationToken, Checksum};
use mh_platform::DataDirs;
use mh_providers::ServerJar;
use mh_supervisor::SupervisorConfig;
use time::macros::datetime;

const WAIT: Duration = Duration::from_secs(10);
const NOTCH_UUID: &str = "069a79f4-44e9-4726-a5be-fca90e38aaf5";

struct FakeCatalog;

impl ServerCatalog for FakeCatalog {
    fn versions(&self, _: LoaderKind) -> BoxFut<'_, Result<Vec<CatalogVersion>, AppError>> {
        Box::pin(async {
            Ok(vec![CatalogVersion {
                id: McVersion::parse("1.21.4").unwrap(),
                release_time: datetime!(2024-12-03 10:12:57 UTC),
            }])
        })
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
                    url: "https://example.invalid/paper.jar".to_owned(),
                    checksum: Checksum::sha256(&"0".repeat(64)).unwrap(),
                    size: 8,
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
            std::fs::write(dest, b"fake jar").unwrap();
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
            let bin = PathBuf::from(env!("CARGO_BIN_EXE_app-fake-mc"));
            Ok(JavaRuntime {
                major,
                home: bin.parent().unwrap().to_path_buf(),
                java_bin: bin,
                version: "21.0.5".to_owned(),
            })
        })
    }
}

/// Mojang giả: chỉ biết Notch.
struct FakeDirectory;

impl PlayerDirectory for FakeDirectory {
    fn profile<'a>(&'a self, name: &'a PlayerName) -> BoxFut<'a, Result<PlayerProfile, AppError>> {
        Box::pin(async move {
            if name.as_str().eq_ignore_ascii_case("notch") {
                Ok(PlayerProfile {
                    uuid: NOTCH_UUID.to_owned(),
                    name: "Notch".to_owned(),
                })
            } else {
                Err(AppError::new(ErrorCode::PlayerNotFound, "không có"))
            }
        })
    }

    fn skin<'a>(&'a self, _: &'a str) -> BoxFut<'a, Result<Vec<u8>, AppError>> {
        Box::pin(async { Err(AppError::new(ErrorCode::NotFound, "không có skin")) })
    }
}

fn test_app() -> (tempfile::TempDir, Arc<App>) {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    dirs.ensure().unwrap();
    let deps = AppDeps {
        catalog: Arc::new(FakeCatalog),
        java: Arc::new(FakeJava),
        port_free: Arc::new(|_| true),
        free_space: Arc::new(|_| Some(100 * 1024 * 1024 * 1024)),
        total_memory_mb: 16 * 1024,
        supervisor: SupervisorConfig {
            stop_timeout: Duration::from_millis(500),
            ..SupervisorConfig::default()
        },
        save_flush_timeout: Duration::from_secs(60),
        metrics: MetricsConfig::default(),
    };
    let app = App::with_deps(dirs, deps)
        .unwrap()
        .with_players(Arc::new(FakeDirectory));
    (tmp, Arc::new(app))
}

async fn create(app: &Arc<App>) -> ServerId {
    let req = CreateRequest {
        name: ServerName::parse("Người chơi").unwrap(),
        loader: LoaderKind::Paper,
        minecraft: McVersion::parse("1.21.4").unwrap(),
        loader_version: None,
        memory_mb: None,
        port: None,
    };
    let created = app.create(req).unwrap();
    let mut rx = app.jobs().subscribe(created.job_id).unwrap();
    let state = tokio::time::timeout(WAIT, rx.wait_for(|s| s.state != JobState::Running))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.state, JobState::Succeeded);
    app.accept_eula(created.server_id).unwrap();
    created.server_id
}

fn name(raw: &str) -> PlayerName {
    PlayerName::parse(raw).unwrap()
}

fn action(action: PlayerAction) -> PlayerRequest {
    PlayerRequest::Action(action)
}

fn read_json(app: &App, id: ServerId, file: &str) -> serde_json::Value {
    let path = app.dirs().servers().join(id.to_string()).join(file);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn set_property(app: &App, id: ServerId, key: &str, value: &str) {
    let mut props = app.read_properties(id).unwrap();
    props.retain(|(k, _)| k != key);
    props.push((key.to_owned(), value.to_owned()));
    app.write_properties(id, &props).unwrap();
}

/// Chờ tới khi `check` đúng, kiểm tra lại mỗi 20 ms.
async fn eventually(what: &str, mut check: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + WAIT;
    while !check() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "hết giờ chờ: {what}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn console_has(app: &App, id: ServerId, text: &str) -> bool {
    let handle = app.handle(id).unwrap();
    handle
        .console()
        .after(None)
        .iter()
        .any(|line| line.text.contains(text))
}

#[tokio::test]
async fn whitelist_while_stopped_uses_uuid_of_online_mode() {
    let (_tmp, app) = test_app();
    let id = create(&app).await;

    // online-mode (mặc định): UUID và tên đúng hoa thường lấy từ Mojang.
    let done = app
        .player_action(id, &name("notch"), action(PlayerAction::WhitelistAdd))
        .await
        .unwrap();
    assert_eq!(done, ActionDone::Saved);
    let list = read_json(&app, id, "server/whitelist.json");
    assert_eq!(list[0]["uuid"], NOTCH_UUID);
    assert_eq!(list[0]["name"], "Notch");
    let err = app
        .player_action(id, &name("NoSuchPlayer"), action(PlayerAction::Op))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::PlayerNotFound);

    // offline-mode: UUID tính từ tên, không hỏi Mojang.
    set_property(&app, id, "online-mode", "false");
    app.player_action(id, &name("Steve"), action(PlayerAction::Op))
        .await
        .unwrap();
    let ops = read_json(&app, id, "server/ops.json");
    assert_eq!(ops[0]["uuid"], "5627dd98-e6be-3c21-b8a8-e92344183641");
    assert_eq!(ops[0]["level"], 4);

    let ban = PlayerAction::Ban {
        reason: String::new(),
    };
    app.player_action(id, &name("Steve"), action(ban))
        .await
        .unwrap();
    let bans = read_json(&app, id, "server/banned-players.json");
    assert_eq!(bans[0]["reason"], "Banned by an operator.");
    assert_eq!(bans[0]["expires"], "forever");

    let view = app.players(id).unwrap();
    let steve = view
        .players
        .iter()
        .find(|p| p.name.as_str() == "Steve")
        .unwrap();
    assert!(steve.op && steve.ban_reason.is_some() && !steve.whitelisted);

    app.player_action(id, &name("STEVE"), action(PlayerAction::Pardon))
        .await
        .unwrap();
    app.player_action(id, &name("Notch"), action(PlayerAction::WhitelistRemove))
        .await
        .unwrap();
    assert_eq!(
        read_json(&app, id, "server/banned-players.json"),
        serde_json::json!([])
    );
    assert_eq!(
        read_json(&app, id, "server/whitelist.json"),
        serde_json::json!([])
    );
}

#[tokio::test]
async fn live_actions_need_a_running_server() {
    let (_tmp, app) = test_app();
    let id = create(&app).await;
    for request in [
        action(PlayerAction::Kick {
            reason: String::new(),
        }),
        action(PlayerAction::Freeze),
        PlayerRequest::TeleportToSpawn,
    ] {
        let err = app
            .player_action(id, &name("Steve"), request)
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::ServerNotRunning);
    }
}

#[tokio::test]
async fn running_server_tracks_online_players_and_freezes() {
    let (_tmp, app) = test_app();
    let id = create(&app).await;
    let mut changes = app.subscribe_players();
    app.start(id).await.unwrap();
    eventually("server chạy", || {
        app.get(id).unwrap().state == ServerState::Running
    })
    .await;

    app.send_command(id, "fake-join Steve").await.unwrap();
    eventually("Steve online", || {
        app.online_players(id) == vec![name("Steve")]
    })
    .await;
    assert_eq!(changes.recv().await.unwrap(), id);
    let view = app.players(id).unwrap();
    assert!(view.running);
    assert!(
        view.players
            .first()
            .is_some_and(|p| p.online && p.name.as_str() == "Steve")
    );

    let done = app
        .player_action(id, &name("Steve"), action(PlayerAction::Freeze))
        .await
        .unwrap();
    assert_eq!(done, ActionDone::Sent);
    // 1.21.4 (sau 1.21.2): thuộc tính không còn tiền tố `generic.`, có cả sức nhảy.
    eventually("lệnh đóng băng", || {
        console_has(&app, id, "attribute Steve movement_speed base set 0")
            && console_has(&app, id, "attribute Steve jump_strength base set 0")
    })
    .await;
    assert_eq!(
        read_json(&app, id, "frozen.json"),
        serde_json::json!(["Steve"])
    );
    assert!(app.players(id).unwrap().players[0].frozen);

    app.player_action(id, &name("Steve"), action(PlayerAction::Unfreeze))
        .await
        .unwrap();
    eventually("bỏ đóng băng", || {
        console_has(&app, id, "attribute Steve movement_speed base set 0.1")
    })
    .await;
    assert_eq!(read_json(&app, id, "frozen.json"), serde_json::json!([]));

    let kick = PlayerAction::Kick {
        reason: "nghỉ thôi".to_owned(),
    };
    app.player_action(id, &name("Steve"), action(kick))
        .await
        .unwrap();
    eventually("lệnh kick", || {
        console_has(&app, id, "kick Steve nghỉ thôi")
    })
    .await;

    app.send_command(id, "fake-leave Steve").await.unwrap();
    eventually("Steve rời", || app.online_players(id).is_empty()).await;
    app.stop(id).await.unwrap();
    eventually("server tắt", || {
        app.get(id).unwrap().state == ServerState::Stopped
    })
    .await;
}
