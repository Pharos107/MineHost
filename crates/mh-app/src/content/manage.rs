//! Bật, tắt, gỡ vào thùng rác và tải lên file mod/plugin.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use mh_core::{ContentKind, ErrorCode, ServerId};
use mh_platform::SafeRoot;
use mh_store::{
    ContentFile, ContentManifest, ContentMetaError, ContentRecord, ContentSource, ServerStore,
    inspect_jar_file,
};
use time::OffsetDateTime;
use ulid::Ulid;

use crate::files::{blocking, delete_sync, exists, io_err, validate_file_name};
use crate::{App, AppError};

struct PartGuard {
    path: PathBuf,
    committed: bool,
}

impl Drop for PartGuard {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

impl App {
    /// Bật hoặc tắt một mod/plugin bằng cách chuyển file `.jar` giữa thư mục bật và thư mục tắt.
    pub async fn set_content_enabled(
        &self,
        id: ServerId,
        file_name: &str,
        enabled: bool,
    ) -> Result<(), AppError> {
        validate_jar_name(file_name)?;
        let mutex = self.content_lock(id);
        let _guard = mutex.lock().await;
        let (_, kind) = self.ensure_content_idle(id)?;
        let (store, name) = (self.store.clone(), file_name.to_owned());
        let changed = blocking(move || toggle_sync(&store, id, kind, &name, enabled)).await?;
        if changed {
            self.notify_changed(id);
        }
        Ok(())
    }

    /// Gỡ một mod/plugin: chuyển file `.jar` vào thùng rác và bỏ bản ghi tương ứng khỏi `content.toml`.
    pub async fn remove_content(&self, id: ServerId, file_name: &str) -> Result<(), AppError> {
        validate_jar_name(file_name)?;
        let mutex = self.content_lock(id);
        let _guard = mutex.lock().await;
        let (_, kind) = self.ensure_content_idle(id)?;
        let (store, trash, name) = (self.store.clone(), self.dirs.trash(), file_name.to_owned());
        blocking(move || remove_sync(&store, &trash, id, kind, &name)).await?;
        self.notify_changed(id);
        Ok(())
    }

    /// Nhận file `.jar` tạm đã ghi xong, kiểm tra zip và chuyển vào thư mục bật.
    pub async fn upload_content(
        &self,
        id: ServerId,
        file_name: &str,
        source_path: &Path,
    ) -> Result<ContentFile, AppError> {
        validate_jar_name(file_name)?;
        let mutex = self.content_lock(id);
        let _guard = mutex.lock().await;
        let (_, kind) = self.ensure_content_idle(id)?;
        let (store, name, src) = (
            self.store.clone(),
            file_name.to_owned(),
            source_path.to_path_buf(),
        );
        let uploaded = blocking(move || upload_sync(&store, id, kind, &name, &src)).await?;
        self.notify_changed(id);
        Ok(uploaded)
    }
}

fn validate_jar_name(name: &str) -> Result<(), AppError> {
    validate_file_name(name)?;
    if !Path::new(name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("jar"))
    {
        return Err(AppError::new(
            ErrorCode::InvalidInput,
            "chỉ chấp nhận file .jar",
        ));
    }
    Ok(())
}

fn is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_file())
}

fn toggle_sync(
    store: &ServerStore,
    id: ServerId,
    kind: ContentKind,
    file_name: &str,
    enabled: bool,
) -> Result<bool, AppError> {
    let paths = store.paths(id);
    if !paths.server_dir.is_dir() {
        return Err(AppError::not_found(file_name));
    }
    let root = SafeRoot::new(&paths.server_dir)?;
    let (from_dir, to_dir) = if enabled {
        (kind.disabled_dir(), kind.dir())
    } else {
        (kind.dir(), kind.disabled_dir())
    };
    let from = root.join(&format!("{from_dir}/{file_name}"))?;
    let to = root.join(&format!("{to_dir}/{file_name}"))?;
    if !is_regular_file(from.as_path()) {
        // Đã ở đúng trạng thái mong muốn thì coi như không cần đổi, tránh báo nhầm FILE_EXISTS.
        if is_regular_file(to.as_path()) {
            return Ok(false);
        }
        return Err(AppError::not_found(file_name));
    }
    if fs::symlink_metadata(to.as_path()).is_ok() {
        return Err(exists(&format!("{to_dir}/{file_name}")));
    }
    if let Some(parent) = to.as_path().parent() {
        fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
    }
    fs::rename(from.as_path(), to.as_path()).map_err(|e| io_err(from.as_path(), e))?;
    Ok(true)
}

fn remove_sync(
    store: &ServerStore,
    trash: &Path,
    id: ServerId,
    kind: ContentKind,
    file_name: &str,
) -> Result<(), AppError> {
    let paths = store.paths(id);
    if !paths.server_dir.is_dir() {
        return Err(AppError::not_found(file_name));
    }
    let root = SafeRoot::new(&paths.server_dir)?;
    let mut trashed = false;
    for dir in [kind.dir(), kind.disabled_dir()] {
        let rel = format!("{dir}/{file_name}");
        let abs = root.join(&rel)?;
        if is_regular_file(abs.as_path()) {
            delete_sync(store, trash, id, &rel)?;
            trashed = true;
        }
    }
    if !trashed {
        return Err(AppError::not_found(file_name));
    }
    let enabled_rel = format!("{}/{file_name}", kind.dir());
    let mut manifest = ContentManifest::load(&paths.content_file)?;
    let before = manifest.file.len();
    manifest.file.retain(|rec| rec.path != enabled_rel);
    if manifest.file.len() != before {
        manifest.save(&paths.content_file)?;
    }
    Ok(())
}

