//! File người chơi của Minecraft và `frozen.json` do MineHost ghi (spec P2, mục 4.4, 5.3).
//!
//! `whitelist.json`, `ops.json`, `banned-players.json` nằm trong `server/`. Khóa lạ nằm ở
//! [`JsonExtra`] và được ghi lại, cùng lý do `instance.toml` giữ khóa không biết. File chưa có,
//! hoặc chỉ toàn khoảng trắng, là danh sách rỗng. `usercache.json` chỉ đọc.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use mh_core::PlayerName;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::StoreError;

const WHITELIST_FILE: &str = "whitelist.json";
const OPS_FILE: &str = "ops.json";
const BANNED_FILE: &str = "banned-players.json";
const USERCACHE_FILE: &str = "usercache.json";
const FROZEN_FILE: &str = "frozen.json";

/// Khóa JSON không thuộc trường MineHost biết. Ghi lại nguyên văn khi sửa file.
pub type JsonExtra = Map<String, Value>;

/// Một dòng `whitelist.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WhitelistEntry {
    pub uuid: String,
    pub name: PlayerName,
    #[serde(flatten)]
    pub extra: JsonExtra,
}

/// Một dòng `ops.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpEntry {
    pub uuid: String,
    pub name: PlayerName,
    /// Mức op của Minecraft, thường là 4.
    pub level: i32,
    #[serde(rename = "bypassesPlayerLimit")]
    pub bypasses_player_limit: bool,
    #[serde(flatten)]
    pub extra: JsonExtra,
}

/// Một dòng `banned-players.json`.
///
/// `created` và `expires` giữ đúng chuỗi Minecraft ghi (`2026-10-27 10:00:00 +0700` hoặc
/// `forever`). Không parse: ghi lại phải khớp từng ký tự, kẻo lệch múi giờ.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BanEntry {
    pub uuid: String,
    pub name: PlayerName,
    pub created: String,
    pub source: String,
    pub expires: String,
    pub reason: String,
    #[serde(flatten)]
    pub extra: JsonExtra,
}

/// Một dòng `usercache.json`. Minecraft tự ghi file này; MineHost không ghi đè.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct UserCacheEntry {
    pub name: PlayerName,
    pub uuid: String,
    /// Ví dụ `2026-11-26 10:00:00 +0700`. Giữ chuỗi vì file này chỉ đọc.
    #[serde(rename = "expiresOn")]
    pub expires_on: String,
}

/// Tên không phân biệt hoa thường, hoặc UUID không phân biệt hoa thường.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerRef<'a> {
    Name(&'a str),
    Uuid(&'a str),
}

/// Đường dẫn file người chơi của một server.
///
/// `root` truyền vào [`PlayerFiles::from_root`] là `servers/<id>/`, không phải `server/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerFiles {
    server_dir: PathBuf,
    frozen_path: PathBuf,
}

impl WhitelistEntry {
    pub fn new(uuid: impl Into<String>, name: PlayerName) -> Self {
        Self {
            uuid: uuid.into(),
            name,
            extra: Map::new(),
        }
    }
}

impl OpEntry {
    pub fn new(
        uuid: impl Into<String>,
        name: PlayerName,
        level: i32,
        bypasses_player_limit: bool,
    ) -> Self {
        Self {
            uuid: uuid.into(),
            name,
            level,
            bypasses_player_limit,
            extra: Map::new(),
        }
    }
}

impl BanEntry {
    pub fn new(
        uuid: impl Into<String>,
        name: PlayerName,
        created: impl Into<String>,
        source: impl Into<String>,
        expires: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            uuid: uuid.into(),
            name,
            created: created.into(),
            source: source.into(),
            expires: expires.into(),
            reason: reason.into(),
            extra: Map::new(),
        }
    }
}

impl PlayerRef<'_> {
    fn matches(self, name: &PlayerName, uuid: &str) -> bool {
        match self {
            Self::Name(want) => name.as_str().eq_ignore_ascii_case(want),
            // UUID rỗng không được khớp mọi dòng thiếu uuid.
            Self::Uuid(want) => uuid_eq(uuid, want),
        }
    }
}

