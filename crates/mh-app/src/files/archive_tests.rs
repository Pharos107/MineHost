//! Test zip của trang Tệp: nén rồi giải nén lại, zip độc, trùng tên, thiếu chỗ, tìm file.

use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::Path;

use mh_core::ErrorCode;
use mh_platform::SafeRoot;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use super::{compress_sync, compress_target, extract_sync, plan_extract, plan_zip};
use crate::files::FileKind;
use crate::files::search::search_sync;

fn tree() -> (tempfile::TempDir, SafeRoot) {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    fs::create_dir_all(dir.join("world/region")).unwrap();
    fs::write(dir.join("world/level.dat"), b"level").unwrap();
    fs::write(dir.join("world/region/r.0.0.mca"), vec![7u8; 200_000]).unwrap();
    fs::create_dir_all(dir.join("plugins/Essentials")).unwrap();
    fs::write(dir.join("plugins/Essentials/config.yml"), b"a: 1").unwrap();
    fs::write(dir.join("server.properties"), b"motd=x").unwrap();
    let root = SafeRoot::new(dir).unwrap();
    (tmp, root)
}

/// Zip viết tay, cho các ca zip độc mà `plan_zip` không bao giờ tạo ra.
fn raw_zip(dir: &Path, name: &str, entries: &[(&str, &[u8])]) {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (entry, bytes) in entries {
        zip.start_file(*entry, SimpleFileOptions::default())
            .unwrap();
        zip.write_all(bytes).unwrap();
    }
    fs::write(dir.join(name), zip.finish().unwrap().into_inner()).unwrap();
}

fn names(path: &Path) -> Vec<String> {
    let mut archive = ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().unwrap().into_owned())
        .collect()
}

#[test]
fn nen_roi_giai_nen_lai_ra_dung_cay_thu_muc() {
    let (tmp, root) = tree();
    let paths = vec!["world".to_owned(), "server.properties".to_owned()];
    let plan = plan_zip(&root, &paths).unwrap();
    assert_eq!(plan.total, 5 + 200_000 + 6);
    let dest = compress_target(&root, "backup.zip").unwrap();
    let mut last = 0;
    compress_sync(&plan, &dest, &mut |done| {
        last = done;
        Ok(())
    })
    .unwrap();
    assert_eq!(last, plan.total);
    assert!(!tmp.path().join("backup.zip.part").exists());
    assert_eq!(
        names(&dest),
        [
            "world/",
            "world/level.dat",
            "world/region/",
            "world/region/r.0.0.mca",
            "server.properties"
        ]
    );
    assert_eq!(
        compress_target(&root, "backup.zip").unwrap_err().code,
        ErrorCode::FileExists
    );

    let plan = plan_extract(&root, "backup.zip", "copy", None).unwrap();
    extract_sync(plan, &mut |_| Ok(())).unwrap();
    let copy = tmp.path().join("copy");
    assert_eq!(fs::read(copy.join("world/level.dat")).unwrap(), b"level");
    assert_eq!(
        fs::read(copy.join("world/region/r.0.0.mca")).unwrap().len(),
        200_000
    );
    assert_eq!(fs::read(copy.join("server.properties")).unwrap(), b"motd=x");
}

#[test]
fn nen_bi_huy_thi_khong_de_lai_file_do() {
    let (tmp, root) = tree();
    let plan = plan_zip(&root, &["world".to_owned()]).unwrap();
    let dest = compress_target(&root, "w.zip").unwrap();
    let err = compress_sync(&plan, &dest, &mut |_| Err(crate::AppError::cancelled())).unwrap_err();
    assert_eq!(err.code, ErrorCode::JobCancelled);
    assert!(!tmp.path().join("w.zip").exists());
    assert!(!tmp.path().join("w.zip.part").exists());
}

#[test]
fn chon_thieu_hoac_trung_ten_bi_tu_choi() {
    let (_tmp, root) = tree();
    assert_eq!(
        plan_zip(&root, &[]).unwrap_err().code,
        ErrorCode::InvalidInput
    );
    assert_eq!(
        plan_zip(&root, &["nope".to_owned()]).unwrap_err().code,
        ErrorCode::NotFound
    );
    assert_eq!(
        plan_zip(&root, &["".to_owned()]).unwrap_err().code,
        ErrorCode::InvalidInput
    );
    let twice = ["world".to_owned(), "./world".to_owned()];
    assert_eq!(
        plan_zip(&root, &twice).unwrap_err().code,
        ErrorCode::InvalidInput
    );
}

#[test]
fn zip_thoat_ra_ngoai_bi_tu_choi_va_khong_ghi_gi() {
    let (tmp, root) = tree();
    raw_zip(
        tmp.path(),
        "evil.zip",
        &[("ok.txt", b"ok"), ("../evil.txt", b"x")],
    );
    let err = plan_extract(&root, "evil.zip", "", None).unwrap_err();
    assert_eq!(err.code, ErrorCode::PathOutsideRoot);
    assert!(!tmp.path().join("ok.txt").exists());
    assert!(!tmp.path().parent().unwrap().join("evil.txt").exists());
}

