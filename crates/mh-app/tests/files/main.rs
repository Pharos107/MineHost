//! Dịch vụ tệp: đường dẫn, etag, thùng rác, upload và `server.jar` khi server đang chạy.

#![allow(clippy::unwrap_used)]

mod support;

use std::io::Cursor;

use mh_app::{FileKind, TEXT_LIMIT};
use mh_core::{ErrorCode, ServerState};
use support::{Env, assert_trash, create, wait_state};

#[tokio::test]
async fn parent_and_absolute_paths_are_outside() {
    let env = Env::new();
    let app = env.app();
    let spec = create(&app).await;
    let abs = env.tmp.path().join("outside.txt");
    std::fs::write(&abs, b"secret").unwrap();
    let abs = abs.to_str().unwrap();

    for bad in ["..", "../x", "a/../../x", r"a\..\..\x", abs] {
        assert_eq!(
            app.list_dir(spec.id, bad).await.unwrap_err().code,
            ErrorCode::PathOutsideRoot,
            "{bad}"
        );
        assert_eq!(
            app.read_text(spec.id, bad).await.unwrap_err().code,
            ErrorCode::PathOutsideRoot,
            "{bad}"
        );
        assert_eq!(
            app.delete(spec.id, bad).await.unwrap_err().code,
            ErrorCode::PathOutsideRoot,
            "{bad}"
        );
    }
    assert_eq!(
        std::fs::read(env.tmp.path().join("outside.txt")).unwrap(),
        b"secret"
    );
    assert!(!env.server(&spec).parent().unwrap().join("x").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn symlink_outside_is_rejected_and_inside_is_listed() {
    let env = Env::new();
    let app = env.app();
    let spec = create(&app).await;
    let server = env.server(&spec);
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret"), b"nope").unwrap();
    std::os::unix::fs::symlink(outside.path().join("secret"), server.join("leak")).unwrap();
    std::os::unix::fs::symlink(outside.path(), server.join("leakdir")).unwrap();
    std::fs::write(server.join("note.txt"), b"hi").unwrap();
    std::os::unix::fs::symlink(server.join("note.txt"), server.join("alias.txt")).unwrap();

    assert_eq!(
        app.read_text(spec.id, "leak").await.unwrap_err().code,
        ErrorCode::PathOutsideRoot
    );
    assert_eq!(
        app.list_dir(spec.id, "leakdir").await.unwrap_err().code,
        ErrorCode::PathOutsideRoot
    );

    let entries = app.list_dir(spec.id, "").await.unwrap();
    assert!(
        entries
            .iter()
            .all(|e| e.name != "leak" && e.name != "leakdir")
    );
    let alias = entries.iter().find(|e| e.name == "alias.txt").unwrap();
    assert_eq!(alias.kind, FileKind::File);
    assert_eq!(alias.size, Some(2));
    assert_eq!(
        app.read_text(spec.id, "alias.txt").await.unwrap().text,
        "hi"
    );
}

#[tokio::test]
async fn binary_and_oversize_text_are_rejected() {
    let env = Env::new();
    let app = env.app();
    let spec = create(&app).await;
    let server = env.server(&spec);
    std::fs::write(server.join("bin.dat"), [0xff, 0xfe]).unwrap();
    std::fs::write(server.join("nul.txt"), b"a\0b").unwrap();
    std::fs::write(server.join("big.txt"), vec![b'a'; TEXT_LIMIT as usize + 1]).unwrap();

    assert_eq!(
        app.read_text(spec.id, "bin.dat").await.unwrap_err().code,
        ErrorCode::FileNotText
    );
    assert_eq!(
        app.read_text(spec.id, "nul.txt").await.unwrap_err().code,
        ErrorCode::FileNotText
    );
    assert_eq!(
        app.read_text(spec.id, "big.txt").await.unwrap_err().code,
        ErrorCode::FileTooLarge
    );

    let huge = "a".repeat(TEXT_LIMIT as usize + 1);
    assert_eq!(
        app.write_text(spec.id, "new.txt", &huge, None)
            .await
            .unwrap_err()
            .code,
        ErrorCode::FileTooLarge
    );
    assert!(!server.join("new.txt").exists());
    assert_eq!(
        app.write_text(spec.id, "nul2.txt", "a\0b", None)
            .await
            .unwrap_err()
            .code,
        ErrorCode::FileNotText
    );
}

#[tokio::test]
async fn write_text_requires_current_etag() {
    let env = Env::new();
    let app = env.app();
    let spec = create(&app).await;
    let server = env.server(&spec);

    assert_eq!(
        app.write_text(spec.id, "fresh.txt", "hi", Some("nope"))
            .await
            .unwrap_err()
            .code,
        ErrorCode::FileChanged
    );
    assert!(!server.join("fresh.txt").exists());

    let etag = app
        .write_text(spec.id, "notes.txt", "hello", None)
        .await
        .unwrap();
    let read = app.read_text(spec.id, "notes.txt").await.unwrap();
    assert_eq!(read.text, "hello");
    assert_eq!(read.etag, etag);
    assert_eq!(read.size, 5);

    assert_eq!(
        app.write_text(spec.id, "notes.txt", "changed", Some("deadbeef"))
            .await
            .unwrap_err()
            .code,
        ErrorCode::FileChanged
    );
    assert_eq!(
        app.write_text(spec.id, "notes.txt", "changed", None)
            .await
            .unwrap_err()
            .code,
        ErrorCode::FileChanged
    );
    assert_eq!(
        std::fs::read_to_string(server.join("notes.txt")).unwrap(),
        "hello"
    );

    let new_etag = app
        .write_text(spec.id, "notes.txt", "changed", Some(&etag))
        .await
        .unwrap();
    assert_ne!(new_etag, etag);
    let read = app.read_text(spec.id, "notes.txt").await.unwrap();
    assert_eq!(read.text, "changed");
    assert_eq!(read.etag, new_etag);
}

#[tokio::test]
async fn delete_moves_file_and_dir_into_trash() {
    let env = Env::new();
    let app = env.app();
    let spec = create(&app).await;
    let server = env.server(&spec);
    app.write_text(spec.id, "keep.txt", "stay", None)
        .await
        .unwrap();
    app.create_dir(spec.id, "world").await.unwrap();
    app.write_text(spec.id, "world/level.dat", "world-data", None)
        .await
        .unwrap();

    let trash_files = env
        .tmp
        .path()
        .join("trash")
        .join(spec.id.to_string())
        .join("files");
    let file_dest = app.delete(spec.id, "keep.txt").await.unwrap();
    assert_eq!(std::fs::read_to_string(&file_dest).unwrap(), "stay");
    assert!(!server.join("keep.txt").exists());
    assert_trash(&file_dest, &trash_files, "keep.txt");

    let dir_dest = app.delete(spec.id, "world").await.unwrap();
    assert_eq!(
        std::fs::read_to_string(dir_dest.join("level.dat")).unwrap(),
        "world-data"
    );
    assert!(!server.join("world").exists());
    assert_trash(&dir_dest, &trash_files, "world");

    for root in ["", ".", "kept/.."] {
        assert_eq!(
            app.delete(spec.id, root).await.unwrap_err().code,
            ErrorCode::InvalidInput,
            "{root}"
        );
    }
    assert!(server.is_dir());
    assert!(server.join("server.jar").is_file());
}

#[tokio::test]
async fn list_dir_puts_directories_first_ignoring_case() {
    let env = Env::new();
    let app = env.app();
    let spec = create(&app).await;
    app.create_dir(spec.id, "box").await.unwrap();
    app.create_dir(spec.id, "box/Mods").await.unwrap();
    app.create_dir(spec.id, "box/config").await.unwrap();
    app.write_text(spec.id, "box/Banner.txt", "b", None)
        .await
        .unwrap();
    app.write_text(spec.id, "box/avocado.txt", "a", None)
        .await
        .unwrap();

    let err = app.create_dir(spec.id, "box").await.unwrap_err();
    assert_eq!(err.code, ErrorCode::FileExists);

    let entries = app.list_dir(spec.id, "box").await.unwrap();
    assert!(
        entries
            .iter()
            .take(2)
            .all(|e| e.kind == FileKind::Dir && e.size.is_none())
    );
    assert!(entries.iter().skip(2).all(|e| e.kind == FileKind::File));
    let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["config", "Mods", "avocado.txt", "Banner.txt"]);
    assert_eq!(entries[2].size, Some(1));
    assert!(entries[2].modified.is_some());

    app.rename(spec.id, "box/avocado.txt", "box/Mods/moved.txt")
        .await
        .unwrap();
    assert_eq!(
        app.read_text(spec.id, "box/Mods/moved.txt")
            .await
            .unwrap()
            .text,
        "a"
    );
    app.write_text(spec.id, "box/other.txt", "o", None)
        .await
        .unwrap();
    assert_eq!(
        app.rename(spec.id, "box/Banner.txt", "box/other.txt")
            .await
            .unwrap_err()
            .code,
        ErrorCode::FileExists
    );
    assert_eq!(
        app.read_text(spec.id, "box/Banner.txt").await.unwrap().text,
        "b"
    );
}

#[tokio::test]
async fn save_upload_over_limit_leaves_no_part_file() {
    let env = Env::new();
    let app = env.app();
    let spec = create(&app).await;
    let server = env.server(&spec);

    let err = app
        .save_upload(spec.id, "", "big.bin", Cursor::new(vec![1u8; 32]), 8)
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::FileTooLarge);
    assert!(!server.join("big.bin").exists());
    assert!(!server.join("big.bin.part").exists());
    assert!(
        std::fs::read_dir(&server)
            .unwrap()
            .all(|e| { !e.unwrap().file_name().to_string_lossy().ends_with(".part") })
    );

    app.save_upload(spec.id, "", "ok.bin", Cursor::new(b"hello".to_vec()), 8)
        .await
        .unwrap();
    assert_eq!(std::fs::read(server.join("ok.bin")).unwrap(), b"hello");
    assert!(!server.join("ok.bin.part").exists());

    assert_eq!(
        app.save_upload(spec.id, "", "ok.bin", Cursor::new(b"x".to_vec()), 8)
            .await
            .unwrap_err()
            .code,
        ErrorCode::FileExists
    );
    for name in ["", ".", "..", "a/b", r"a\b"] {
        assert_eq!(
            app.save_upload(spec.id, "", name, Cursor::new(Vec::<u8>::new()), 8)
                .await
                .unwrap_err()
                .code,
            ErrorCode::InvalidInput,
            "{name}"
        );
    }
}

