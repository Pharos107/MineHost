//! Nén rồi giải nén, loại trừ thư mục, chặn đường dẫn, hash và hủy.

// Hàm trợ giúp ngoài `#[test]` không được clippy coi là code test.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Cursor;
use std::path::{Path, PathBuf};

use mh_backup::{BackupError, DEFAULT_EXCLUDE, create, restore};
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

const NOTE: &[u8] = "xin chào".as_bytes();

const KEPT: &[(&str, &[u8])] = &[
    ("empty.dat", b""),
    ("server.properties", "motd=xin chào\n".as_bytes()),
    ("cache.txt", b"keep"),
    ("logs-extra/keep.txt", b"keep-log"),
    ("plugins/logs/a.txt", b"plugin"),
    ("thế giới/ghi chú.txt", NOTE),
    ("world/level.dat", b"level"),
    ("world/region/r.0.0.mca", b""),
];

const EXCLUDED: &[(&str, &[u8])] = &[
    ("logs/latest.log", b"log"),
    ("crash-reports/crash.txt", b"crash"),
    ("cache/a.dat", b"cache"),
];

const EMPTY_DIR: &str = "thư mục trống";

#[derive(Debug, PartialEq, Eq)]
enum Kind {
    Dir,
    File(Vec<u8>),
}

#[derive(Clone, Copy)]
struct Crafted {
    name: &'static str,
    kind: tar::EntryType,
    data: &'static [u8],
    link: Option<&'static str>,
}

fn token() -> CancellationToken {
    CancellationToken::new()
}

fn write_tree(root: &Path) {
    for (rel, bytes) in KEPT.iter().chain(EXCLUDED.iter()) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    fs::create_dir_all(root.join(EMPTY_DIR)).unwrap();
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn sha256_file(path: &Path) -> String {
    sha256_hex(&fs::read(path).unwrap())
}

fn pack(src: &Path, dest: &Path) -> mh_backup::ArchiveInfo {
    create(src, dest, DEFAULT_EXCLUDE, |_, _| {}, &token()).unwrap()
}

/// Khớp `logs/` theo thành phần, không khớp `logs-extra` hay `plugins/logs`.
fn is_excluded_name(name: &str) -> bool {
    let parts: Vec<&str> = name
        .split(['/', '\\'])
        .filter(|part| !part.is_empty() && *part != ".")
        .collect();
    [["logs"], ["crash-reports"], ["cache"]]
        .into_iter()
        .any(|rule| {
            parts.len() >= rule.len() && parts.iter().zip(rule).all(|(got, want)| *got == want)
        })
}

fn snapshot(root: &Path) -> BTreeMap<String, Kind> {
    let mut out = BTreeMap::new();
    walk_snap(root, "", &mut out);
    out
}

fn walk_snap(dir: &Path, rel: &str, out: &mut BTreeMap<String, Kind>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap())
        .collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name().to_str().unwrap().to_owned();
        let child = if rel.is_empty() {
            name
        } else {
            format!("{rel}/{name}")
        };
        let meta = fs::symlink_metadata(entry.path()).unwrap();
        assert!(!meta.file_type().is_symlink(), "{}", child);
        if meta.is_dir() {
            out.insert(child.clone(), Kind::Dir);
            walk_snap(&entry.path(), &child, out);
        } else {
            out.insert(child, Kind::File(fs::read(entry.path()).unwrap()));
        }
    }
}

fn kept_snapshot(root: &Path) -> BTreeMap<String, Kind> {
    snapshot(root)
        .into_iter()
        .filter(|(name, _)| !is_excluded_name(name))
        .collect()
}

fn entry_names(archive: &Path) -> Vec<String> {
    let file = File::open(archive).unwrap();
    let decoder = zstd::stream::read::Decoder::new(file).unwrap();
    let mut tar = tar::Archive::new(decoder);
    tar.entries()
        .unwrap()
        .map(|entry| {
            entry
                .unwrap()
                .path()
                .unwrap()
                .components()
                .filter_map(|component| component.as_os_str().to_str().map(str::to_owned))
                .collect::<Vec<_>>()
                .join("/")
        })
        .filter(|name| !name.is_empty())
        .collect()
}

fn kept_bytes() -> u64 {
    KEPT.iter().map(|(_, data)| data.len() as u64).sum()
}

