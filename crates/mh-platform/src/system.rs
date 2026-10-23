use std::path::{Path, PathBuf};

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
    pub memory_total_bytes: u64,
    pub memory_available_bytes: u64,
    /// Ổ đĩa chứa thư mục dữ liệu của MineHost, nếu tìm được.
    pub data_disk: Option<DiskSpace>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskSpace {
    pub mount_point: PathBuf,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

/// Đọc thông tin máy. Hàm này chặn luồng vài mili giây, nên gọi trong `spawn_blocking`.
pub fn probe(data_dir: &Path) -> SystemInfo {
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();

    let cpu_brand = sys
        .cpus()
        .first()
        .map(|cpu| cpu.brand().trim().to_owned())
        .unwrap_or_default();

    SystemInfo {
        os: std::env::consts::OS.to_owned(),
        os_version: System::long_os_version(),
        arch: std::env::consts::ARCH.to_owned(),
        cpu_brand,
        cpu_threads: sys.cpus().len(),
        memory_total_bytes: sys.total_memory(),
        memory_available_bytes: sys.available_memory(),
        data_disk: disk_for(data_dir),
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
        assert_eq!(info.os, std::env::consts::OS);
    }

    #[test]
    fn free_space_and_memory_are_known_for_current_dir() {
        let dir = std::env::current_dir().unwrap();
        assert!(free_space(&dir).is_some_and(|b| b > 0));
        assert!(total_memory_bytes() > 0);
    }
}
