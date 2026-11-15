//! Đọc metadata mod/plugin bên trong một jar, kể cả jar lồng (jar-in-jar).
//!
//! Jar lồng được đọc vào RAM rồi mở như zip con; có giới hạn độ sâu, kích thước và số lượng để
//! một jar hỏng hay cố ý phình to không làm treo hoặc cạn RAM khi quét.

use std::io::{Cursor, Read, Seek};

use mh_core::{ModMeta, ModMetaError, ModMetaFormat, parse_jarjar_metadata, parse_mod_meta};
use zip::ZipArchive;
use zip::result::ZipError;

pub(super) const MAX_META_BYTES: usize = 1024 * 1024;
/// Jar lồng lớn nhất chịu đọc vào RAM (thư viện lồng thật thường dưới vài MB).
const MAX_NESTED_JAR_BYTES: u64 = 64 * 1024 * 1024;
/// Fabric API lồng một tầng; vài mod lồng thư viện có jar lồng riêng. Ba tầng là đủ.
const MAX_NESTED_DEPTH: usize = 3;
/// Tổng số mod lồng tối đa đọc từ một jar (Fabric API có khoảng 50).
const MAX_BUNDLED: usize = 512;
const JARJAR_METADATA: &str = "META-INF/jarjar/metadata.json";

pub(super) const META_PRIORITY: [&str; 6] = [
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

/// Metadata đọc được từ một jar.
#[derive(Debug, Clone)]
pub(super) struct JarMetadata {
    /// Các file metadata có trong jar, theo thứ tự `META_PRIORITY`.
    pub formats: Vec<ModMetaFormat>,
    /// Metadata chính: định dạng đầu tiên theo `META_PRIORITY`.
    pub meta: Result<ModMeta, ContentMetaError>,
    /// Mọi metadata đọc được, mỗi định dạng một bản, để preflight chọn đúng bản loader dùng.
    pub format_metas: Vec<(ModMetaFormat, ModMeta)>,
    /// Mod trong jar lồng (mọi tầng, mọi định dạng).
    pub bundled: Vec<ModMeta>,
}

pub(super) fn read_jar_metadata<R: Read + Seek>(reader: R) -> JarMetadata {
    let mut archive = match ZipArchive::new(reader) {
        Ok(a) => a,
        Err(e) => {
            return JarMetadata {
                formats: Vec::new(),
                meta: Err(ContentMetaError::InvalidArchive(e.to_string())),
                format_metas: Vec::new(),
                bundled: Vec::new(),
            };
        }
    };
    let (formats, meta, format_metas) = read_formats(&mut archive);
    let mut bundled = Vec::new();
    collect_bundled(&mut archive, &format_metas, 1, &mut bundled);
    JarMetadata {
        formats,
        meta,
        format_metas,
        bundled,
    }
}

type FormatsRead = (
    Vec<ModMetaFormat>,
    Result<ModMeta, ContentMetaError>,
    Vec<(ModMetaFormat, ModMeta)>,
);

fn read_formats<R: Read + Seek>(archive: &mut ZipArchive<R>) -> FormatsRead {
    let mut formats = Vec::new();
    let mut primary = None;
    let mut format_metas = Vec::new();
    for entry_name in META_PRIORITY {
        let Some(format) = ModMetaFormat::from_archive_path(entry_name) else {
            continue;
        };
        let text = match read_zip_text(archive, entry_name) {
            Ok(Some(text)) => text,
            Ok(None) => continue,
            Err(err) => {
                primary.get_or_insert(Err(err));
                continue;
            }
        };
        formats.push(format);
        let mut parsed = parse_mod_meta(format, &text).map_err(ContentMetaError::from);
        if let Ok(meta) = &mut parsed
            && meta.version.is_none()
        {
            meta.version = read_manifest_impl_version(archive);
        }
        if let Ok(meta) = &parsed {
            format_metas.push((format, meta.clone()));
        }
        primary.get_or_insert(parsed);
    }
    (
        formats,
        primary.unwrap_or(Err(ContentMetaError::MissingMetadata)),
        format_metas,
    )
}

/// Đọc đệ quy các jar lồng mà metadata (hoặc `META-INF/jarjar/metadata.json`) khai báo.
fn collect_bundled<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    metas: &[(ModMetaFormat, ModMeta)],
    depth: usize,
    out: &mut Vec<ModMeta>,
) {
    if depth > MAX_NESTED_DEPTH {
        return;
    }
    let mut paths: Vec<String> = metas
        .iter()
        .flat_map(|(_, m)| m.nested_jars.iter().cloned())
        .collect();
    if let Ok(Some(text)) = read_zip_text(archive, JARJAR_METADATA)
        && let Ok(list) = parse_jarjar_metadata(&text)
    {
        paths.extend(list);
    }
    paths.sort();
    paths.dedup();
    for path in paths {
        if out.len() >= MAX_BUNDLED {
            return;
        }
        let Some(bytes) = read_nested_bytes(archive, &path) else {
            continue;
        };
        let Ok(mut inner) = ZipArchive::new(Cursor::new(bytes)) else {
            continue;
        };
        let (_, _, inner_metas) = read_formats(&mut inner);
        out.extend(inner_metas.iter().map(|(_, m)| m.clone()));
        collect_bundled(&mut inner, &inner_metas, depth + 1, out);
    }
}

fn read_nested_bytes<R: Read + Seek>(archive: &mut ZipArchive<R>, path: &str) -> Option<Vec<u8>> {
    let entry = archive.by_name(path).ok()?;
    if entry.is_dir() || entry.size() > MAX_NESTED_JAR_BYTES {
        return None;
    }
    let mut buf = Vec::new();
    entry
        .take(MAX_NESTED_JAR_BYTES + 1)
        .read_to_end(&mut buf)
        .ok()?;
    (buf.len() as u64 <= MAX_NESTED_JAR_BYTES).then_some(buf)
}

fn read_zip_text<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
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

fn read_manifest_impl_version<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Option<String> {
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

#[cfg(test)]
#[path = "jar_tests.rs"]
mod tests;
