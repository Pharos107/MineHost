use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use mh_core::ErrorCode;
use mh_platform::DataDirs;

use super::*;

const PASSWORD: &str = "correct-horse";
const NEXT: &str = "battery-staple";

fn fresh() -> (tempfile::TempDir, AuthStore) {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    let store = AuthStore::open(&dirs).unwrap();
    (tmp, store)
}

fn overwrite(path: &Path, contents: &str) {
    fs::write(path, contents).unwrap();
    // Ép mtime đổi: độ phân giải thấp có thể giữ nguyên mtime vừa ghi.
    let file = fs::File::options().write(true).open(path).unwrap();
    file.set_modified(SystemTime::now() + Duration::from_secs(2))
        .unwrap();
}

#[test]
fn set_verify_and_change_password() {
    let (_tmp, store) = fresh();
    assert!(!store.password_set().unwrap());
    assert!(!store.verify(PASSWORD).unwrap());

    store.set_password(None, PASSWORD).unwrap();
    assert!(store.password_set().unwrap());
    assert!(store.verify(PASSWORD).unwrap());
    assert!(!store.verify(NEXT).unwrap());

    let text = fs::read_to_string(&store.path).unwrap();
    assert!(text.contains("$argon2id$v=19$m=19456,t=2,p=1$"));
    assert!(text.contains("lan = false"));
    assert!(!text.contains(PASSWORD));

    let err = store.set_password(None, NEXT).unwrap_err();
    assert_eq!(err.code, ErrorCode::AuthInvalid);
    let err = store.set_password(Some(NEXT), "other-secret").unwrap_err();
    assert_eq!(err.code, ErrorCode::AuthInvalid);
    assert!(store.verify(PASSWORD).unwrap());

    let first = fs::read_to_string(&store.path).unwrap();
    store.set_password(Some(PASSWORD), PASSWORD).unwrap();
    let second = fs::read_to_string(&store.path).unwrap();
    assert_ne!(first, second);
    assert!(store.verify(PASSWORD).unwrap());

    store.set_password(Some(PASSWORD), NEXT).unwrap();
    assert!(store.verify(NEXT).unwrap());
    assert!(!store.verify(PASSWORD).unwrap());
}

#[test]
fn password_shorter_than_8_chars_is_rejected() {
    let (_tmp, store) = fresh();
    // 7 ký tự Unicode nhưng dài hơn 8 byte: phải đếm ký tự, không đếm byte.
    let err = store.set_password(None, "ảảảảảảả").unwrap_err();
    assert_eq!(err.code, ErrorCode::PasswordTooShort);
    assert!(!store.password_set().unwrap());
    assert!(!store.path.exists());

    let err = store.set_password(None, "1234567").unwrap_err();
    assert_eq!(err.code, ErrorCode::PasswordTooShort);

    store.set_password(None, "12345678").unwrap();
    let err = store.set_password(Some("12345678"), "1234567").unwrap_err();
    assert_eq!(err.code, ErrorCode::PasswordTooShort);
    assert!(store.verify("12345678").unwrap());
}

#[test]
fn clear_password_turns_lan_off() {
    let (_tmp, store) = fresh();
    store.set_password(None, PASSWORD).unwrap();
    store.set_lan(true).unwrap();
    store.set_password(Some(PASSWORD), NEXT).unwrap();
    assert!(store.lan().unwrap());

    let err = store.clear_password(PASSWORD).unwrap_err();
    assert_eq!(err.code, ErrorCode::AuthInvalid);
    assert!(store.password_set().unwrap());
    assert!(store.lan().unwrap());

    store.clear_password(NEXT).unwrap();
    assert!(!store.password_set().unwrap());
    assert!(!store.lan().unwrap());
    assert!(!store.path.exists());
}

#[test]
fn set_lan_requires_a_password() {
    let (_tmp, store) = fresh();
    let err = store.set_lan(true).unwrap_err();
    assert_eq!(err.code, ErrorCode::LanNeedsPassword);
    assert!(!store.lan().unwrap());

    store.set_password(None, PASSWORD).unwrap();
    store.set_lan(true).unwrap();
    assert!(store.lan().unwrap());
    store.set_lan(false).unwrap();
    assert!(!store.lan().unwrap());
    assert!(store.password_set().unwrap());
}

