use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use serde::Serialize;
use sysinfo::{Disks, System};

/// Ảnh chụp tài nguyên của máy, dùng để hiện trên UI và đề xuất RAM cho server.
#[derive(Debug, Clone, Serialize)]
pub struct SystemInfo {
    /// `windows`, `linux`, … theo `std::env::consts::OS`.
    pub os: String,
    /// Tên đầy đủ, ví dụ "Windows 11 Home" hoặc "Ubuntu 24.04".
    pub os_version: Option<String>,
    pub arch: String,
    pub cpu_brand: String,
    pub cpu_threads: usize,
    /// CPU toàn máy, từ 0 đến 100. Xem [`probe`]: lần đo đầu có thể là 0.
    pub cpu_percent: f32,
    pub memory_total_bytes: u64,
    pub memory_available_bytes: u64,
    /// RAM đang dùng, byte.
    pub memory_used_bytes: u64,
    /// Ổ đĩa chứa thư mục dữ liệu của MineHost, nếu tìm được.
    pub data_disk: Option<DiskSpace>,
    /// Byte còn trống trên ổ chứa thư mục dữ liệu. Trùng `data_disk.available_bytes`.
    pub disk_free_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskSpace {
    pub mount_point: PathBuf,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

/// Đọc thông tin máy. Hàm này chặn luồng vài mili giây, nên gọi trong `spawn_blocking`.
///
/// CPU lấy từ một [`System`] dùng chung, giữ sau `Mutex`, và được làm mới mỗi lần gọi.
/// Sysinfo tính phần trăm bằng hiệu hai lần đo: lần đầu chưa có mốc nên `cpu_percent`
/// có thể là 0; lần sau là mức dùng kể từ lần gọi trước.
pub fn probe(data_dir: &Path) -> SystemInfo {
    let sample = sample_machine();
    let data_disk = disk_for(data_dir);
    SystemInfo {
        os: std::env::consts::OS.to_owned(),
        os_version: System::long_os_version(),
        arch: std::env::consts::ARCH.to_owned(),
        cpu_brand: sample.cpu_brand,
        cpu_threads: sample.cpu_threads,
        cpu_percent: sample.cpu_percent,
        memory_total_bytes: sample.memory_total_bytes,
        memory_available_bytes: sample.memory_available_bytes,
        memory_used_bytes: sample.memory_used_bytes,
        disk_free_bytes: data_disk.as_ref().map(|disk| disk.available_bytes),
        data_disk,
    }
}

struct MachineSample {
    cpu_brand: String,
    cpu_threads: usize,
    cpu_percent: f32,
    memory_total_bytes: u64,
    memory_available_bytes: u64,
    memory_used_bytes: u64,
}

fn sample_machine() -> MachineSample {
    let mut sys = shared_machine();
    sys.refresh_memory();
    sys.refresh_cpu_all();
    let cpu_brand = sys
        .cpus()
        .first()
        .map(|cpu| cpu.brand().trim().to_owned())
        .unwrap_or_default();
    MachineSample {
        cpu_brand,
        cpu_threads: sys.cpus().len(),
        cpu_percent: finite_percent(sys.global_cpu_usage()),
        memory_total_bytes: sys.total_memory(),
        memory_available_bytes: sys.available_memory(),
        memory_used_bytes: sys.used_memory(),
    }
}

/// Cùng một `System` cho mọi lần `probe`, vì phần trăm CPU cần hai lần làm mới.
fn shared_machine() -> MutexGuard<'static, System> {
    static MACHINE: OnceLock<Mutex<System>> = OnceLock::new();
    // Poison chỉ xảy ra khi luồng giữ khóa bị panic; số đo bên trong vẫn đọc được.
    MACHINE
        .get_or_init(|| Mutex::new(System::new()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

fn finite_percent(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 100.0)
    } else {
        0.0
    }
}

/// Số byte còn trống trên ổ chứa `path`; `None` nếu không tìm được ổ.
pub fn free_space(path: &Path) -> Option<u64> {
    disk_for(path).map(|disk| disk.available_bytes)
}

/// Tổng RAM của máy, rẻ hơn `probe` vì không đọc CPU và ổ đĩa.
pub fn total_memory_bytes() -> u64 {
    let mut sys = System::new();
    sys.refresh_memory();
    sys.total_memory()
}

/// Địa chỉ IPv4 trong mạng LAN của máy, để bạn bè vào chơi.
///
/// Mở một socket UDP "kết nối" tới một địa chỉ ngoài để OS chọn card mạng mặc định; UDP
/// không gửi gói tin nào khi `connect`, nên hàm này chạy được cả khi không có Internet.
pub fn lan_ipv4() -> Vec<std::net::Ipv4Addr> {
    let probe = || -> std::io::Result<std::net::SocketAddr> {
        let socket = std::net::UdpSocket::bind(("0.0.0.0", 0))?;
        socket.connect(("10.255.255.255", 9))?;
        socket.local_addr()
    };
    match probe() {
        Ok(std::net::SocketAddr::V4(addr))
            if !addr.ip().is_loopback() && !addr.ip().is_unspecified() =>
        {
            vec![*addr.ip()]
        }
        _ => Vec::new(),
    }
}

/// Chọn ổ có điểm gắn dài nhất mà vẫn là tiền tố của `path`.
fn disk_for(path: &Path) -> Option<DiskSpace> {
    let disks = Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .filter(|disk| path.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().components().count())
        .map(|disk| DiskSpace {
            mount_point: disk.mount_point().to_path_buf(),
            total_bytes: disk.total_space(),
            available_bytes: disk.available_space(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_reports_plausible_values() {
        let dir = std::env::current_dir().unwrap();
        let info = probe(&dir);

        assert!(info.cpu_threads > 0);
        assert!(info.memory_total_bytes > 0);
        assert!(info.memory_available_bytes <= info.memory_total_bytes);
        assert!(info.memory_used_bytes <= info.memory_total_bytes);
        assert!((0.0..=100.0).contains(&info.cpu_percent));
        let again = probe(&dir);
        assert!((0.0..=100.0).contains(&again.cpu_percent));
        assert_eq!(info.os, std::env::consts::OS);
        assert_eq!(
            info.disk_free_bytes,
            info.data_disk.as_ref().map(|disk| disk.available_bytes)
        );
    }

    #[test]
    fn free_space_and_memory_are_known_for_current_dir() {
        let dir = std::env::current_dir().unwrap();
        assert!(free_space(&dir).is_some_and(|b| b > 0));
        assert!(total_memory_bytes() > 0);
    }
}
