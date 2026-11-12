//! Ánh xạ `instance.toml`, nguồn dữ liệu chính của một server (spec P1, mục 4.1).

use std::fmt;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use toml::Table;

use crate::backup_schedule::{BackupSchedule, DEFAULT_KEEP};
use crate::error::{ErrorCode, InvalidValue};
use crate::ids::{GamePort, ServerId, ServerName};
use crate::memory::MemoryLimits;
use crate::schedule::DailyTime;
use crate::version::{LoaderKind, McVersion};

/// Bản schema mà MineHost này đọc và ghi.
pub const CURRENT_SCHEMA: u32 = 1;

/// Nội dung `instance.toml`.
///
/// Mỗi bảng có `extra` giữ các khóa lạ: bản MineHost mới hơn có thể thêm khóa, bản cũ
/// đọc rồi ghi lại không được làm mất chúng.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstanceSpec {
    /// Luôn bằng `CURRENT_SCHEMA` vì `from_toml` từ chối mọi giá trị khác.
    schema: u32,
    pub id: ServerId,
    pub name: ServerName,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    pub game: GameSection,
    pub runtime: RuntimeSection,
    pub network: NetworkSection,
    #[serde(default)]
    pub policy: PolicySection,
    /// Sao lưu tự động (spec P2, mục 4.1). Không ghi khi để mặc định, để file P1 không đổi.
    #[serde(default, skip_serializing_if = "BackupSection::is_default")]
    pub backup: BackupSection,
    #[serde(default, skip_serializing_if = "EulaSection::is_empty")]
    pub eula: EulaSection,
    #[serde(flatten)]
    pub extra: Table,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameSection {
    pub loader: LoaderKind,
    pub minecraft: McVersion,
    /// Build của Paper; loại server không có số build thì để trống.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<u32>,
    pub jar: JarName,
    /// Lấy từ `javaVersion.majorVersion` trong manifest của Mojang.
    pub java_major: u16,
    #[serde(flatten)]
    pub extra: Table,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeSection {
    pub memory_mb: MemoryLimits,
    /// Chỉ sửa được trong chế độ Nâng cao.
    #[serde(default)]
    pub jvm_args: Vec<String>,
    #[serde(flatten)]
    pub extra: Table,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NetworkSection {
    /// Nguồn chính của cổng; controller ghi đè `server-port` trong `server.properties` mỗi lần bật.
    pub port: GamePort,
    #[serde(flatten)]
    pub extra: Table,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicySection {
    #[serde(default)]
    pub autostart: bool,
    #[serde(default = "default_restart_on_crash")]
    pub restart_on_crash: bool,
    /// Giờ khởi động lại hằng ngày (giờ máy), tối đa 4 mốc, đã sắp xếp (spec P2, mục 4.1).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub restart_at: Vec<DailyTime>,
    #[serde(flatten)]
    pub extra: Table,
}

const fn default_restart_on_crash() -> bool {
    true
}

impl Default for PolicySection {
    fn default() -> Self {
        Self {
            autostart: false,
            restart_on_crash: default_restart_on_crash(),
            restart_at: Vec::new(),
            extra: Table::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackupSection {
    #[serde(default)]
    pub schedule: BackupSchedule,
    /// Số bản `scheduled` giữ lại, 1..=100. Bản tạo tay và bản được bảo vệ không tính.
    #[serde(default = "default_keep", deserialize_with = "keep_in_range")]
    pub keep: u16,
    #[serde(flatten)]
    pub extra: Table,
}

const fn default_keep() -> u16 {
    DEFAULT_KEEP
}

fn keep_in_range<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u16, D::Error> {
    let keep = u16::deserialize(deserializer)?;
    crate::backup_schedule::check_keep(keep).map_err(serde::de::Error::custom)
}

impl Default for BackupSection {
    fn default() -> Self {
        Self {
            schedule: BackupSchedule::Off,
            keep: DEFAULT_KEEP,
            extra: Table::new(),
        }
    }
}

impl BackupSection {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EulaSection {
    /// Không có giá trị nghĩa là người dùng chưa bấm đồng ý EULA.
    #[serde(
        default,
        with = "time::serde::rfc3339::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub accepted_at: Option<OffsetDateTime>,
    #[serde(flatten)]
    pub extra: Table,
}

impl EulaSection {
    fn is_empty(&self) -> bool {
        self.accepted_at.is_none() && self.extra.is_empty()
    }
}

/// Lỗi khi đọc hoặc ghi `instance.toml`.
#[derive(Debug, thiserror::Error)]
pub enum InstanceError {
    #[error("instance.toml không hợp lệ: {0}")]
    Parse(#[source] toml::de::Error),
    #[error("instance.toml thiếu `schema` hoặc `schema` không hợp lệ")]
    BadSchema,
    /// Server được đánh dấu `Broken` với lý do "Tạo bởi bản MineHost mới hơn".
    #[error("instance.toml có schema {found}, bản MineHost này chỉ đọc được tới {CURRENT_SCHEMA}")]
    NewerSchema { found: i64 },
    #[error("không tạo được nội dung instance.toml: {0}")]
    Serialize(#[source] toml::ser::Error),
}

impl InstanceError {
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::Parse(_) | Self::BadSchema | Self::NewerSchema { .. } => ErrorCode::ConfigInvalid,
            Self::Serialize(_) => ErrorCode::Internal,
        }
    }
}

/// Chỉ đọc `schema`, để từ chối file của bản mới hơn trước khi các bảng khác làm parse lỗi.
#[derive(Deserialize)]
struct SchemaProbe {
    schema: Option<i64>,
}

impl InstanceSpec {
    /// Server mới chưa đồng ý EULA, chính sách mặc định.
    pub fn new(
        id: ServerId,
        name: ServerName,
        created_at: OffsetDateTime,
        game: GameSection,
        memory_mb: MemoryLimits,
        port: GamePort,
    ) -> Self {
        Self {
            schema: CURRENT_SCHEMA,
            id,
            name,
            created_at,
            game,
            runtime: RuntimeSection {
                memory_mb,
                jvm_args: Vec::new(),
                extra: Table::new(),
            },
            network: NetworkSection {
                port,
                extra: Table::new(),
            },
            policy: PolicySection::default(),
            backup: BackupSection::default(),
            eula: EulaSection::default(),
            extra: Table::new(),
        }
    }

    pub fn from_toml(text: &str) -> Result<Self, InstanceError> {
        let probe: SchemaProbe = toml::from_str(text).map_err(InstanceError::Parse)?;
        match probe.schema {
            Some(found) if found > i64::from(CURRENT_SCHEMA) => {
                return Err(InstanceError::NewerSchema { found });
            }
            Some(found) if found == i64::from(CURRENT_SCHEMA) => {}
            _ => return Err(InstanceError::BadSchema),
        }
        toml::from_str(text).map_err(InstanceError::Parse)
    }

    pub fn to_toml(&self) -> Result<String, InstanceError> {
        toml::to_string(self).map_err(InstanceError::Serialize)
    }

    pub fn eula_accepted(&self) -> bool {
        self.eula.accepted_at.is_some()
    }
}

impl GameSection {
    pub fn new(
        loader: LoaderKind,
        minecraft: McVersion,
        build: Option<u32>,
        java_major: u16,
    ) -> Self {
        Self {
            loader,
            minecraft,
            build,
            jar: JarName::server_jar(),
            java_major,
            extra: Table::new(),
        }
    }
}

/// Tên file jar trong thư mục `server/`. Chỉ là tên file, không chứa đường dẫn.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct JarName(String);

impl JarName {
    pub fn server_jar() -> Self {
        Self("server.jar".to_owned())
    }

    pub fn parse(s: &str) -> Result<Self, InvalidValue> {
        let err = |reason| Err(InvalidValue::new("JarName", reason));
        let Some(stem) = s.strip_suffix(".jar") else {
            return err("không có đuôi .jar");
        };
        if stem.is_empty() || stem.len() > 128 {
            return err("tên rỗng hoặc quá dài");
        }
        let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_');
        if !stem.chars().all(allowed) || stem.starts_with('.') {
            return err("chỉ được là tên file, không chứa đường dẫn");
        }
        Ok(Self(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for JarName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for JarName {
    type Error = InvalidValue;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl From<JarName> for String {
    fn from(jar: JarName) -> Self {
        jar.0
    }
}

#[cfg(test)]
mod tests;