#[test]
fn round_trip_keeps_content_and_drops_excluded_dirs() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    write_tree(&src);
    let archive = tmp.path().join("world.tar.zst");
    let mut seen_total = None;
    let info = create(
        &src,
        &archive,
        DEFAULT_EXCLUDE,
        |done, total| {
            assert_eq!(total, kept_bytes());
            assert!(done <= total);
            seen_total = Some(total);
        },
        &token(),
    )
    .unwrap();

    assert_eq!(seen_total, Some(kept_bytes()));
    assert_eq!(info.size, fs::metadata(&archive).unwrap().len());
    assert_eq!(info.sha256, sha256_file(&archive));

    let names = entry_names(&archive);
    assert!(
        names.iter().all(|name| !is_excluded_name(name)),
        "{names:?}"
    );
    assert!(
        names.iter().any(|name| name.contains("ghi chú.txt")),
        "{names:?}"
    );
    assert!(
        names.iter().any(|name| name.contains("logs-extra")),
        "{names:?}"
    );
    assert!(
        names.iter().any(|name| name.ends_with("cache.txt")),
        "{names:?}"
    );

    let dest = tmp.path().join("out");
    restore(&archive, &info.sha256, &dest, |_, _| {}, &token()).unwrap();
    assert_eq!(snapshot(&dest), kept_snapshot(&src));
    assert!(snapshot(&dest).contains_key(EMPTY_DIR));
}

#[test]
fn two_runs_share_sha256() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    write_tree(&src);
    let first = tmp.path().join("a.tar.zst");
    let second = tmp.path().join("b.tar.zst");
    let left = pack(&src, &first);
    let right = pack(&src, &second);
    assert_eq!(left, right);
    assert_eq!(fs::read(first).unwrap(), fs::read(second).unwrap());
}

#[test]
fn wrong_sha256_does_not_extract() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    write_tree(&src);
    let archive = tmp.path().join("a.tar.zst");
    pack(&src, &archive);
    let dest = tmp.path().join("out");
    let err = restore(&archive, &"ab".repeat(32), &dest, |_, _| {}, &token()).unwrap_err();
    assert!(matches!(err, BackupError::Corrupt { .. }), "{err}");
    assert_eq!(err.code().as_str(), "BACKUP_CORRUPT");
    assert!(!dest.exists());
}

#[test]
fn cancel_during_create_removes_part_file() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(src.join("world")).unwrap();
    fs::write(src.join("world/level.dat"), vec![9u8; 8192]).unwrap();
    let dest = tmp.path().join("a.tar.zst");
    let part = tmp.path().join("a.tar.zst.part");
    let cancel = token();
    let flag = cancel.clone();
    let mut saw_bytes = false;
    let err = create(
        &src,
        &dest,
        DEFAULT_EXCLUDE,
        |done, _| {
            if done > 0 {
                saw_bytes = true;
                flag.cancel();
            }
        },
        &cancel,
    )
    .unwrap_err();
    assert!(saw_bytes);
    assert!(matches!(err, BackupError::Cancelled), "{err}");
    assert_eq!(err.code().as_str(), "JOB_CANCELLED");
    assert!(!part.exists());
    assert!(!dest.exists());
}

#[test]
fn restore_rejects_non_empty_dest() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    write_tree(&src);
    let archive = tmp.path().join("a.tar.zst");
    let info = pack(&src, &archive);
    let dest = tmp.path().join("out");
    fs::create_dir(&dest).unwrap();
    fs::write(dest.join("keep.txt"), b"old").unwrap();
    let err = restore(&archive, &info.sha256, &dest, |_, _| {}, &token()).unwrap_err();
    assert!(matches!(err, BackupError::DestNotEmpty(_)), "{err}");
    assert_eq!(fs::read(dest.join("keep.txt")).unwrap(), b"old");
}

fn crafted(
    name: &'static str,
    kind: tar::EntryType,
    data: &'static [u8],
    link: Option<&'static str>,
) -> Crafted {
    Crafted {
        name,
        kind,
        data,
        link,
    }
}

fn is_traversal(entry: &Crafted) -> bool {
    entry.name.starts_with('/')
        || entry.name.starts_with('\\')
        || entry.name.split(['/', '\\']).any(|part| part == "..")
}

/// `append_data` từ chối `..` và đường dẫn tuyệt đối. Ghi thẳng byte tên vào header.
fn raw_header(entry: &Crafted) -> tar::Header {
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(entry.kind);
    header.set_size(entry.data.len() as u64);
    header.set_mode(0o644);
    header.set_mtime(0);
    header.set_uid(0);
    header.set_gid(0);
    let old = header.as_old_mut();
    let name = entry.name.as_bytes();
    assert!(name.len() < old.name.len());
    old.name[..name.len()].copy_from_slice(name);
    if let Some(link) = entry.link {
        let link = link.as_bytes();
        assert!(link.len() < old.linkname.len());
        old.linkname[..link.len()].copy_from_slice(link);
    }
    header.set_cksum();
    header
}

