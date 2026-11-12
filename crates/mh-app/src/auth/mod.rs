//! Mật khẩu admin và cờ LAN, đọc từ `<data>/auth.toml` (spec P2, mục 4.3 và 5.7).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use argon2::Argon2;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use mh_core::ErrorCode;
use mh_platform::fs::write_atomic;
use mh_platform::{DataDirs, PlatformError};
use serde::{Deserialize, Serialize};

use crate::{AppError, lock};

/// Mật khẩu mới ngắn hơn mức này thì [`ErrorCode::PasswordTooShort`].
/// Đếm Unicode scalar, cùng cách với tên server, không đếm byte.
const MIN_PASSWORD_CHARS: usize = 8;

/// Mật khẩu admin và cờ LAN.
///
/// [`Self::verify`], [`Self::set_password`] và [`Self::clear_password`] băm
/// argon2id đồng bộ, tốn vài chục mili giây. Người gọi trong runtime async phải
/// đưa chúng vào `spawn_blocking`. [`Self::password_set`] và [`Self::lan`] chỉ
/// `stat` file nên gọi trực tiếp được.
///
/// Mỗi lần gọi đều xem thời điểm sửa file. Nội dung chỉ đọc lại khi mtime đổi
/// hoặc file biến mất, để `reset-password` xóa `auth.toml` lúc controller đang
/// chạy có hiệu lực ngay.
pub struct AuthStore {
    path: PathBuf,
    inner: Mutex<Inner>,
}

struct Inner {
    /// `None` khi chưa đọc. Cache chỉ giữ bản đọc được; file hỏng không bị
    /// nhớ thành "chưa có mật khẩu".
    seen: Option<Seen>,
}

#[derive(Clone)]
struct Seen {
    /// `None` khi file không tồn tại.
    mtime: Option<SystemTime>,
    record: AuthRecord,
}

#[derive(Clone, Default)]
struct AuthRecord {
    password_hash: Option<String>,
    /// Đã áp dụng quy tắc "có hash mới được bật LAN".
    lan: bool,
}

#[derive(Serialize, Deserialize)]
struct AuthFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    password_hash: Option<String>,
    #[serde(default)]
    lan: bool,
}

impl AuthStore {
    /// Đọc `auth.toml` ngay. File hỏng thì lỗi: controller không được lên,
    /// vì coi file hỏng là "chưa đặt mật khẩu" sẽ mở khóa API.
    pub fn open(dirs: &DataDirs) -> Result<Self, AppError> {
        let store = Self {
            path: dirs.auth_file(),
            inner: Mutex::new(Inner { seen: None }),
        };
        store.password_set()?;
        Ok(store)
    }

    /// `true` khi file có `password_hash` dùng được.
    pub fn password_set(&self) -> Result<bool, AppError> {
        Ok(self.record()?.password_hash.is_some())
    }

    /// So mật khẩu với hash. Argon2 kiểm tra chuỗi PHC trong thời gian hằng.
    /// Chưa đặt mật khẩu thì `false`. File hỏng thì lỗi, không phải `false`.
    pub fn verify(&self, password: &str) -> Result<bool, AppError> {
        let hash = self.record()?.password_hash;
        Ok(hash.is_some_and(|phc| password_matches(password, &phc)))
    }

    /// `true` chỉ khi file ghi `lan = true` và có hash. Thiếu hash thì `false`.
    pub fn lan(&self) -> Result<bool, AppError> {
        Ok(self.record()?.lan)
    }

