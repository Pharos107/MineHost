//! Các địa chỉ LAN đang có listener (spec P2, mục 5.9).
//!
//! Binary mở và đóng listener; API chỉ đọc danh sách này để kiểm tra `Host`, `Origin` và trả
//! `lan_urls`. Một bản dùng chung cho mọi listener, nên phiên đăng nhập cũng dùng chung.

use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use tokio::sync::{Notify, watch};

#[derive(Debug)]
pub struct LanState {
    open: RwLock<Vec<SocketAddr>>,
    wake: Notify,
    /// Tăng mỗi lần binary áp dụng xong cờ `lan`. `PUT /settings/access` chờ giá trị này đổi.
    applied: watch::Sender<u64>,
}

impl LanState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            open: RwLock::new(Vec::new()),
            wake: Notify::new(),
            applied: watch::channel(0).0,
        })
    }

    /// Địa chỉ đang có listener LAN, theo thứ tự binary ghi vào.
    pub fn open_addrs(&self) -> Vec<SocketAddr> {
        self.open
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
    }

    /// Binary gọi sau mỗi lần mở hoặc đóng listener, kể cả khi danh sách không đổi.
    pub fn set_open(&self, addrs: Vec<SocketAddr>) {
        *self.open.write().unwrap_or_else(|err| err.into_inner()) = addrs;
        self.applied.send_modify(|n| *n = n.wrapping_add(1));
    }

    /// Chờ tới khi API báo cờ `lan` vừa đổi. Lỡ lúc không có ai chờ thì lần gọi sau trả ngay.
    pub async fn woken(&self) {
        self.wake.notified().await;
    }

    /// Báo binary áp dụng ngay, rồi chờ nó xong tối đa `limit`. Hết giờ thì trả trạng thái hiện có.
    pub(crate) async fn apply_now(&self, limit: Duration) {
        let mut applied = self.applied.subscribe();
        applied.mark_unchanged();
        self.wake.notify_one();
        let _ = tokio::time::timeout(limit, applied.changed()).await;
    }

    /// `host` là giá trị header `Host`, ví dụ `192.168.1.5:7878`.
    pub(crate) fn has_host(&self, host: &str) -> bool {
        self.open
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .iter()
            .any(|addr| addr.to_string().eq_ignore_ascii_case(host))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn host_matches_only_open_listeners() {
        let lan = LanState::new();
        assert!(!lan.has_host("192.168.1.5:7878"));
        lan.set_open(vec!["192.168.1.5:7878".parse().unwrap()]);
        assert!(lan.has_host("192.168.1.5:7878"));
        assert!(!lan.has_host("192.168.1.5:7879"));
        assert!(!lan.has_host("192.168.1.6:7878"));
        lan.set_open(Vec::new());
        assert!(!lan.has_host("192.168.1.5:7878"));
    }

    #[tokio::test]
    async fn apply_now_returns_when_binary_applies() {
        let lan = LanState::new();
        let worker = Arc::clone(&lan);
        tokio::spawn(async move {
            worker.woken().await;
            worker.set_open(vec!["10.0.0.2:7878".parse().unwrap()]);
        });
        let started = std::time::Instant::now();
        lan.apply_now(Duration::from_secs(10)).await;
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(lan.open_addrs().len(), 1);
    }
}
