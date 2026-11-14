//! Quét thư mục `mods/` + `mods-disabled/` (hoặc `plugins/` + `plugins-disabled/`),
//! đọc metadata từ jar, tính SHA-1, cache theo `(path, mtime, size)` và đồng bộ `content.toml`.

use std::collections::{HashMap, HashSet};
use std::fs::{self, DirEntry, File};
use std::io::{self, Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use mh_core::{ContentKind, ModMeta, ModMetaError, ModMetaFormat, parse_mod_meta};
use mh_platform::SafeRoot;
use sha1::{Digest, Sha1};
use zip::ZipArchive;
use zip::result::ZipError;

use super::{CONTENT_FILE, ContentManifest, ContentRecord};
use crate::StoreError;

const MAX_META_BYTES: usize = 1024 * 1024;
const META_PRIORITY: [&str; 6] = [
    "quilt.mod.json",
    "fabric.mod.json",
    "META-INF/neoforge.mods.toml",
    "META-INF/mods.toml",
    "paper-plugin.yml",
    "plugin.yml",
];

/// Lỗi khi đọc metadata mod/plugin từ một file jar cụ thể.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ContentMetaError {
    #[error("jar không phải file zip hợp lệ: {0}")]
    InvalidArchive(String),
    #[error("jar không có file metadata mod/plugin")]
    MissingMetadata,
    #[error("file metadata trong jar lớn quá {MAX_META_BYTES} byte")]
    MetadataTooLarge,
    #[error("file metadata không phải UTF-8 hợp lệ")]
    InvalidUtf8,
    #[error(transparent)]
    Parse(#[from] ModMetaError),
}

/// Thông tin một file `.jar` sau khi quét đĩa và ghép với `content.toml`.
#[derive(Debug, Clone, PartialEq)]
pub struct ContentFile {
    pub file_name: String,
    /// Đường dẫn tương đối với `server/` theo dạng đang bật (`mods/x.jar`).
    pub path: String,
    pub enabled: bool,
    pub size: u64,
    pub sha1: String,
    /// Các file metadata tìm thấy trong jar theo thứ tự `META_PRIORITY` (để nhận biết jar đa loader).
    pub formats: Vec<ModMetaFormat>,
    pub meta: Result<ModMeta, ContentMetaError>,
    pub record: Option<ContentRecord>,
    /// `true` khi jar không có bản ghi tương ứng trong `content.toml`.
    pub manual: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    path: PathBuf,
    mtime: SystemTime,
    size: u64,
}

#[derive(Debug, Clone)]
struct CachedJar {
    sha1: String,
    formats: Vec<ModMetaFormat>,
    meta: Result<ModMeta, ContentMetaError>,
}

/// Thông tin SHA-1 và metadata đọc từ một file `.jar`.
#[derive(Debug, Clone)]
pub struct JarInfo {
    pub sha1: String,
    pub formats: Vec<ModMetaFormat>,
    pub meta: Result<ModMeta, ContentMetaError>,
}

/// Tính SHA-1 và đọc metadata của một file `.jar` tại `path`.
pub fn inspect_jar_file(path: &Path) -> Result<JarInfo, StoreError> {
    let jar = inspect_jar(path)?;
    Ok(JarInfo {
        sha1: jar.sha1,
        formats: jar.formats,
        meta: jar.meta,
    })
}

/// Tính SHA-1 dạng hex chữ thường cho dữ liệu trong RAM.
pub fn sha1_hex(bytes: &[u8]) -> String {
    to_hex(&Sha1::digest(bytes))
}

struct ScannedJar {
    file_name: String,
    path: String,
    enabled: bool,
    size: u64,
    cached: CachedJar,
}

/// Bộ quét nội dung giữ cache trong RAM theo `(path, mtime, size)`, an toàn đa luồng.
#[derive(Debug, Default)]
pub struct ContentScanner {
    cache: Mutex<HashMap<CacheKey, CachedJar>>,
    #[cfg(test)]
    hits: AtomicU64,
    #[cfg(test)]
    misses: AtomicU64,
}

/// Quét `server_root` (thư mục `servers/<id>/`) mà không giữ cache giữa các lần gọi.
pub fn scan_content(server_root: &Path, kind: ContentKind) -> Result<Vec<ContentFile>, StoreError> {
    ContentScanner::new().scan(server_root, kind)
}

impl ContentScanner {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub(crate) fn cache_stats(&self) -> (u64, u64) {
        (
            self.hits.load(Ordering::Relaxed),
            self.misses.load(Ordering::Relaxed),
        )
    }

    /// Quét server tại `server_root` (`servers/<id>/`, chứa `server/` và `content.toml`).
    pub fn scan(
        &self,
        server_root: &Path,
        kind: ContentKind,
    ) -> Result<Vec<ContentFile>, StoreError> {
        self.scan_dirs(
            &server_root.join("server"),
            &server_root.join(CONTENT_FILE),
            kind,
        )
    }

    /// Quét trực tiếp với đường dẫn `server_dir` và `content_file` tách riêng.
    pub fn scan_dirs(
        &self,
        server_dir: &Path,
        content_file: &Path,
        kind: ContentKind,
    ) -> Result<Vec<ContentFile>, StoreError> {
        let mut jars = self.scan_disk_jars(server_dir, kind)?;
        jars.sort_by(|a, b| {
            a.file_name
                .to_ascii_lowercase()
                .cmp(&b.file_name.to_ascii_lowercase())
                .then_with(|| a.file_name.cmp(&b.file_name))
                .then_with(|| b.enabled.cmp(&a.enabled))
        });

        let mut manifest = ContentManifest::load(content_file)?;
        let (files, changed) = reconcile_records(jars, &mut manifest);
        if changed {
            manifest.save(content_file)?;
        }
        Ok(files)
    }

    fn scan_disk_jars(
        &self,
        server_dir: &Path,
        kind: ContentKind,
    ) -> Result<Vec<ScannedJar>, StoreError> {
        if !server_dir.exists() {
            return Ok(Vec::new());
        }
        let root = SafeRoot::new(server_dir)?;
        let enabled_dir = root.join(kind.dir())?;
        let disabled_dir = root.join(kind.disabled_dir())?;
        let mut jars = Vec::new();
        let mut seen = HashSet::new();
        for (dir, enabled) in [
            (enabled_dir.as_path(), true),
            (disabled_dir.as_path(), false),
        ] {
            self.scan_dir(dir, kind.dir(), enabled, &mut jars, &mut seen)?;
        }
        // Xóa cache của file đã bị gỡ/đổi tên trong hai thư mục vừa quét để RAM không phình.
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.retain(|k, _| {
            !(k.path.starts_with(enabled_dir.as_path())
                || k.path.starts_with(disabled_dir.as_path()))
                || seen.contains(k)
        });
        Ok(jars)
    }

    fn scan_dir(
        &self,
        dir: &Path,
        prefix: &str,
        enabled: bool,
        out: &mut Vec<ScannedJar>,
        seen: &mut HashSet<CacheKey>,
    ) -> Result<(), StoreError> {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(StoreError::io(dir, e)),
        };
        for entry in entries {
            let entry = entry.map_err(|e| StoreError::io(dir, e))?;
            if let Some(jar) = self.read_entry(&entry, prefix, enabled, seen)? {
                out.push(jar);
            }
        }
        Ok(())
    }

    fn read_entry(
        &self,
        entry: &DirEntry,
        prefix: &str,
        enabled: bool,
        seen: &mut HashSet<CacheKey>,
    ) -> Result<Option<ScannedJar>, StoreError> {
        let path = entry.path();
        let Ok(ftype) = skip_not_found(entry.file_type(), &path)? else {
            return Ok(None);
        };
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if ftype.is_symlink()
            || !ftype.is_file()
            || !Path::new(&file_name)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("jar"))
        {
            return Ok(None);
        }
        let Ok(meta_fs) = skip_not_found(entry.metadata(), &path)? else {
            return Ok(None);
        };
        let size = meta_fs.len();
        let key = meta_fs.modified().ok().map(|mtime| CacheKey {
            path: path.clone(),
            mtime,
            size,
        });
        let cached = self.get_or_inspect(&path, key.as_ref(), seen)?;
        Ok(Some(ScannedJar {
            path: format!("{prefix}/{file_name}"),
            file_name,
            enabled,
            size,
            cached,
        }))
    }

    fn get_or_inspect(
        &self,
        path: &Path,
        key: Option<&CacheKey>,
        seen: &mut HashSet<CacheKey>,
    ) -> Result<CachedJar, StoreError> {
        if let Some(k) = key {
            seen.insert(k.clone());
            if let Some(hit) = self.cache.lock().unwrap_or_else(|e| e.into_inner()).get(k) {
                #[cfg(test)]
                self.hits.fetch_add(1, Ordering::Relaxed);
                return Ok(hit.clone());
            }
        }
        #[cfg(test)]
        self.misses.fetch_add(1, Ordering::Relaxed);
        let inspected = inspect_jar(path)?;
        if let Some(k) = key {
            self.cache
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(k.clone(), inspected.clone());
        }
        Ok(inspected)
    }
}