    /// Đặt hoặc đổi mật khẩu. Đồng bộ, tốn vài chục mili giây: từ async thì
    /// đưa vào `spawn_blocking`.
    ///
    /// Chưa có mật khẩu thì `current` bị bỏ qua. Đã có thì `current` phải đúng,
    /// sai thì [`ErrorCode::AuthInvalid`]. Mật khẩu mới dưới 8 ký tự thì
    /// [`ErrorCode::PasswordTooShort`].
    pub fn set_password(&self, current: Option<&str>, new: &str) -> Result<(), AppError> {
        if new.chars().count() < MIN_PASSWORD_CHARS {
            return Err(AppError::new(
                ErrorCode::PasswordTooShort,
                "mật khẩu mới ngắn hơn 8 ký tự",
            ));
        }
        // Giữ mutex trong lúc băm để hai lần ghi không xen vào nhau.
        let mut inner = lock(&self.inner);
        let mut record = refresh(&self.path, &mut inner)?;
        if let Some(hash) = &record.password_hash
            && !current.is_some_and(|password| password_matches(password, hash))
        {
            return Err(invalid_password());
        }
        record.password_hash = Some(hash_password(new)?);
        commit(&self.path, &mut inner, record)
    }

    /// Xóa mật khẩu nếu `current` đúng. Xóa thì LAN tắt theo.
    ///
    /// Đồng bộ vì phải băm để kiểm tra `current`. Từ async thì `spawn_blocking`.
    pub fn clear_password(&self, current: &str) -> Result<(), AppError> {
        let mut inner = lock(&self.inner);
        let record = refresh(&self.path, &mut inner)?;
        let Some(hash) = &record.password_hash else {
            return Err(invalid_password());
        };
        if !password_matches(current, hash) {
            return Err(invalid_password());
        }
        commit(&self.path, &mut inner, AuthRecord::default())
    }

    /// Bật hoặc tắt LAN. Bật khi chưa có mật khẩu thì [`ErrorCode::LanNeedsPassword`].
    pub fn set_lan(&self, on: bool) -> Result<(), AppError> {
        let mut inner = lock(&self.inner);
        let mut record = refresh(&self.path, &mut inner)?;
        if on && record.password_hash.is_none() {
            return Err(AppError::new(
                ErrorCode::LanNeedsPassword,
                "bật LAN khi chưa có mật khẩu",
            ));
        }
        if record.lan == on {
            return Ok(());
        }
        record.lan = on;
        commit(&self.path, &mut inner, record)
    }

    /// Xóa mật khẩu và tắt LAN, không kiểm tra mật khẩu cũ.
    ///
    /// Chỉ gọi từ đường mà người dùng phải ngồi trước máy (`minehost reset-password`,
    /// menu khay). Gọi từ API là cho phép xóa mật khẩu từ xa.
    pub fn reset(&self) -> Result<(), AppError> {
        let mut inner = lock(&self.inner);
        commit(&self.path, &mut inner, AuthRecord::default())
    }

    /// Xóa `auth.toml` mà không đọc nó, để file hỏng cũng xóa được. Dùng cho
    /// `minehost reset-password`, chạy ở process khác với controller. Controller đang chạy
    /// thấy file biến mất ở lần đọc kế tiếp (xem mtime trong [`Self::password_set`]).
    pub fn reset_file(dirs: &DataDirs) -> Result<(), AppError> {
        remove_auth_file(&dirs.auth_file())
    }

    fn record(&self) -> Result<AuthRecord, AppError> {
        let mut inner = lock(&self.inner);
        refresh(&self.path, &mut inner)
    }
}

impl std::fmt::Debug for AuthStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Không in hash: Debug có thể đi vào log.
        f.debug_struct("AuthStore")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

fn refresh(path: &Path, inner: &mut Inner) -> Result<AuthRecord, AppError> {
    let mtime = mtime_of(path)?;
    if let Some(seen) = &inner.seen
        && seen.mtime == mtime
    {
        return Ok(seen.record.clone());
    }
    let record = match mtime {
        None => AuthRecord::default(),
        Some(_) => read_auth_file(path)?,
    };
    inner.seen = Some(Seen {
        mtime,
        record: record.clone(),
    });
    Ok(record)
}

fn read_auth_file(path: &Path) -> Result<AuthRecord, AppError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        // Xóa file đúng lúc đang đọc: coi như chưa đặt mật khẩu.
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(AuthRecord::default()),
        Err(e) => return Err(PlatformError::io(path, e).into()),
    };
    parse_auth(&text)
}