impl PlayerFiles {
    pub fn from_root(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            server_dir: root.join("server"),
            frozen_path: root.join(FROZEN_FILE),
        }
    }

    pub fn read_whitelist(&self) -> Result<Vec<WhitelistEntry>, StoreError> {
        read_list(&self.minecraft(WHITELIST_FILE))
    }

    pub fn write_whitelist(&self, entries: &[WhitelistEntry]) -> Result<(), StoreError> {
        write_list(&self.minecraft(WHITELIST_FILE), entries)
    }

    /// `Ok(true)` khi đã thêm. Trùng tên hoặc UUID thì không ghi bản thứ hai.
    pub fn add_whitelist(&self, entry: WhitelistEntry) -> Result<bool, StoreError> {
        add_row(&self.minecraft(WHITELIST_FILE), entry)
    }

    /// `Ok(true)` khi đã gỡ ít nhất một dòng.
    pub fn remove_whitelist(&self, player: PlayerRef<'_>) -> Result<bool, StoreError> {
        remove_row::<WhitelistEntry>(&self.minecraft(WHITELIST_FILE), player)
    }

    pub fn read_ops(&self) -> Result<Vec<OpEntry>, StoreError> {
        read_list(&self.minecraft(OPS_FILE))
    }

    pub fn write_ops(&self, entries: &[OpEntry]) -> Result<(), StoreError> {
        write_list(&self.minecraft(OPS_FILE), entries)
    }

    /// `Ok(true)` khi đã thêm. Trùng tên hoặc UUID thì không ghi bản thứ hai.
    pub fn add_op(&self, entry: OpEntry) -> Result<bool, StoreError> {
        add_row(&self.minecraft(OPS_FILE), entry)
    }

    /// `Ok(true)` khi đã gỡ ít nhất một dòng.
    pub fn remove_op(&self, player: PlayerRef<'_>) -> Result<bool, StoreError> {
        remove_row::<OpEntry>(&self.minecraft(OPS_FILE), player)
    }

    pub fn read_bans(&self) -> Result<Vec<BanEntry>, StoreError> {
        read_list(&self.minecraft(BANNED_FILE))
    }

    pub fn write_bans(&self, entries: &[BanEntry]) -> Result<(), StoreError> {
        write_list(&self.minecraft(BANNED_FILE), entries)
    }

    /// `Ok(true)` khi đã thêm. Trùng tên hoặc UUID thì không ghi bản thứ hai.
    pub fn add_ban(&self, entry: BanEntry) -> Result<bool, StoreError> {
        add_row(&self.minecraft(BANNED_FILE), entry)
    }

    /// `Ok(true)` khi đã gỡ ít nhất một dòng.
    pub fn remove_ban(&self, player: PlayerRef<'_>) -> Result<bool, StoreError> {
        remove_row::<BanEntry>(&self.minecraft(BANNED_FILE), player)
    }

    pub fn read_usercache(&self) -> Result<Vec<UserCacheEntry>, StoreError> {
        read_list(&self.minecraft(USERCACHE_FILE))
    }

    pub fn read_frozen(&self) -> Result<Vec<PlayerName>, StoreError> {
        read_list(&self.frozen_path)
    }

    pub fn write_frozen(&self, names: &[PlayerName]) -> Result<(), StoreError> {
        write_list(&self.frozen_path, names)
    }

    /// `Ok(true)` khi đã thêm. Tên trùng, kể cả khác hoa thường, thì giữ bản đã có.
    pub fn add_frozen(&self, name: &PlayerName) -> Result<bool, StoreError> {
        let mut names = self.read_frozen()?;
        if names
            .iter()
            .any(|have| have.as_str().eq_ignore_ascii_case(name.as_str()))
        {
            return Ok(false);
        }
        names.push(name.clone());
        self.write_frozen(&names)?;
        Ok(true)
    }

    /// `Ok(true)` khi đã gỡ. Không có ai thì không tạo file.
    pub fn remove_frozen(&self, name: &str) -> Result<bool, StoreError> {
        let mut names = self.read_frozen()?;
        let before = names.len();
        names.retain(|have| !have.as_str().eq_ignore_ascii_case(name));
        if names.len() == before {
            return Ok(false);
        }
        self.write_frozen(&names)?;
        Ok(true)
    }

    fn minecraft(&self, file_name: &str) -> PathBuf {
        self.server_dir.join(file_name)
    }
}