#[tokio::test]
async fn running_server_blocks_server_jar_edits() {
    let env = Env::new();
    let app = env.app();
    let spec = create(&app).await;
    let server = env.server(&spec);
    app.accept_eula(spec.id).unwrap();
    app.start(spec.id).await.unwrap();
    wait_state(&app, &spec, ServerState::Running).await;

    app.create_dir(spec.id, "plugins").await.unwrap();
    app.write_text(spec.id, "plugins/server.jar", "plugin", None)
        .await
        .unwrap();
    app.write_text(spec.id, "notes.txt", "n", None)
        .await
        .unwrap();
    app.delete(spec.id, "notes.txt").await.unwrap();

    assert_eq!(
        app.delete(spec.id, "server.jar").await.unwrap_err().code,
        ErrorCode::ServerBusy
    );
    assert_eq!(
        app.write_text(spec.id, "./server.jar", "x", None)
            .await
            .unwrap_err()
            .code,
        ErrorCode::ServerBusy
    );
    assert_eq!(
        app.rename(spec.id, "server.jar", "other.jar")
            .await
            .unwrap_err()
            .code,
        ErrorCode::ServerBusy
    );
    assert_eq!(
        app.save_upload(spec.id, "", "server.jar", Cursor::new(b"x".to_vec()), 8)
            .await
            .unwrap_err()
            .code,
        ErrorCode::ServerBusy
    );
    assert_eq!(
        std::fs::read(server.join("server.jar")).unwrap(),
        b"fake jar"
    );
    assert!(!server.join("server.jar.part").exists());

    app.stop(spec.id).await.unwrap();
    wait_state(&app, &spec, ServerState::Stopped).await;
    let dest = app.delete(spec.id, "server.jar").await.unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), b"fake jar");
    assert!(!server.join("server.jar").exists());
}
