//! Trạng thái đăng nhập trong RAM: phiên, khóa theo IP sau nhiều lần sai, và chỗ băm mật khẩu
//! (spec P2, mục 3.3, 6.10). Tách khỏi `auth.rs` để mỗi file dưới 500 dòng.

use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand_core::{OsRng, RngCore};

use crate::error::ApiError;

pub(crate) const SESSION_DAYS: u64 = 30;
const SESSION_TTL: Duration = Duration::from_secs(SESSION_DAYS * 24 * 60 * 60);
const MAX_FAILURES: u32 = 5;
const FIRST_LOCK_SECS: u64 = 60;
const MAX_LOCK_SECS: u64 = 15 * 60;

/// Số lần băm mật khẩu cùng lúc của cả controller. Argon2 mặc định tốn khoảng 19 MiB RAM mỗi lần;
/// không giới hạn thì vài trăm request đăng nhập song song từ LAN chiếm hàng GB RAM và hết CPU.
const MAX_VERIFYING: usize = 4;

pub(crate) struct AuthGate {
    sessions: Mutex<HashMap<String, Session>>,
    logins: Mutex<HashMap<IpAddr, Attempt>>,
    verifying: Arc<tokio::sync::Semaphore>,
    /// IP đang được băm mật khẩu. Mỗi IP một lần tại một lúc: một máy không chiếm hết chỗ của người
    /// khác, và lần sai thứ 5 luôn được ghi trước lần thử kế tiếp của cùng IP.
    in_flight: Mutex<HashSet<IpAddr>>,
}

impl Default for AuthGate {
    fn default() -> Self {
        Self {
            sessions: Mutex::default(),
            logins: Mutex::default(),
            verifying: Arc::new(tokio::sync::Semaphore::new(MAX_VERIFYING)),
            in_flight: Mutex::default(),
        }
    }
}

/// Chỗ băm mật khẩu của một IP. Đi vào luồng blocking cùng việc băm, nên client ngắt kết nối giữa
/// chừng không nhả chỗ trong khi Argon2 vẫn chạy.
pub(crate) struct VerifySlot {
    gate: Arc<AuthGate>,
    ip: IpAddr,
    _permit: tokio::sync::OwnedSemaphorePermit,
}

impl Drop for VerifySlot {
    fn drop(&mut self) {
        lock(&self.gate.in_flight).remove(&self.ip);
    }
}

impl AuthGate {
    pub(crate) fn claim(gate: &Arc<Self>, ip: IpAddr) -> Option<VerifySlot> {
        let mut in_flight = lock(&gate.in_flight);
        if in_flight.contains(&ip) {
            return None;
        }
        let permit = Arc::clone(&gate.verifying).try_acquire_owned().ok()?;
        in_flight.insert(ip);
        Some(VerifySlot {
            gate: Arc::clone(gate),
            ip,
            _permit: permit,
        })
    }
}

struct Session {
    csrf: String,
    expires_at: SystemTime,
}

struct Attempt {
    failures: u32,
    locked_until: Option<SystemTime>,
    next_lock_secs: u64,
    touched: SystemTime,
}

impl AuthGate {
    pub(crate) fn open_session(&self) -> Result<(String, String), ApiError> {
        let token = random_token()?;
        let csrf = random_token()?;
        let now = SystemTime::now();
        let expires_at = now
            .checked_add(SESSION_TTL)
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let mut sessions = lock(&self.sessions);
        // Phiên hết hạn chỉ bị xóa khi được dùng lại; dọn ở đây để bảng không phình qua nhiều tháng.
        sessions.retain(|_, session| session.expires_at > now);
        sessions.insert(
            token.clone(),
            Session {
                csrf: csrf.clone(),
                expires_at,
            },
        );
        Ok((token, csrf))
    }

    pub(crate) fn csrf_if_valid(&self, token: &str) -> Option<String> {
        let mut sessions = lock(&self.sessions);
        let expires_at = sessions.get(token)?.expires_at;
        if expires_at <= SystemTime::now() {
            sessions.remove(token);
            return None;
        }
        Some(sessions.get(token)?.csrf.clone())
    }

    pub(crate) fn remove(&self, token: &str) {
        lock(&self.sessions).remove(token);
    }

    /// Đổi hoặc xóa mật khẩu thì mọi trình duyệt phải đăng nhập lại.
    pub(crate) fn clear_sessions(&self) {
        lock(&self.sessions).clear();
    }

    pub(crate) fn retry_after_secs(&self, ip: IpAddr) -> Option<u64> {
        let now = SystemTime::now();
        let mut logins = lock(&self.logins);
        prune(&mut logins, now);
        let until = logins.get(&ip)?.locked_until?;
        secs_until(until, now)
    }

    /// Lần sai thứ 5 chỉ ghi nhận. Lần gọi sau, trong lúc còn khóa, mới bị 429.
    pub(crate) fn record_failure(&self, ip: IpAddr) {
        let now = SystemTime::now();
        let mut logins = lock(&self.logins);
        prune(&mut logins, now);
        let attempt = logins.entry(ip).or_insert_with(|| Attempt {
            failures: 0,
            locked_until: None,
            next_lock_secs: FIRST_LOCK_SECS,
            touched: now,
        });
        if attempt.locked_until.is_some_and(|until| until <= now) {
            attempt.locked_until = None;
        }
        attempt.touched = now;
        attempt.failures += 1;
        if attempt.failures < MAX_FAILURES {
            return;
        }
        let lock_secs = attempt.next_lock_secs;
        attempt.locked_until = now.checked_add(Duration::from_secs(lock_secs));
        attempt.next_lock_secs = (lock_secs * 2).min(MAX_LOCK_SECS);
        attempt.failures = 0;
    }

    pub(crate) fn clear_failures(&self, ip: IpAddr) {
        lock(&self.logins).remove(&ip);
    }
}

fn random_token() -> Result<String, ApiError> {
    let mut bytes = [0u8; 32];
    // `fill_bytes` panic khi OS không có entropy. Trả lỗi thay vì làm sập request.
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(ApiError::internal)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// Bản ghi không còn khóa và lâu không bị đụng thì bỏ, để bảng IP không phình.
fn prune(logins: &mut HashMap<IpAddr, Attempt>, now: SystemTime) {
    let keep = Duration::from_secs(MAX_LOCK_SECS);
    logins.retain(|_, attempt| {
        if attempt.locked_until.is_some_and(|until| until > now) {
            return true;
        }
        let anchor = attempt.locked_until.unwrap_or(attempt.touched);
        now.duration_since(anchor).is_ok_and(|age| age < keep)
    });
}

fn secs_until(until: SystemTime, now: SystemTime) -> Option<u64> {
    let left = until.duration_since(now).ok()?;
    if left.is_zero() {
        return None;
    }
    Some(left.as_secs() + u64::from(left.subsec_nanos() > 0))
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // Chỉ bị poison khi một luồng panic lúc đang giữ; dữ liệu bên trong vẫn dùng được.
    mutex.lock().unwrap_or_else(|err| err.into_inner())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn login_drops_expired_sessions() {
        let gate = AuthGate::default();
        let (old, _) = gate.open_session().unwrap();
        lock(&gate.sessions).get_mut(&old).unwrap().expires_at = SystemTime::UNIX_EPOCH;
        let (fresh, csrf) = gate.open_session().unwrap();
        let sessions = lock(&gate.sessions);
        assert!(!sessions.contains_key(&old));
        assert_eq!(sessions.get(&fresh).map(|s| s.csrf.clone()), Some(csrf));
    }
}