fn archive_bytes(entries: &[Crafted]) -> Vec<u8> {
    let mut raw = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut raw);
        for entry in entries {
            let header = raw_header(entry);
            builder.append(&header, entry.data).unwrap();
        }
        builder.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(raw), 3).unwrap()
}

fn list_tree(root: &Path) -> BTreeSet<PathBuf> {
    let mut out = BTreeSet::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let meta = fs::symlink_metadata(&path).unwrap();
            assert!(!meta.file_type().is_symlink(), "symlink {}", path.display());
            if meta.is_dir() {
                stack.push(path.clone());
            }
            out.insert(path);
        }
    }
    out
}

/// File archive nằm cạnh `dest` từ trước. Chỉ đường dẫn mới phải nằm trong `dest`.
fn assert_new_paths_stay_in(parent: &Path, dest: &Path, before: &BTreeSet<PathBuf>) {
    let root = dest.canonicalize().unwrap();
    for path in list_tree(parent) {
        if before.contains(&path) {
            continue;
        }
        assert!(
            path.canonicalize().unwrap().starts_with(&root),
            "thoát ra ngoài: {}",
            path.display()
        );
    }
}

fn assert_missing(path: &Path) {
    assert!(
        fs::symlink_metadata(path).is_err(),
        "không được tạo {}",
        path.display()
    );
}

fn assert_stays_inside(entries: &[Crafted]) {
    let tmp = tempfile::tempdir().unwrap();
    let bytes = archive_bytes(entries);
    let archive = tmp.path().join("evil.tar.zst");
    fs::write(&archive, &bytes).unwrap();
    let dest = tmp.path().join("out");
    let before = list_tree(tmp.path());
    let abs = Path::new("/abs");
    let abs_existed = fs::symlink_metadata(abs).is_ok();
    let result = restore(&archive, &sha256_hex(&bytes), &dest, |_, _| {}, &token());
    if entries.iter().any(is_traversal) {
        let err = result.unwrap_err();
        assert!(matches!(err, BackupError::PathOutsideRoot(_)), "{err}");
        assert_eq!(err.code().as_str(), "PATH_OUTSIDE_ROOT");
    } else {
        result.unwrap();
    }
    assert_new_paths_stay_in(tmp.path(), &dest, &before);
    if !abs_existed {
        assert_missing(abs);
    }
    assert_missing(&dest.join("abs"));
    assert_missing(&dest.join("evil"));
    assert_missing(&tmp.path().join("evil"));
    for entry in entries {
        if entry.name != "ok.txt" && !is_traversal(entry) {
            assert_missing(&dest.join(entry.name));
        }
    }
    if entries.iter().any(|entry| entry.name == "ok.txt") {
        assert_eq!(fs::read(dest.join("ok.txt")).unwrap(), b"ok");
    }
}

#[test]
fn crafted_entries_do_not_escape_dest() {
    let each = [
        crafted("../evil", tar::EntryType::Regular, b"pwned", None),
        crafted("/abs", tar::EntryType::Regular, b"abs", None),
        crafted("link", tar::EntryType::Symlink, b"", Some("../evil")),
        crafted("hard", tar::EntryType::Link, b"", Some("../evil")),
        crafted("fifo", tar::EntryType::Fifo, b"", None),
    ];
    for entry in &each {
        assert_stays_inside(std::slice::from_ref(entry));
    }
    let mut all = each.to_vec();
    all.push(crafted("ok.txt", tar::EntryType::Regular, b"ok", None));
    assert_stays_inside(&all);
}

#[cfg(unix)]
#[test]
fn create_does_not_follow_symlink() {
    use std::os::unix::fs::symlink;

    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("real.txt"), b"real").unwrap();
    symlink("real.txt", src.join("link")).unwrap();
    let archive = tmp.path().join("a.tar.zst");
    pack(&src, &archive);
    let names = entry_names(&archive);
    assert!(names.iter().all(|name| !name.contains("link")), "{names:?}");
    assert!(
        names.iter().any(|name| name.contains("real.txt")),
        "{names:?}"
    );
}

#[cfg(unix)]
#[test]
fn restore_keeps_unix_exec_bit() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    let script = src.join("run.sh");
    fs::write(&script, b"#!/bin/sh\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let archive = tmp.path().join("a.tar.zst");
    let info = pack(&src, &archive);
    let dest = tmp.path().join("out");
    restore(&archive, &info.sha256, &dest, |_, _| {}, &token()).unwrap();
    let mode = fs::metadata(dest.join("run.sh"))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o755);
}