fn skip_not_found<T>(res: io::Result<T>, path: &Path) -> Result<Result<T, ()>, StoreError> {
    match res {
        Ok(v) => Ok(Ok(v)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Err(())),
        Err(e) => Err(StoreError::io(path, e)),
    }
}

fn claim_record(
    records: &[ContentRecord],
    taken: &mut [bool],
    pred: impl Fn(&ContentRecord) -> bool,
) -> Option<usize> {
    let idx = records
        .iter()
        .enumerate()
        .position(|(i, r)| !taken[i] && pred(r))?;
    taken[idx] = true;
    Some(idx)
}

fn reconcile_records(
    jars: Vec<ScannedJar>,
    manifest: &mut ContentManifest,
) -> (Vec<ContentFile>, bool) {
    let mut matched = vec![None; jars.len()];
    let mut taken = vec![false; manifest.file.len()];
    let mut changed = false;

    for (ji, jar) in jars.iter().enumerate() {
        matched[ji] = claim_record(&manifest.file, &mut taken, |r| r.path == jar.path);
    }
    for (ji, jar) in jars.iter().enumerate() {
        if matched[ji].is_none()
            && let Some(ri) = claim_record(&manifest.file, &mut taken, |r| {
                !r.sha1.is_empty() && r.sha1.eq_ignore_ascii_case(&jar.cached.sha1)
            })
        {
            matched[ji] = Some(ri);
            if manifest.file[ri].path != jar.path {
                manifest.file[ri].path = jar.path.clone();
                changed = true;
            }
        }
    }
    let files = jars
        .into_iter()
        .enumerate()
        .map(|(ji, jar)| {
            let record = matched[ji].map(|ri| manifest.file[ri].clone());
            ContentFile {
                file_name: jar.file_name,
                path: jar.path,
                enabled: jar.enabled,
                size: jar.size,
                sha1: jar.cached.sha1,
                formats: jar.cached.formats,
                meta: jar.cached.meta,
                manual: record.is_none(),
                record,
            }
        })
        .collect();
    if taken.iter().any(|&used| !used) {
        let mut iter = taken.into_iter();
        manifest.file.retain(|_| iter.next().unwrap_or(false));
        changed = true;
    }
    (files, changed)
}