trait PlayerRow {
    fn player_name(&self) -> &PlayerName;
    fn player_uuid(&self) -> &str;
}

impl PlayerRow for WhitelistEntry {
    fn player_name(&self) -> &PlayerName {
        &self.name
    }
    fn player_uuid(&self) -> &str {
        &self.uuid
    }
}

impl PlayerRow for OpEntry {
    fn player_name(&self) -> &PlayerName {
        &self.name
    }
    fn player_uuid(&self) -> &str {
        &self.uuid
    }
}

impl PlayerRow for BanEntry {
    fn player_name(&self) -> &PlayerName {
        &self.name
    }
    fn player_uuid(&self) -> &str {
        &self.uuid
    }
}

fn add_row<T>(path: &Path, entry: T) -> Result<bool, StoreError>
where
    T: PlayerRow + Serialize + DeserializeOwned,
{
    let mut rows: Vec<T> = read_list(path)?;
    // Minecraft coi khác hoa thường là một người. Hai dòng thì lệnh gỡ chỉ trúng một.
    let duplicate = rows.iter().any(|row| {
        same_player(
            row.player_name(),
            row.player_uuid(),
            entry.player_name(),
            entry.player_uuid(),
        )
    });
    if duplicate {
        return Ok(false);
    }
    rows.push(entry);
    write_list(path, &rows)?;
    Ok(true)
}

fn remove_row<T>(path: &Path, player: PlayerRef<'_>) -> Result<bool, StoreError>
where
    T: PlayerRow + Serialize + DeserializeOwned,
{
    let mut rows = read_list::<T>(path)?;
    let before = rows.len();
    rows.retain(|row| !player.matches(row.player_name(), row.player_uuid()));
    if rows.len() == before {
        return Ok(false);
    }
    write_list(path, &rows)?;
    Ok(true)
}

fn same_player(name: &PlayerName, uuid: &str, other_name: &PlayerName, other_uuid: &str) -> bool {
    name.as_str().eq_ignore_ascii_case(other_name.as_str()) || uuid_eq(uuid, other_uuid)
}

fn uuid_eq(have: &str, want: &str) -> bool {
    !have.is_empty() && have.eq_ignore_ascii_case(want)
}

fn read_list<T: DeserializeOwned>(path: &Path) -> Result<Vec<T>, StoreError> {
    let Some(text) = read_text(path)? else {
        return Ok(Vec::new());
    };
    // File trống không phải JSON hỏng: editor có thể lưu nháp trước khi có người chơi.
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&text).map_err(|err| StoreError::invalid_players(path, err))
}

fn write_list<T: Serialize>(path: &Path, rows: &[T]) -> Result<(), StoreError> {
    let bytes = pretty_json(path, rows)?;
    mh_platform::fs::write_atomic(path, &bytes)?;
    Ok(())
}

fn pretty_json(path: &Path, value: &(impl Serialize + ?Sized)) -> Result<Vec<u8>, StoreError> {
    let mut bytes =
        serde_json::to_vec_pretty(value).map_err(|err| StoreError::invalid_players(path, err))?;
    // Gson của Minecraft kết thúc file bằng newline; thụt dòng là 2 dấu cách (`to_vec_pretty`).
    bytes.push(b'\n');
    Ok(bytes)
}

fn read_text(path: &Path) -> Result<Option<String>, StoreError> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(StoreError::io(path, err)),
    }
}
