//! File thêm vào archive ngoài cây thư mục (bản chụp `content.toml`, spec P3 mục 4.2).

#![allow(clippy::unwrap_used, clippy::panic)]

use std::fs;

use mh_backup::{DEFAULT_EXCLUDE, Extra, create_with, restore};
use tokio_util::sync::CancellationToken;

#[test]
fn extras_are_restored_next_to_the_tree() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("server");
    fs::create_dir_all(src.join("world")).unwrap();
    fs::write(src.join("world/level.dat"), b"level").unwrap();
    let archive = tmp.path().join("a.tar.zst");
    let extras = [Extra {
        path: ".minehost/content.toml",
        bytes: b"schema = 1\n",
    }];
    let info = create_with(
        &src,
        &archive,
        DEFAULT_EXCLUDE,
        &extras,
        |_, _| {},
        &CancellationToken::new(),
    )
    .unwrap();

    let out = tmp.path().join("out");
    restore(
        &archive,
        &info.sha256,
        &out,
        |_, _| {},
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(fs::read(out.join("world/level.dat")).unwrap(), b"level");
    assert_eq!(
        fs::read(out.join(".minehost/content.toml")).unwrap(),
        b"schema = 1\n"
    );
}

#[test]
fn extra_path_escaping_the_root_is_refused_and_leaves_no_part_file() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("server");
    fs::create_dir_all(&src).unwrap();
    let archive = tmp.path().join("a.tar.zst");
    for path in ["../x", "/abs", "a//b", "a\\b", "c:x", ""] {
        let extras = [Extra { path, bytes: b"x" }];
        let result = create_with(
            &src,
            &archive,
            DEFAULT_EXCLUDE,
            &extras,
            |_, _| {},
            &CancellationToken::new(),
        );
        assert!(result.is_err(), "{path}");
        assert!(!archive.exists(), "{path}");
        assert!(!tmp.path().join("a.tar.zst.part").exists(), "{path}");
    }
}
