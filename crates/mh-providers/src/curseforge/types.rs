//! Kiểu dữ liệu cho CurseForge Core API v1.

use std::fmt;

use mh_core::{LoaderKind, McVersion};
use mh_net::Checksum;
use serde::Deserialize;
use time::OffsetDateTime;

use super::{CLASS_MODPACK, DEFAULT_PAGE_SIZE};

/// Lớp nội dung dùng trong P3 (plugin Bukkit `5` không dùng trong P3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClassId {
    Mod,
    Modpack,
}

impl ClassId {
    pub const fn as_u32(self) -> u32 {
        match self {
            Self::Mod => super::CLASS_MOD,
            Self::Modpack => CLASS_MODPACK,
        }
    }

    pub const fn url_segment(self) -> &'static str {
        match self {
            Self::Mod => "mc-mods",
            Self::Modpack => "modpacks",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SortField {
    Featured = 1,
    #[default]
    Popularity = 2,
    LastUpdated = 3,
    Name = 4,
    Author = 5,
    TotalDownloads = 6,
    Category = 7,
    GameVersion = 8,
    EarlyAccess = 9,
    FeaturedReleased = 10,
    ReleasedDate = 11,
    Rating = 12,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchFilters {
    pub class_id: Option<ClassId>,
    pub loader: Option<LoaderKind>,
    pub minecraft: Option<McVersion>,
    pub sort_field: SortField,
    pub index: u32,
    pub page_size: u32,
}

impl Default for SearchFilters {
    fn default() -> Self {
        Self {
            class_id: None,
            loader: None,
            minecraft: None,
            sort_field: SortField::default(),
            index: 0,
            page_size: DEFAULT_PAGE_SIZE,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    pub index: u32,
    pub page_size: u32,
    pub result_count: u32,
    pub total_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub hits: Vec<ModInfo>,
    pub pagination: Pagination,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModLogo {
    #[serde(default, deserialize_with = "crate::non_empty_opt")]
    pub thumbnail_url: Option<String>,
    #[serde(default, deserialize_with = "crate::non_empty_opt")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModLinks {
    #[serde(default, deserialize_with = "crate::non_empty_opt")]
    pub website_url: Option<String>,
    #[serde(default, deserialize_with = "crate::non_empty_opt")]
    pub wiki_url: Option<String>,
    #[serde(default, deserialize_with = "crate::non_empty_opt")]
    pub issues_url: Option<String>,
    #[serde(default, deserialize_with = "crate::non_empty_opt")]
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModInfo {
    pub id: u64,
    pub name: String,
    pub slug: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub class_id: Option<u32>,
    #[serde(default, deserialize_with = "deserialize_u64_lenient")]
    pub download_count: u64,
    #[serde(default)]
    pub allow_mod_distribution: Option<bool>,
    #[serde(default)]
    pub logo: Option<ModLogo>,
    #[serde(default)]
    pub links: ModLinks,
}

impl ModInfo {
    /// Chỉ cho phép tải qua API khi `allowModDistribution` là `true`.
    pub const fn allows_distribution(&self) -> bool {
        matches!(self.allow_mod_distribution, Some(true))
    }

    pub const fn class_segment(&self) -> &'static str {
        match self.class_id {
            Some(CLASS_MODPACK) => ClassId::Modpack.url_segment(),
            _ => ClassId::Mod.url_segment(),
        }
    }

    pub fn download_page_url(&self, file_id: u64) -> String {
        format!(
            "https://www.curseforge.com/minecraft/{}/{}/download/{file_id}",
            self.class_segment(),
            self.slug
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelationType {
    EmbeddedLibrary,
    OptionalDependency,
    RequiredDependency,
    Tool,
    Incompatible,
    Include,
    Unknown(u32),
}

impl RelationType {
    pub const fn from_raw(raw: u32) -> Self {
        match raw {
            1 => Self::EmbeddedLibrary,
            2 => Self::OptionalDependency,
            3 => Self::RequiredDependency,
            4 => Self::Tool,
            5 => Self::Incompatible,
            6 => Self::Include,
            other => Self::Unknown(other),
        }
    }

    pub const fn is_required(self) -> bool {
        matches!(self, Self::RequiredDependency)
    }
}

impl<'de> Deserialize<'de> for RelationType {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        u32::deserialize(d).map(Self::from_raw)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReleaseType {
    Release,
    Beta,
    Alpha,
    Unknown(u32),
}

impl ReleaseType {
    pub const fn from_raw(raw: u32) -> Self {
        match raw {
            1 => Self::Release,
            2 => Self::Beta,
            3 => Self::Alpha,
            other => Self::Unknown(other),
        }
    }
}

impl<'de> Deserialize<'de> for ReleaseType {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        u32::deserialize(d).map(Self::from_raw)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDependency {
    pub mod_id: u64,
    pub relation_type: RelationType,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModFile {
    pub id: u64,
    pub mod_id: u64,
    pub display_name: String,
    pub file_name: String,
    pub release_type: ReleaseType,
    #[serde(with = "time::serde::rfc3339")]
    pub file_date: OffsetDateTime,
    pub file_length: u64,
    #[serde(default, deserialize_with = "crate::non_empty_opt")]
    pub download_url: Option<String>,
    #[serde(skip)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<FileDependency>,
    #[serde(default)]
    pub is_server_pack: bool,
    #[serde(default)]
    pub server_pack_file_id: Option<u64>,
    /// `true` khi `allowModDistribution = false` hoặc `downloadUrl` rỗng (spec P3, mục 3.6).
    #[serde(skip)]
    pub manual_download: bool,
    /// Link trang tải CurseForge (`https://www.curseforge.com/minecraft/<lớp>/<slug>/download/<fileId>`).
    #[serde(skip)]
    pub download_page_url: Option<String>,
}

impl ModFile {
    pub fn checksum(&self) -> Option<Checksum> {
        self.sha1.as_deref().and_then(Checksum::sha1)
    }

    /// URL tải qua API; trả `None` nếu file bị đánh dấu không tải được qua API.
    pub fn api_download_url(&self) -> Option<&str> {
        (!self.manual_download)
            .then_some(self.download_url.as_deref())
            .flatten()
    }

    pub fn required_dependencies(&self) -> impl Iterator<Item = u64> + '_ {
        self.dependencies
            .iter()
            .filter(|d| d.relation_type.is_required())
            .map(|d| d.mod_id)
    }

    /// Cập nhật cờ phân phối và link trang tải từ thông tin mod (hữu ích sau `files_by_id`).
    pub fn apply_mod(&mut self, mod_info: &ModInfo) {
        if !mod_info.allows_distribution() {
            self.manual_download = true;
        }
        self.download_page_url = Some(mod_info.download_page_url(self.id));
    }
}

#[derive(Deserialize)]
pub(super) struct RawFileHash {
    pub value: String,
    pub algo: u32,
}

#[derive(Deserialize)]
pub(super) struct RawFile {
    #[serde(flatten)]
    pub file: ModFile,
    #[serde(default)]
    pub hashes: Vec<RawFileHash>,
}

fn deserialize_u64_lenient<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    struct Visitor;
    impl serde::de::Visitor<'_> for Visitor {
        type Value = u64;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("số nguyên hoặc số thực không âm")
        }
        fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<u64, E> {
            Ok(v)
        }
        fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<u64, E> {
            u64::try_from(v).map_err(|_| E::custom("số âm"))
        }
        fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<u64, E> {
            if v.is_finite() && v >= 0.0 && v <= u64::MAX as f64 {
                Ok(v as u64)
            } else {
                Err(E::custom("số không hợp lệ"))
            }
        }
    }
    d.deserialize_any(Visitor)
}