fn upload_sync(
    store: &ServerStore,
    id: ServerId,
    kind: ContentKind,
    file_name: &str,
    source_path: &Path,
) -> Result<ContentFile, AppError> {
    let (size, info) = inspect_upload_source(source_path)?;
    let paths = store.paths(id);
    let mut manifest = ContentManifest::load(&paths.content_file)?;
    // Khớp một mục `[[pending]]` thì ưu tiên tên file gốc mà pack/CurseForge mong đợi.
    let pending_name = manifest
        .pending
        .iter()
        .find(|p| p.sha1.eq_ignore_ascii_case(&info.sha1))
        .and_then(|p| pending_jar_name(&p.path));
    let target_name = pending_name.as_deref().unwrap_or(file_name);

    fs::create_dir_all(&paths.server_dir).map_err(|e| io_err(&paths.server_dir, e))?;
    let root = SafeRoot::new(&paths.server_dir)?;
    let rel = format!("{}/{target_name}", kind.dir());
    let dest = root.join(&rel)?;
    if fs::symlink_metadata(dest.as_path()).is_ok() {
        return Err(exists(&rel));
    }
    if let Some(parent) = dest.as_path().parent() {
        fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
    }
    move_file_into_place(source_path, dest.as_path())?;

    let record = save_upload_record(
        &mut manifest,
        &paths.content_file,
        dest.as_path(),
        &rel,
        &info,
    )?;
    Ok(ContentFile {
        file_name: target_name.to_owned(),
        path: rel,
        enabled: true,
        size,
        sha1: info.sha1,
        meta: info.meta,
        record: Some(record),
        manual: false,
    })
}

fn inspect_upload_source(source_path: &Path) -> Result<(u64, mh_store::JarInfo), AppError> {
    let meta = fs::symlink_metadata(source_path).map_err(|e| io_err(source_path, e))?;
    if !meta.is_file() {
        return Err(AppError::new(
            ErrorCode::InvalidInput,
            "nguồn tải lên phải là một file",
        ));
    }
    let info = inspect_jar_file(source_path)?;
    if let Err(ContentMetaError::InvalidArchive(detail)) = &info.meta {
        return Err(AppError::new(
            ErrorCode::InvalidInput,
            format!("file jar hỏng: {detail}"),
        ));
    }
    Ok((meta.len(), info))
}

fn save_upload_record(
    manifest: &mut ContentManifest,
    content_file: &Path,
    dest: &Path,
    rel: &str,
    info: &mh_store::JarInfo,
) -> Result<ContentRecord, AppError> {
    let client_only = info.meta.as_ref().is_ok_and(|m| m.side.is_client_only());
    manifest
        .pending
        .retain(|p| !p.sha1.eq_ignore_ascii_case(&info.sha1));
    manifest.file.retain(|rec| rec.path != rel);
    let record = ContentRecord {
        path: rel.to_owned(),
        sha1: info.sha1.clone(),
        source: ContentSource::Upload,
        project_id: None,
        version_id: None,
        client_only,
        from_modpack: false,
        installed_at: OffsetDateTime::now_utc(),
        extra: Default::default(),
    };
    manifest.file.push(record.clone());
    if let Err(err) = manifest.save(content_file) {
        let _ = fs::remove_file(dest);
        return Err(err.into());
    }
    Ok(record)
}

fn pending_jar_name(pending_path: &str) -> Option<String> {
    let name = Path::new(pending_path).file_name()?.to_str()?;
    validate_jar_name(name).ok()?;
    Some(name.to_owned())
}

fn move_file_into_place(source: &Path, dest: &Path) -> Result<(), AppError> {
    match fs::rename(source, dest) {
        Ok(()) => Ok(()),
        Err(e) if is_cross_device(&e) => copy_and_remove(source, dest),
        Err(e) => Err(io_err(source, e)),
    }
}

fn is_cross_device(err: &io::Error) -> bool {
    // Thư viện chuẩn ánh xạ cả `EXDEV` (Linux) lẫn `ERROR_NOT_SAME_DEVICE` (Windows) về kiểu này.
    err.kind() == io::ErrorKind::CrossesDevices
}

fn copy_and_remove(source: &Path, dest: &Path) -> Result<(), AppError> {
    let parent = dest
        .parent()
        .ok_or_else(|| AppError::new(ErrorCode::Internal, "thiếu thư mục đích"))?;
    let part_path = parent.join(format!(".upload-{}.part", Ulid::generate()));
    let mut guard = PartGuard {
        path: part_path.clone(),
        committed: false,
    };
    {
        let mut src = fs::File::open(source).map_err(|e| io_err(source, e))?;
        let mut dst = fs::File::create(&part_path).map_err(|e| io_err(&part_path, e))?;
        io::copy(&mut src, &mut dst).map_err(|e| io_err(&part_path, e))?;
        dst.sync_all().map_err(|e| io_err(&part_path, e))?;
    }
    if fs::symlink_metadata(dest).is_ok() {
        return Err(exists(&dest.display().to_string()));
    }
    fs::rename(&part_path, dest).map_err(|e| io_err(&part_path, e))?;
    guard.committed = true;
    if let Err(e) = fs::remove_file(source)
        && e.kind() != io::ErrorKind::NotFound
    {
        tracing::warn!(path = %source.display(), error = %e, "không xóa được file tạm upload");
    }
    Ok(())
}