#[test]
fn ten_co_dau_gach_nguoc_bi_tu_choi() {
    let (tmp, root) = tree();
    raw_zip(tmp.path(), "bs.zip", &[("a\\..\\..\\evil.txt", b"x")]);
    let err = plan_extract(&root, "bs.zip", "out", None).unwrap_err();
    assert_eq!(err.code, ErrorCode::PathOutsideRoot);
    assert!(!tmp.path().join("out").exists());
}

#[cfg(unix)]
#[test]
fn symlink_chen_vao_sau_khi_kiem_thi_khong_ghi_ra_ngoai() {
    let (tmp, root) = tree();
    let outside_dir = tempfile::tempdir().unwrap();
    raw_zip(tmp.path(), "p.zip", &[("sub/x.txt", b"x")]);
    let plan = plan_extract(&root, "p.zip", "out", None).unwrap();
    std::os::unix::fs::symlink(outside_dir.path(), tmp.path().join("out")).unwrap();
    let err = extract_sync(plan, &mut |_| Ok(())).unwrap_err();
    assert_eq!(err.code, ErrorCode::PathOutsideRoot);
    assert!(!outside_dir.path().join("sub").exists());
}

#[test]
fn trung_file_da_co_thi_bao_duong_dan_thu_muc_thi_gop() {
    let (tmp, root) = tree();
    raw_zip(
        tmp.path(),
        "pack.zip",
        &[("plugins/New.jar", b"jar"), ("server.properties", b"y")],
    );
    let err = plan_extract(&root, "pack.zip", "", None).unwrap_err();
    assert_eq!(err.code, ErrorCode::FileExists);
    assert_eq!(err.params["path"], "server.properties");
    assert!(!tmp.path().join("plugins/New.jar").exists());

    raw_zip(tmp.path(), "plugin.zip", &[("plugins/New.jar", b"jar")]);
    let plan = plan_extract(&root, "plugin.zip", "", None).unwrap();
    extract_sync(plan, &mut |_| Ok(())).unwrap();
    assert_eq!(
        fs::read(tmp.path().join("plugins/New.jar")).unwrap(),
        b"jar"
    );
}

#[test]
fn thieu_cho_trong_hoac_khong_phai_zip() {
    let (tmp, root) = tree();
    raw_zip(tmp.path(), "big.zip", &[("a.bin", &[0u8; 4096])]);
    let err = plan_extract(&root, "big.zip", "out", Some(1024)).unwrap_err();
    assert_eq!(err.code, ErrorCode::NotEnoughDisk);
    assert_eq!(
        plan_extract(&root, "server.properties", "", None)
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
    assert_eq!(
        plan_extract(&root, "world", "", None).unwrap_err().code,
        ErrorCode::InvalidInput
    );
    // Đích là một file có sẵn.
    assert_eq!(
        plan_extract(&root, "big.zip", "server.properties", None)
            .unwrap_err()
            .code,
        ErrorCode::FileExists
    );
}

#[test]
fn zip_rong_van_giai_nen_duoc() {
    let (tmp, root) = tree();
    raw_zip(tmp.path(), "empty.zip", &[]);
    let plan = plan_extract(&root, "empty.zip", "empty", None).unwrap();
    extract_sync(plan, &mut |_| Ok(())).unwrap();
    assert!(tmp.path().join("empty").is_dir());
    let mut bytes = Vec::new();
    fs::File::open(tmp.path().join("empty.zip"))
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(!bytes.is_empty());
}

#[test]
fn tim_theo_ten_trong_moi_thu_muc_con() {
    let (_tmp, root) = tree();
    let found = search_sync(&root, "", "CONFIG", (200, 100_000)).unwrap();
    assert!(!found.truncated);
    assert_eq!(found.items.len(), 1);
    assert_eq!(found.items[0].path, "plugins/Essentials/config.yml");
    assert_eq!(found.items[0].size, Some(4));

    let found = search_sync(&root, "world", "r", (200, 100_000)).unwrap();
    let paths: Vec<_> = found.items.iter().map(|item| item.path.as_str()).collect();
    assert_eq!(paths.len(), 2, "{paths:?}");
    assert!(paths.contains(&"world/region"));
    assert!(paths.contains(&"world/region/r.0.0.mca"));
    assert_eq!(
        found
            .items
            .iter()
            .find(|i| i.path == "world/region")
            .unwrap()
            .kind,
        FileKind::Dir
    );

    let capped = search_sync(&root, "", "e", (1, 100_000)).unwrap();
    assert!(capped.truncated);
    assert_eq!(capped.items.len(), 1);
    assert!(search_sync(&root, "", "e", (200, 2)).unwrap().truncated);
    assert_eq!(
        search_sync(&root, "", "  ", (200, 100)).unwrap_err().code,
        ErrorCode::InvalidInput
    );
    assert_eq!(
        search_sync(&root, "server.properties", "a", (200, 100))
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
}