/// File không phân tích được thì lỗi, không phải "chưa đặt mật khẩu".
/// `auth.toml` bị cắt giữa chừng mà bị coi là trống sẽ mở khóa API.
fn parse_auth(text: &str) -> Result<AuthRecord, AppError> {
    let file: AuthFile =
        toml::from_str(text).map_err(|_| corrupt("auth.toml không phải TOML hợp lệ"))?;
    let password_hash = match file.password_hash {
        Some(raw) => Some(checked_phc(&raw)?),
        None => None,
    };
    // `lan = true` mà thiếu hash thì coi như tắt (spec P2, mục 4.3).
    Ok(AuthRecord {
        lan: file.lan && password_hash.is_some(),
        password_hash,
    })
}

/// Hash lạ cũng là file hỏng. Để `verify` trả `false` thì máy bị khóa
/// mà không có lỗi rõ, hoặc tệ hơn là bị hiểu nhầm thành chưa đặt mật khẩu.
fn checked_phc(raw: &str) -> Result<String, AppError> {
    let parsed =
        PasswordHash::new(raw).map_err(|_| corrupt("password_hash không phải chuỗi PHC"))?;
    if parsed.algorithm.as_str() != argon2::ARGON2ID_IDENT.as_str() {
        return Err(corrupt("password_hash không phải argon2id"));
    }
    if parsed.hash.is_none() {
        return Err(corrupt("password_hash không phải chuỗi PHC"));
    }
    Ok(raw.to_owned())
}

fn commit(path: &Path, inner: &mut Inner, record: AuthRecord) -> Result<(), AppError> {
    let mtime = write_auth_file(path, &record)?;
    // File biến mất ngay sau khi ghi thì không giữ hash cũ trong bộ nhớ.
    let record = if mtime.is_some() {
        record
    } else {
        AuthRecord::default()
    };
    inner.seen = Some(Seen { mtime, record });
    Ok(())
}

fn write_auth_file(path: &Path, record: &AuthRecord) -> Result<Option<SystemTime>, AppError> {
    if record.password_hash.is_none() {
        remove_auth_file(path)?;
        return Ok(None);
    }
    let file = AuthFile {
        password_hash: record.password_hash.clone(),
        lan: record.lan,
    };
    let text = toml::to_string(&file)
        .map_err(|_| AppError::new(ErrorCode::Internal, "không ghi được auth.toml"))?;
    write_atomic(path, text.as_bytes())?;
    set_private_mode(path)?;
    mtime_of(path)
}

fn remove_auth_file(path: &Path) -> Result<(), AppError> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(PlatformError::io(path, e).into()),
        _ => Ok(()),
    }
}

/// Hash không phải để tài khoản khác trên cùng máy đọc được.
#[cfg(unix)]
fn set_private_mode(path: &Path) -> Result<(), AppError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|e| PlatformError::io(path, e))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_mode(_path: &Path) -> Result<(), AppError> {
    Ok(())
}

fn mtime_of(path: &Path) -> Result<Option<SystemTime>, AppError> {
    match fs::metadata(path) {
        Ok(meta) => meta
            .modified()
            .map(Some)
            .map_err(|e| PlatformError::io(path, e).into()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(PlatformError::io(path, e).into()),
    }
}

fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| AppError::new(ErrorCode::Internal, "không băm được mật khẩu"))?;
    Ok(hash.to_string())
}

/// Không so sánh mật khẩu bằng `==` trước khi gọi Argon2: thư viện tự so
/// trong thời gian hằng khi chuỗi PHC hợp lệ.
fn password_matches(password: &str, phc: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(phc) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

fn invalid_password() -> AppError {
    AppError::new(ErrorCode::AuthInvalid, "mật khẩu hiện tại không đúng")
}

fn corrupt(detail: &'static str) -> AppError {
    AppError::new(ErrorCode::ConfigInvalid, detail)
}

#[cfg(test)]
mod tests;
