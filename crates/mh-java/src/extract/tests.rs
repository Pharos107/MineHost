use std::io::Write;

use zip::write::SimpleFileOptions;

use super::*;

fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
    for (name, data) in entries {
        if name.ends_with('/') {
            zip.add_directory(*name, SimpleFileOptions::default())
                .unwrap();
        } else {
            zip.start_file(*name, SimpleFileOptions::default()).unwrap();
            zip.write_all(data).unwrap();
        }
    }
    zip.finish().unwrap();
}

/// `symlink`: tên entry kiểu symlink trỏ ra ngoài, để chắc chắn nó bị bỏ qua.
fn make_tar_gz(path: &Path, files: &[(&str, &[u8])], symlink: Option<&str>) {
    let gz = flate2::write::GzEncoder::new(File::create(path).unwrap(), Default::default());
    let mut tar = tar::Builder::new(gz);
    for (name, data) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        tar.append_data(&mut header, name, *data).unwrap();
    }
    if let Some(name) = symlink {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        tar.append_link(&mut header, name, "/etc/passwd").unwrap();
    }
    tar.into_inner().unwrap().finish().unwrap();
}

#[test]
fn zip_strips_single_root_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("jre.zip");
    make_zip(
        &archive,
        &[
            ("jdk-21.0.5+11-jre/", b""),
            ("jdk-21.0.5+11-jre/bin/java.exe", b"exe"),
            ("jdk-21.0.5+11-jre/release", b"JAVA_VERSION=\"21.0.5\""),
        ],
    );
    let dest = tmp.path().join("x");

    let root = unpack(ArchiveKind::Zip, &archive, &dest).unwrap();
    assert_eq!(root, dest.join("jdk-21.0.5+11-jre"));
    assert_eq!(fs::read(root.join("bin").join("java.exe")).unwrap(), b"exe");
    assert!(root.join("release").is_file());
}

#[test]
fn zip_slip_is_blocked() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("evil.zip");
    make_zip(&archive, &[("jre/bin/java", b"ok"), ("../evil", b"pwned")]);
    let dest = tmp.path().join("x");

    let err = unpack(ArchiveKind::Zip, &archive, &dest).unwrap_err();
    assert!(
        matches!(err, JavaError::UnsafeArchive(ref n) if n == "../evil"),
        "{err:?}"
    );
    assert!(!tmp.path().join("evil").exists());
}

#[test]
fn tar_gz_strips_root_and_skips_symlinks() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("jre.tar.gz");
    make_tar_gz(
        &archive,
        &[
            ("jdk-21-jre/bin/java", b"elf"),
            ("jdk-21-jre/release", b"v"),
        ],
        Some("jdk-21-jre/legal/link"),
    );
    let dest = tmp.path().join("x");

    let root = unpack(ArchiveKind::TarGz, &archive, &dest).unwrap();
    assert_eq!(root, dest.join("jdk-21-jre"));
    assert_eq!(fs::read(root.join("bin").join("java")).unwrap(), b"elf");
    assert!(!root.join("legal").join("link").exists());
}

#[cfg(unix)]
#[test]
fn tar_gz_keeps_exec_permission() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("jre.tar.gz");
    make_tar_gz(&archive, &[("jre/bin/java", b"elf")], None);

    let root = unpack(ArchiveKind::TarGz, &archive, &tmp.path().join("x")).unwrap();
    let mode = fs::metadata(root.join("bin/java"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o111, 0o111);
}

#[test]
fn archive_without_root_directory_is_kept_as_is() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("flat.zip");
    make_zip(&archive, &[("bin/java", b"x"), ("release", b"v")]);
    let dest = tmp.path().join("x");

    assert_eq!(unpack(ArchiveKind::Zip, &archive, &dest).unwrap(), dest);
}

#[test]
fn corrupt_archive_is_install_error() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("bad.zip");
    fs::write(&archive, b"not a zip").unwrap();
    let err = unpack(ArchiveKind::Zip, &archive, &tmp.path().join("x")).unwrap_err();
    assert!(matches!(err, JavaError::Install(_)), "{err:?}");
}