#[test]
fn lan_true_without_hash_reads_as_off() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("auth.toml");
    fs::write(&path, "lan = true\n").unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    let store = AuthStore::open(&dirs).unwrap();
    assert!(!store.password_set().unwrap());
    assert!(!store.lan().unwrap());
    assert_eq!(fs::read_to_string(&path).unwrap(), "lan = true\n");
}

#[test]
fn deleting_the_file_is_seen_without_restart() {
    let (_tmp, store) = fresh();
    store.set_password(None, PASSWORD).unwrap();
    store.set_lan(true).unwrap();
    fs::remove_file(&store.path).unwrap();
    assert!(!store.password_set().unwrap());
    assert!(!store.lan().unwrap());
    assert!(!store.verify(PASSWORD).unwrap());
}

#[test]
fn external_rewrite_is_reread() {
    let (_tmp, store) = fresh();
    store.set_password(None, PASSWORD).unwrap();
    overwrite(&store.path, "lan = true\n");
    assert!(!store.password_set().unwrap());
    assert!(!store.lan().unwrap());
}

#[test]
fn corrupt_file_is_an_error_not_unlocked() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("auth.toml"), b"@@@").unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    let err = AuthStore::open(&dirs).unwrap_err();
    assert_eq!(err.code, ErrorCode::ConfigInvalid);
    assert_eq!(err.detail, "auth.toml không phải TOML hợp lệ");

    fs::write(tmp.path().join("auth.toml"), "password_hash = \"nope\"\n").unwrap();
    let err = AuthStore::open(&dirs).unwrap_err();
    assert_eq!(err.detail, "password_hash không phải chuỗi PHC");
    assert!(!err.detail.contains("nope"));

    fs::write(
        tmp.path().join("auth.toml"),
        "password_hash = \"$argon2i\"\n",
    )
    .unwrap();
    let err = AuthStore::open(&dirs).unwrap_err();
    assert_eq!(err.detail, "password_hash không phải argon2id");

    let (_kept, store) = fresh();
    store.set_password(None, PASSWORD).unwrap();
    overwrite(&store.path, "@@@");
    assert_eq!(
        store.password_set().unwrap_err().code,
        ErrorCode::ConfigInvalid
    );
    assert_eq!(
        store.verify(PASSWORD).unwrap_err().code,
        ErrorCode::ConfigInvalid
    );
    assert_eq!(store.lan().unwrap_err().code, ErrorCode::ConfigInvalid);

    store.reset().unwrap();
    assert!(!store.password_set().unwrap());
    assert!(!store.path.exists());
}

#[test]
fn reset_clears_without_the_old_password() {
    let (_tmp, store) = fresh();
    store.set_password(None, PASSWORD).unwrap();
    store.set_lan(true).unwrap();
    store.reset().unwrap();
    assert!(!store.password_set().unwrap());
    assert!(!store.lan().unwrap());
    assert!(!store.verify(PASSWORD).unwrap());
}

#[cfg(unix)]
#[test]
fn auth_file_mode_is_600() {
    use std::os::unix::fs::PermissionsExt;
    let (_tmp, store) = fresh();
    store.set_password(None, PASSWORD).unwrap();
    let mode = fs::metadata(&store.path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    store.set_lan(true).unwrap();
    let mode = fs::metadata(&store.path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn reset_file_unlocks_a_running_store_even_if_corrupt() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
    let store = AuthStore::open(&dirs).unwrap();
    store.set_password(None, PASSWORD).unwrap();
    store.set_lan(true).unwrap();

    // Process khác (lệnh `reset-password`) xóa file trong lúc store vẫn sống.
    AuthStore::reset_file(&dirs).unwrap();
    assert!(!store.password_set().unwrap());
    assert!(!store.lan().unwrap());

    fs::write(dirs.auth_file(), "@@@").unwrap();
    assert!(AuthStore::open(&dirs).is_err());
    AuthStore::reset_file(&dirs).unwrap();
    assert!(!dirs.auth_file().exists());
    // Không có file thì cũng không lỗi.
    AuthStore::reset_file(&dirs).unwrap();
}