fn inspect_jar(path: &Path) -> Result<CachedJar, StoreError> {
    let mut file = File::open(path).map_err(|e| StoreError::io(path, e))?;
    let sha1 = compute_sha1(&mut file).map_err(|e| StoreError::io(path, e))?;
    file.rewind().map_err(|e| StoreError::io(path, e))?;
    let (formats, meta) = read_jar_metadata(file);
    Ok(CachedJar {
        sha1,
        formats,
        meta,
    })
}

fn compute_sha1(reader: &mut impl Read) -> io::Result<String> {
    let mut hasher = Sha1::new();
    let mut buf = [0u8; 16 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            return Ok(to_hex(&hasher.finalize()));
        }
        hasher.update(&buf[..n]);
    }
}

pub(super) fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn read_jar_metadata(file: File) -> (Vec<ModMetaFormat>, Result<ModMeta, ContentMetaError>) {
    let mut archive = match ZipArchive::new(file) {
        Ok(a) => a,
        Err(e) => {
            return (
                Vec::new(),
                Err(ContentMetaError::InvalidArchive(e.to_string())),
            );
        }
    };
    let mut formats = Vec::new();
    let mut primary = None;
    for entry_name in META_PRIORITY {
        let Some(format) = ModMetaFormat::from_archive_path(entry_name) else {
            continue;
        };
        let text = match read_zip_text(&mut archive, entry_name) {
            Ok(Some(text)) => text,
            Ok(None) => continue,
            Err(err) => {
                primary.get_or_insert(Err(err));
                continue;
            }
        };
        formats.push(format);
        if primary.is_none() {
            let mut parsed = parse_mod_meta(format, &text).map_err(ContentMetaError::from);
            if let Ok(meta) = &mut parsed
                && meta.version.is_none()
            {
                meta.version = read_manifest_impl_version(&mut archive);
            }
            primary = Some(parsed);
        }
    }
    (
        formats,
        primary.unwrap_or(Err(ContentMetaError::MissingMetadata)),
    )
}

fn read_zip_text(
    archive: &mut ZipArchive<File>,
    entry_name: &str,
) -> Result<Option<String>, ContentMetaError> {
    let entry = match archive.by_name(entry_name) {
        Ok(entry) => entry,
        Err(ZipError::FileNotFound) => return Ok(None),
        Err(e) => return Err(ContentMetaError::InvalidArchive(e.to_string())),
    };
    if entry.is_dir() {
        return Ok(None);
    }
    if entry.size() > MAX_META_BYTES as u64 {
        return Err(ContentMetaError::MetadataTooLarge);
    }
    let mut buf = Vec::new();
    entry
        .take((MAX_META_BYTES + 1) as u64)
        .read_to_end(&mut buf)
        .map_err(|e| ContentMetaError::InvalidArchive(e.to_string()))?;
    if buf.len() > MAX_META_BYTES {
        return Err(ContentMetaError::MetadataTooLarge);
    }
    String::from_utf8(buf)
        .map(Some)
        .map_err(|_| ContentMetaError::InvalidUtf8)
}

fn read_manifest_impl_version(archive: &mut ZipArchive<File>) -> Option<String> {
    let text = read_zip_text(archive, "META-INF/MANIFEST.MF")
        .ok()
        .flatten()?;
    // Định dạng MANIFEST.MF cho phép xuống dòng tiếp nối bằng một dấu cách ở đầu dòng kế tiếp.
    let normalized = text.replace("\r\n", "\n").replace("\n ", "");
    normalized.split(['\n', '\r']).find_map(|line| {
        let (key, val) = line.split_once(':')?;
        key.trim()
            .eq_ignore_ascii_case("Implementation-Version")
            .then(|| val.trim())
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
    })
}
