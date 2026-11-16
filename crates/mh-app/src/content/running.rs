//! Thêm mod khi server đang chạy (spec trình duyệt mod, mục 2).
//!
//! Process đang chạy đã nạp xong danh sách jar nên file mới chỉ có hiệu lực ở lần khởi động sau.
//! Nhớ file theo `pid` của process lúc thêm: process mới có `pid` khác nên nhãn "chờ khởi động lại"
//! tự mất mà không cần nghe sự kiện của supervisor. Controller tắt thì server cũng tắt theo, nên giữ
//! trong bộ nhớ là đủ.

use std::collections::{BTreeSet, HashMap};

use mh_core::{ContentKind, ErrorCode, InstanceSpec, ServerId};

use super::content_kind_for;
use crate::{App, AppError, lock};

/// File đã thêm khi server có process, theo `pid` của process đó.
pub(crate) type AddedWhileRunning = HashMap<ServerId, HashMap<String, u32>>;

impl App {
    /// Cho phép thêm file mới cả khi server đang chạy; chỉ chặn khi server đang được cài.
    pub(crate) fn ensure_content_addable(
        &self,
        id: ServerId,
    ) -> Result<(InstanceSpec, ContentKind), AppError> {
        let view = self.get(id)?;
        if view.installing.is_some() {
            return Err(AppError::new(
                ErrorCode::ServerBusy,
                "không thêm mod khi server đang được cài",
            ));
        }
        let spec = view
            .spec
            .ok_or_else(|| AppError::new(ErrorCode::ConfigInvalid, "server hỏng cấu hình"))?;
        let kind = content_kind_for(&spec)?;
        Ok((spec, kind))
    }

    /// Kiểm tra nhanh trước khi nhận upload: server tồn tại, hỗ trợ mod/plugin, không đang cài.
    pub fn check_content_addable(&self, id: ServerId) -> Result<(), AppError> {
        self.ensure_content_addable(id).map(|_| ())
    }

    /// `pid` của process đang chạy, `None` khi server không có process.
    pub(crate) fn running_pid(&self, id: ServerId) -> Option<u32> {
        let view = self.get(id).ok()?;
        if !view.state.has_process() {
            return None;
        }
        view.status.and_then(|s| s.pid)
    }

    /// Ghi nhận các file vừa thêm; không làm gì khi server không chạy.
    pub(crate) fn mark_added_while_running(
        &self,
        id: ServerId,
        names: impl IntoIterator<Item = String>,
    ) {
        let Some(pid) = self.running_pid(id) else {
            return;
        };
        let mut added = lock(&self.content_added);
        let entry = added.entry(id).or_default();
        entry.retain(|_, p| *p == pid);
        entry.extend(names.into_iter().map(|n| (n, pid)));
    }

    /// Tên file đã thêm khi process hiện tại đang chạy, tức là process này chưa nạp.
    pub(crate) fn content_needs_restart(&self, id: ServerId) -> BTreeSet<String> {
        let pid = self.running_pid(id);
        let mut added = lock(&self.content_added);
        let Some(pid) = pid else {
            added.remove(&id);
            return BTreeSet::new();
        };
        let Some(entry) = added.get_mut(&id) else {
            return BTreeSet::new();
        };
        entry.retain(|_, p| *p == pid);
        entry.keys().cloned().collect()
    }
}

/// Lỗi khi thao tác phải thay một jar mà server đang mở.
pub(crate) fn replace_while_running() -> AppError {
    AppError::new(
        ErrorCode::ServerBusy,
        "không thay mod server đang dùng; tắt server trước",
    )
}
