//! Đưa chuỗi ngẫu nhiên vào `SafeRoot::join` (spec P2, mục 9).
//!
//! Thư mục gốc có sẵn một thư mục con, một file và một symlink trỏ ra ngoài, để fuzzer gặp được
//! cả nhánh "đường dẫn đã tồn tại" lẫn nhánh symlink. Mọi đường dẫn được nhận phải nằm trong gốc,
//! cả theo chữ lẫn sau khi chuẩn hóa.

#![no_main]

use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use mh_platform::SafeRoot;

struct Fixture {
    /// Giữ thư mục tạm sống suốt phiên fuzz.
    _tmp: tempfile::TempDir,
    root: SafeRoot,
    canonical: PathBuf,
}

fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let tmp = tempfile::tempdir().expect("tạo thư mục tạm");
        let root_dir = tmp.path().join("root");
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(root_dir.join("world/region")).expect("tạo thư mục con");
        std::fs::create_dir_all(&outside).expect("tạo thư mục ngoài");
        std::fs::write(root_dir.join("server.properties"), "motd=fuzz\n").expect("tạo file");
        std::fs::write(outside.join("secret.txt"), "bí mật").expect("tạo file ngoài");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, root_dir.join("escape")).expect("tạo symlink");
        let root = SafeRoot::new(&root_dir).expect("SafeRoot");
        let canonical = root.path().to_path_buf();
        Fixture {
            _tmp: tmp,
            root,
            canonical,
        }
    })
}

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };
    let fx = fixture();
    let Ok(joined) = fx.root.join(input) else {
        return;
    };
    let path = joined.as_path();
    assert!(
        path.starts_with(&fx.canonical),
        "{input:?} cho ra {} nằm ngoài gốc",
        path.display()
    );
    assert!(
        !path.components().any(|c| matches!(c, Component::ParentDir)),
        "{input:?} còn `..`: {}",
        path.display()
    );
    assert_resolves_inside(input, path, &fx.canonical);
});

/// Phần đã tồn tại của đường dẫn, sau khi đi theo symlink, vẫn phải nằm trong gốc.
fn assert_resolves_inside(input: &str, path: &Path, root: &Path) {
    let Some(existing) = path.ancestors().find(|p| p.exists()) else {
        return;
    };
    if let Ok(resolved) = existing.canonicalize() {
        assert!(
            resolved.starts_with(root),
            "{input:?} đi qua symlink ra {}",
            resolved.display()
        );
    }
}
