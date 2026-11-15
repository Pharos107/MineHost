//! Loại Job và các bước của nó. Tách khỏi `jobs` để danh sách dài thêm theo từng giai đoạn
//! không làm phình phần chạy Job.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    CreateServer,
    CreateModpack,
    ImportServer,
    Backup,
    Restore,
    InstallJava,
    InstallContent,
    /// Hỏi nguồn mod xem có bản mới; kết quả nằm trong `result` (spec P3, mục 5.7).
    CheckUpdates,
    UpdateContent,
    ChangeVersion,
    UpdateModpack,
}

/// Bước đang làm. UI dịch theo `kind`, ví dụ "Tải Java 21 (48/61 MB)".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JobStep {
    CheckDisk,
    ResolveVersion {
        minecraft: String,
    },
    DownloadJava {
        major: u16,
    },
    DownloadServer {
        minecraft: String,
    },
    PrepareFiles,
    /// `save-off` rồi `save-all flush` trước khi nén world đang mở.
    SaveWorld,
    Compress,
    /// Tắt hẳn server trước khi thay world. Không cắt ngang khi Job bị hủy.
    StopServer,
    /// Sao lưu hiện trạng. Server đã tắt nên không gửi lệnh save.
    BeforeRestore,
    /// Bản `before_update` trước khi cập nhật hoặc đổi phiên bản (spec P3, mục 3.9).
    BeforeUpdate,
    Unpack,
    Swap,
    /// Chép thư mục server có sẵn khi nhập.
    CopyFiles,
    /// Chạy installer của mod loader (spec P3, mục 3.1).
    RunInstaller {
        loader: String,
    },
    ResolveContent,
    DownloadContent {
        name: String,
    },
    DownloadModpack {
        name: String,
    },
    DownloadPackFile {
        name: String,
        index: usize,
        total: usize,
    },
    ApplyOverrides,
}
