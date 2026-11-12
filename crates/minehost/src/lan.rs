//! Mở và đóng listener LAN theo cờ `lan` trong `auth.toml` (spec P2, mục 5.9).
//!
//! Cùng một router (bản clone) phục vụ `127.0.0.1` và từng địa chỉ LAN, nên phiên đăng nhập,
//! khóa đăng nhập sai và kiểm tra CSRF dùng chung.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Router;
use mh_api::LanState;
use mh_app::App;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

/// Đọc lại cờ `lan` mỗi chừng này. `reset-password` chạy ở process khác nên không báo được.
const FLAG_POLL: Duration = Duration::from_secs(2);
/// Quét lại địa chỉ LAN (DHCP có thể đổi IP).
const RESCAN: Duration = Duration::from_secs(60);

/// Hàm lấy địa chỉ LAN. Test thay bằng `127.0.0.2` để không phụ thuộc mạng của máy CI.
pub type AddrSource = Arc<dyn Fn() -> Vec<Ipv4Addr> + Send + Sync>;

pub struct LanManager {
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl LanManager {
    pub fn spawn(
        app: Arc<App>,
        router: Router,
        lan: Arc<LanState>,
        port: u16,
        addrs: AddrSource,
    ) -> Self {
        let (stop, stopped) = oneshot::channel();
        let worker = Worker {
            app,
            router,
            lan,
            port,
            addrs,
            open: HashMap::new(),
            scanned: Vec::new(),
            last_scan: None,
        };
        let task = tokio::spawn(worker.run(stopped));
        Self {
            stop: Some(stop),
            task,
        }
    }

    /// Đóng mọi listener LAN và chờ vòng lặp dừng.
    pub async fn stop(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        let _ = (&mut self.task).await;
    }
}

struct Listener {
    stop: Option<oneshot::Sender<()>>,
}

impl Listener {
    /// Báo `with_graceful_shutdown` dừng nhận kết nối mới. Lỗi gửi nghĩa là task đã dừng.
    fn close(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

struct Worker {
    app: Arc<App>,
    router: Router,
    lan: Arc<LanState>,
    port: u16,
    addrs: AddrSource,
    open: HashMap<Ipv4Addr, Listener>,
    scanned: Vec<Ipv4Addr>,
    last_scan: Option<Instant>,
}

impl Worker {
    async fn run(mut self, mut stopped: oneshot::Receiver<()>) {
        loop {
            self.apply().await;
            tokio::select! {
                _ = &mut stopped => break,
                () = tokio::time::sleep(FLAG_POLL) => {}
                () = self.lan.woken() => {}
            }
        }
        self.close_all();
        self.lan.set_open(Vec::new());
    }

    async fn apply(&mut self) {
        let want = match self.app.auth().lan() {
            Ok(on) => on,
            Err(err) => {
                // File hỏng: đóng LAN cho chắc, controller vẫn chạy trên máy này.
                tracing::warn!(%err, "không đọc được cờ LAN, đóng listener LAN");
                false
            }
        };
        if !want {
            self.close_all();
            self.last_scan = None;
            self.lan.set_open(Vec::new());
            return;
        }
        if self.last_scan.is_none_or(|at| at.elapsed() >= RESCAN) {
            self.scanned = (self.addrs)();
            self.last_scan = Some(Instant::now());
        }
        let target = self.scanned.clone();
        self.open.retain(|ip, listener| {
            let keep = target.contains(ip);
            if !keep {
                tracing::info!(%ip, "đóng listener LAN: địa chỉ không còn");
                listener.close();
            }
            keep
        });
        for ip in target {
            if !self.open.contains_key(&ip) {
                self.open_one(ip).await;
            }
        }
        self.lan.set_open(self.open_addrs());
    }

    async fn open_one(&mut self, ip: Ipv4Addr) {
        let addr = SocketAddr::from((ip, self.port));
        let listener = match TcpListener::bind(addr).await {
            Ok(listener) => listener,
            Err(err) => {
                // Thử lại ở lần quét sau; trong lúc đó `lan_urls` không có địa chỉ này.
                tracing::warn!(%addr, %err, "không mở được listener LAN");
                return;
            }
        };
        let (stop, stopped) = oneshot::channel::<()>();
        let service = self
            .router
            .clone()
            .into_make_service_with_connect_info::<SocketAddr>();
        tokio::spawn(async move {
            let server = axum::serve(listener, service).with_graceful_shutdown(async {
                let _ = stopped.await;
            });
            if let Err(err) = server.await {
                tracing::warn!(%addr, %err, "listener LAN dừng bất thường");
            }
        });
        tracing::info!(url = %format!("http://{addr}"), "đã mở giao diện ra LAN");
        self.open.insert(ip, Listener { stop: Some(stop) });
    }

    fn open_addrs(&self) -> Vec<SocketAddr> {
        let mut addrs: Vec<SocketAddr> = self
            .open
            .keys()
            .map(|ip| SocketAddr::from((*ip, self.port)))
            .collect();
        addrs.sort();
        addrs
    }

    fn close_all(&mut self) {
        if !self.open.is_empty() {
            tracing::info!("đóng giao diện LAN");
        }
        for (_, mut listener) in self.open.drain() {
            listener.close();
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use mh_platform::DataDirs;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Địa chỉ loopback khác `127.0.0.1`: đóng vai IP LAN mà không cần mạng thật trên máy CI.
    const FAKE_LAN: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 2);

    async fn get_session(addr: SocketAddr) -> std::io::Result<String> {
        let mut stream = tokio::net::TcpStream::connect(addr).await?;
        let request = format!(
            "GET /api/v1/auth/session HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
        );
        stream.write_all(request.as_bytes()).await?;
        let mut response = String::new();
        stream.read_to_string(&mut response).await?;
        Ok(response)
    }

    async fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
        let start = Instant::now();
        while !done() {
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "chờ quá lâu: {what}"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    fn free_port() -> u16 {
        let probe = std::net::TcpListener::bind((FAKE_LAN, 0)).unwrap();
        probe.local_addr().unwrap().port()
    }

    #[tokio::test]
    async fn listener_follows_lan_flag() {
        let tmp = tempfile::tempdir().unwrap();
        let dirs = DataDirs::resolve(Some(tmp.path().to_path_buf())).unwrap();
        dirs.ensure().unwrap();
        let app = Arc::new(App::new(dirs).unwrap());
        let lan = LanState::new();
        let port = free_port();
        let router = mh_api::router_with_lan(Arc::clone(&app), port, Arc::clone(&lan));
        let addr = SocketAddr::from((FAKE_LAN, port));
        let manager = LanManager::spawn(
            Arc::clone(&app),
            router,
            Arc::clone(&lan),
            port,
            Arc::new(|| vec![FAKE_LAN]),
        );

        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(
            get_session(addr).await.is_err(),
            "LAN tắt thì không có listener"
        );

        app.auth().set_password(None, "correct-horse").unwrap();
        app.auth().set_lan(true).unwrap();
        wait_until("mở listener", || lan.open_addrs() == vec![addr]).await;
        let response = get_session(addr).await.unwrap();
        assert!(response.starts_with("HTTP/1.1 200"), "{response}");
        assert!(response.contains("\"password_set\":true"), "{response}");

        app.auth().set_lan(false).unwrap();
        wait_until("đóng listener", || lan.open_addrs().is_empty()).await;
        let start = Instant::now();
        while get_session(addr).await.is_ok() {
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "listener LAN không đóng"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        manager.stop().await;
    }
}
