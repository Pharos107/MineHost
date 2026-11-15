//! Phần chung của mọi Job cập nhật và đổi phiên bản (spec P3, mục 3.9): tắt server nếu đang
//! chạy, tạo bản `before_update` rồi mới đổi gì. Không tự bật lại server, không tự khôi phục khi
//! lỗi: lỗi trả kèm `backup_id` để người dùng biết bản nào để quay về.

use mh_core::ServerId;

use crate::backup::{BackupKind, BackupMeta, BackupRequest};
use crate::jobs::{JobContext, JobStep};
use crate::restore::server_is_live;
use crate::{App, AppError};

impl App {
    /// Tắt server rồi sao lưu. `first` là số thứ tự bước đầu, `steps` là tổng số bước của Job.
    /// Trả bản `before_update` và số thứ tự bước tiếp theo.
    pub(crate) async fn prepare_update(
        &self,
        ctx: &JobContext,
        id: ServerId,
        first: u8,
        steps: u8,
    ) -> Result<(BackupMeta, u8), AppError> {
        let handle = self.handle(id)?;
        let mut index = first;
        if server_is_live(&handle) {
            ctx.step(JobStep::StopServer, index, steps);
            self.stop_for_change(&handle).await?;
            index += 1;
        }
        ctx.check_cancel()?;
        ctx.step(JobStep::BeforeUpdate, index, steps);
        let spec = self.store.load(id)?;
        self.ensure_disk(&self.store.paths(id).server_dir).await?;
        ctx.check_cancel()?;
        let req = BackupRequest {
            note: None,
            protected: true,
            kind: BackupKind::BeforeUpdate,
        };
        let backup = self.archive_world(ctx, &spec, &req).await?;
        Ok((backup, index + 1))
    }
}

/// Thêm `backup_id` vào lỗi của bước sau sao lưu, giữ nguyên các tham số đã có.
pub(crate) fn with_backup(mut err: AppError, backup: &BackupMeta) -> AppError {
    let id = serde_json::Value::String(backup.id.clone());
    match &mut err.params {
        serde_json::Value::Object(map) => {
            map.insert("backup_id".to_owned(), id);
        }
        other => {
            *other = serde_json::json!({ "backup_id": id });
        }
    }
    err
}
