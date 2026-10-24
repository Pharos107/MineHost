use mh_core::instance::GameSection;
use mh_core::{GamePort, LoaderKind, McVersion, MemoryLimits, ServerName};
use time::macros::datetime;

use super::*;

fn store() -> (tempfile::TempDir, ServerStore) {
    let tmp = tempfile::tempdir().unwrap();
    let store = ServerStore::new(tmp.path().join("servers"), tmp.path().join("trash"));
    (tmp, store)
}

fn spec(id: ServerId) -> InstanceSpec {
    let game = GameSection::new(
        LoaderKind::Paper,
        McVersion::parse("1.21.4").unwrap(),
        Some(232),
        21,
    );
    InstanceSpec::new(
        id,
        ServerName::parse("Server của tôi").unwrap(),
        datetime!(2026-10-23 10:00 UTC),
        game,
        MemoryLimits::new(1024, 2048).unwrap(),
        GamePort::DEFAULT,
    )
}

fn create(store: &ServerStore) -> InstanceSpec {
    let spec = spec(ServerId::generate());
    store.create_dir(spec.id).unwrap();
    store.save(&spec).unwrap();
    spec
}

#[test]
fn missing_servers_dir_lists_nothing() {
    let (_tmp, store) = store();
    assert!(store.list().unwrap().is_empty());
}

#[test]
fn create_save_load_round_trip() {
    let (_tmp, store) = store();
    let spec = create(&store);
    let paths = store.paths(spec.id);
    assert!(paths.server_dir.is_dir());
    assert!(paths.logs_dir.is_dir());
    assert_eq!(store.load(spec.id).unwrap(), spec);
}

#[test]
fn create_dir_twice_is_an_error() {
    let (_tmp, store) = store();
    let id = ServerId::generate();
    store.create_dir(id).unwrap();
    assert!(matches!(
        store.create_dir(id),
        Err(StoreError::AlreadyExists(_))
    ));
}

#[test]
fn list_reports_good_and_broken_servers() {
    let (_tmp, store) = store();
    let good = create(&store);

    let missing = ServerId::generate();
    store.create_dir(missing).unwrap();

    let corrupt = ServerId::generate();
    store.create_dir(corrupt).unwrap();
    fs::write(store.paths(corrupt).instance_file, "schema = 1\nid = [").unwrap();

    let newer = ServerId::generate();
    store.create_dir(newer).unwrap();
    fs::write(store.paths(newer).instance_file, "schema = 9\n").unwrap();

    // Thư mục chép tay: tên mới nhưng `id` bên trong là của server khác.
    let copied = ServerId::generate();
    store.create_dir(copied).unwrap();
    let text = good.to_toml().unwrap();
    fs::write(store.paths(copied).instance_file, text).unwrap();

    let servers = store.paths(good.id).root.parent().unwrap().to_path_buf();
    fs::create_dir(servers.join("not-a-server")).unwrap();
    fs::write(
        servers.join(ServerId::generate().to_string()),
        "file, not dir",
    )
    .unwrap();

    let listed = store.list().unwrap();
    let ids: Vec<ServerId> = listed.iter().map(|l| l.id).collect();
    // ULID sinh trong cùng một mili giây không có thứ tự cố định, nên chỉ kiểm tra đã sắp xếp.
    let mut sorted = vec![good.id, missing, corrupt, newer, copied];
    sorted.sort();
    assert_eq!(ids, sorted);
    let spec_of = |id| &listed.iter().find(|l| l.id == id).unwrap().spec;
    assert_eq!(spec_of(good.id).as_ref().unwrap(), &good);
    assert_eq!(spec_of(missing), &Err(BrokenReason::MissingInstance));
    assert!(matches!(
        spec_of(corrupt),
        Err(BrokenReason::Invalid { .. })
    ));
    assert_eq!(spec_of(newer), &Err(BrokenReason::NewerSchema { found: 9 }));
    assert!(matches!(
        spec_of(copied),
        Err(BrokenReason::IdMismatch { .. })
    ));
}

#[test]
fn load_reports_not_found_and_broken() {
    let (_tmp, store) = store();
    let err = store.load(ServerId::generate()).unwrap_err();
    assert_eq!(err.code(), mh_core::ErrorCode::NotFound);

    let id = ServerId::generate();
    store.create_dir(id).unwrap();
    let err = store.load(id).unwrap_err();
    assert_eq!(err.code(), mh_core::ErrorCode::ConfigInvalid);
}

#[test]
fn save_keeps_unknown_keys() {
    let (_tmp, store) = store();
    let spec = create(&store);
    let file = store.paths(spec.id).instance_file;
    let with_extra = format!("future = \"x\"\n{}", fs::read_to_string(&file).unwrap());
    fs::write(&file, with_extra).unwrap();

    let mut loaded = store.load(spec.id).unwrap();
    loaded.policy.autostart = true;
    store.save(&loaded).unwrap();
    let text = fs::read_to_string(&file).unwrap();
    assert!(text.contains("future = \"x\""), "{text}");
    assert!(store.load(spec.id).unwrap().policy.autostart);
}

#[test]
fn trash_moves_every_file() {
    let (tmp, store) = store();
    let spec = create(&store);
    let paths = store.paths(spec.id);
    fs::create_dir_all(paths.server_dir.join("world/region")).unwrap();
    fs::write(paths.server_dir.join("world/region/r.0.0.mca"), b"chunk").unwrap();
    fs::write(paths.logs_dir.join("console-2026-10-23.log"), b"log").unwrap();

    let dest = store
        .trash(spec.id, datetime!(2026-10-23 17:05:09 +7))
        .unwrap();
    assert_eq!(
        dest,
        tmp.path()
            .join("trash")
            .join(format!("{}-20261023-100509", spec.id))
    );
    assert!(!paths.root.exists());
    assert_eq!(
        fs::read(dest.join("server/world/region/r.0.0.mca")).unwrap(),
        b"chunk"
    );
    assert_eq!(
        fs::read(dest.join("logs/console-2026-10-23.log")).unwrap(),
        b"log"
    );
    assert!(dest.join("instance.toml").is_file());
    assert!(store.list().unwrap().is_empty());
}

#[test]
fn trash_missing_server_is_not_found() {
    let (_tmp, store) = store();
    let err = store.trash(ServerId::generate(), datetime!(2026-10-23 10:00 UTC));
    assert!(matches!(err, Err(StoreError::NotFound(_))));
}

#[test]
fn running_marker_shows_in_list() {
    let (_tmp, store) = store();
    let spec = create(&store);
    assert!(!store.list().unwrap()[0].marked_running);

    store.mark_running(spec.id).unwrap();
    assert!(store.list().unwrap()[0].marked_running);
    store.clear_running(spec.id).unwrap();
    store.clear_running(spec.id).unwrap();
    assert!(!store.list().unwrap()[0].marked_running);
}

#[test]
fn remove_unfinished_is_idempotent() {
    let (_tmp, store) = store();
    let id = ServerId::generate();
    store.create_dir(id).unwrap();
    store.remove_unfinished(id).unwrap();
    store.remove_unfinished(id).unwrap();
    assert!(!store.paths(id).root.exists());
}

#[test]
fn broken_reason_serializes_with_kind() {
    let json = serde_json::to_value(BrokenReason::NewerSchema { found: 2 }).unwrap();
    assert_eq!(
        json,
        serde_json::json!({ "kind": "newer_schema", "found": 2 })
    );
}
