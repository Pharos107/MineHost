//! Đưa byte ngẫu nhiên vào bộ giải nén tar (không kiểm tra sha256).

#![no_main]

use std::fs;
use std::io::Cursor;
use std::path::Path;

use libfuzzer_sys::fuzz_target;
use tokio_util::sync::CancellationToken;

fuzz_target!(|data: &[u8]| {
    let Ok(tmp) = tempfile::tempdir() else {
        return;
    };
    let dest = tmp.path().join("out");
    // Hash không thuộc bề mặt tấn công đường dẫn. `unpack_tar` là chỗ ghi file.
    let _ = mh_backup::unpack_tar(
        Cursor::new(data),
        &dest,
        |_, _| {},
        &CancellationToken::new(),
    );
    assert_contained(tmp.path(), &dest);
});

/// Mọi file tạo ra phải nằm trong `dest`. Symlink bị cấm vì nó có thể trỏ ra ngoài.
fn assert_contained(parent: &Path, dest: &Path) {
    let Ok(root) = dest.canonicalize() else {
        assert_empty(parent);
        return;
    };
    let mut stack = vec![parent.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in read_entries(&dir) {
            let entry = unwrap_entry(&dir, entry);
            let path = entry.path();
            let meta = match fs::symlink_metadata(&path) {
                Ok(meta) => meta,
                Err(err) => panic!("không xem được {}: {err}", path.display()),
            };
            if meta.file_type().is_symlink() {
                panic!("symlink {}", path.display());
            }
            let canon = match path.canonicalize() {
                Ok(canon) => canon,
                Err(err) => panic!("không chuẩn hóa {}: {err}", path.display()),
            };
            if !canon.starts_with(&root) {
                panic!("thoát ra ngoài: {}", canon.display());
            }
            if meta.is_dir() {
                stack.push(path);
            }
        }
    }
}

fn assert_empty(dir: &Path) {
    for entry in read_entries(dir) {
        let entry = unwrap_entry(dir, entry);
        panic!(
            "có file khi thư mục đích không tồn tại: {}",
            entry.path().display()
        );
    }
}

fn read_entries(dir: &Path) -> fs::ReadDir {
    match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => panic!("không đọc được {}: {err}", dir.display()),
    }
}

fn unwrap_entry(dir: &Path, entry: std::io::Result<fs::DirEntry>) -> fs::DirEntry {
    match entry {
        Ok(entry) => entry,
        Err(err) => panic!("không đọc entry trong {}: {err}", dir.display()),
    }
}
