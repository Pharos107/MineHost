//! Hồ sơ Mojang và ảnh skin (spec P2, mục 3.8, 5.4, 8).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use mh_core::PlayerName;
use mh_net::NetError;
use serde::Deserialize;
use url::Url;

use super::{Mojang, SKIN_ORIGIN, SOURCE};
use crate::ProviderError;

/// Hồ sơ tài khoản. `uuid` luôn có gạch ngang, chữ thường.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub uuid: String,
    pub name: String,
}

const SKIN_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const PROFILE_MAX: usize = 8 * 1024;
const SESSION_MAX: usize = 32 * 1024;
const SKIN_MAX: usize = 64 * 1024;
const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

#[derive(Deserialize)]
struct RawProfile {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct SessionProfile {
    #[serde(default)]
    properties: Vec<SessionProperty>,
}

#[derive(Deserialize)]
struct SessionProperty {
    name: String,
    value: String,
}

#[derive(Deserialize)]
struct TexturesPayload {
    #[serde(default)]
    textures: TextureMap,
}

#[derive(Default, Deserialize)]
struct TextureMap {
    #[serde(rename = "SKIN")]
    skin: Option<SkinTexture>,
}

#[derive(Deserialize)]
struct SkinTexture {
    url: Option<String>,
}

impl Mojang {
    /// `id` không gạch của Mojang được đổi sang UUID có gạch.
    pub async fn profile_by_name(&self, name: &PlayerName) -> Result<Profile, ProviderError> {
        let url = format!(
            "{}/users/profiles/minecraft/{}",
            trim_slash(&self.profile_base),
            name.as_str()
        );
        let body = match self.http.get_bytes(&url, PROFILE_MAX).await {
            Ok(body) => body,
            Err(NetError::Status { status: 404, .. }) => {
                return Err(ProviderError::PlayerNotFound(name.to_string()));
            }
            Err(err) => return Err(err.into()),
        };
        // 204 No Content cũng là "không có tài khoản". Thân rỗng không phải JSON hỏng.
        if body.iter().all(u8::is_ascii_whitespace) {
            return Err(ProviderError::PlayerNotFound(name.to_string()));
        }
        parse_profile(&body)
    }

    /// PNG của skin. Cache `<cache>/skins/<uuid>.png` sống 24 giờ theo thời điểm sửa file.
    ///
    /// Hết hạn mà mạng lỗi thì trả bản cũ. Không có skin thì [`ProviderError::NoSkin`].
    pub async fn skin(&self, uuid: &str) -> Result<Vec<u8>, ProviderError> {
        let dashed = dashed_uuid(uuid)?;
        let path = self.skin_path(&dashed);
        if let Some(png) = read_fresh(&path)? {
            return Ok(png);
        }
        match self.download_skin(&dashed).await {
            Ok(png) => {
                if let Err(err) = store_skin(&path, &png) {
                    tracing::warn!(error = %err, "không ghi được cache skin");
                }
                Ok(png)
            }
            Err(ProviderError::Net(err)) => stale_or(&path, err),
            Err(err) => Err(err),
        }
    }

    fn skin_path(&self, dashed: &str) -> PathBuf {
        self.http
            .cache_dir()
            .join("skins")
            .join(format!("{dashed}.png"))
    }

    async fn download_skin(&self, dashed: &str) -> Result<Vec<u8>, ProviderError> {
        let url = format!(
            "{}/session/minecraft/profile/{}",
            trim_slash(&self.session_base),
            dashed.replace('-', "")
        );
        let body = match self.http.get_bytes(&url, SESSION_MAX).await {
            Ok(body) => body,
            Err(NetError::Status { status: 404, .. }) => return Err(ProviderError::NoSkin),
            Err(err) => return Err(err.into()),
        };
        let skin_url = skin_url_from_profile(&body)?;
        ensure_skin_origin(&skin_url, &self.skin_origin)?;
        let png = match self.http.get_bytes(&skin_url, SKIN_MAX).await {
            Ok(png) => png,
            Err(NetError::Status { status: 404, .. }) => return Err(ProviderError::NoSkin),
            Err(err) => return Err(err.into()),
        };
        if png.starts_with(PNG_MAGIC) {
            Ok(png)
        } else {
            Err(NetError::Download {
                url: skin_url,
                reason: "không phải PNG".to_owned(),
            }
            .into())
        }
    }
}

fn parse_profile(body: &[u8]) -> Result<Profile, ProviderError> {
    let raw: RawProfile = serde_json::from_slice(body)
        .map_err(|_| ProviderError::invalid(SOURCE, "hồ sơ Mojang không đúng JSON"))?;
    Ok(Profile {
        uuid: dashed_uuid(&raw.id)?,
        name: raw.name,
    })
}

fn skin_url_from_profile(body: &[u8]) -> Result<String, ProviderError> {
    let profile: SessionProfile = serde_json::from_slice(body)
        .map_err(|_| ProviderError::invalid(SOURCE, "phiên Mojang không đúng JSON"))?;
    let encoded = profile
        .properties
        .iter()
        .find(|property| property.name == "textures")
        .ok_or(ProviderError::NoSkin)?;
    let compact: String = encoded
        .value
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    let bytes = STANDARD
        .decode(compact)
        .map_err(|_| ProviderError::invalid(SOURCE, "textures không phải base64"))?;
    let payload: TexturesPayload = serde_json::from_slice(&bytes)
        .map_err(|_| ProviderError::invalid(SOURCE, "textures giải mã không phải JSON"))?;
    payload
        .textures
        .skin
        .and_then(|skin| skin.url)
        .filter(|url| !url.is_empty())
        .ok_or(ProviderError::NoSkin)
}

/// So scheme, host và cổng. Không dùng `contains`: `textures.minecraft.net.evil.com` phải bị từ chối.
fn ensure_skin_origin(skin_url: &str, allowed: &str) -> Result<(), ProviderError> {
    if same_origin(skin_url, allowed) {
        Ok(())
    } else {
        Err(ProviderError::invalid(
            SOURCE,
            format!("url skin không thuộc {SKIN_ORIGIN}"),
        ))
    }
}

fn same_origin(candidate: &str, allowed: &str) -> bool {
    let Ok(url) = Url::parse(candidate) else {
        return false;
    };
    let Ok(allowed) = Url::parse(allowed) else {
        return false;
    };
    url.username().is_empty()
        && url.password().is_none()
        && url.scheme() == allowed.scheme()
        && url.host_str() == allowed.host_str()
        && url.port_or_known_default() == allowed.port_or_known_default()
}

fn stale_or(path: &Path, err: NetError) -> Result<Vec<u8>, ProviderError> {
    match read_any(path)? {
        Some(png) => {
            tracing::warn!(error = %err, "tải skin lỗi, dùng bản cache hết hạn");
            Ok(png)
        }
        None => Err(err.into()),
    }
}

fn read_fresh(path: &Path) -> Result<Option<Vec<u8>>, ProviderError> {
    let meta = match fs::metadata(path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(io_err(path, err)),
    };
    if !is_fresh(&meta) {
        return Ok(None);
    }
    match fs::read(path) {
        Ok(png) if png.starts_with(PNG_MAGIC) => Ok(Some(png)),
        Ok(_) => Ok(None),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(io_err(path, err)),
    }
}

fn is_fresh(meta: &fs::Metadata) -> bool {
    // Không đọc được mtime thì coi là hết hạn để tải lại. Lỗi mạng vẫn còn bản cũ.
    let Ok(modified) = meta.modified() else {
        return false;
    };
    // Thời điểm sửa ở tương lai (đồng hồ lệch): coi là còn mới, không tải lại.
    match modified.elapsed() {
        Ok(age) => age < SKIN_TTL,
        Err(_) => true,
    }
}

fn read_any(path: &Path) -> Result<Option<Vec<u8>>, ProviderError> {
    match fs::read(path) {
        Ok(png) if png.starts_with(PNG_MAGIC) => Ok(Some(png)),
        Ok(_) => Ok(None),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(io_err(path, err)),
    }
}

fn store_skin(path: &Path, png: &[u8]) -> Result<(), ProviderError> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|err| io_err(dir, err))?;
    }
    mh_platform::fs::write_atomic(path, png)?;
    Ok(())
}

fn io_err(path: &Path, source: io::Error) -> ProviderError {
    ProviderError::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn trim_slash(base: &str) -> &str {
    base.trim_end_matches('/')
}

/// 32 ký tự hex, có hoặc không có gạch, thành `8-4-4-4-12` chữ thường.
fn dashed_uuid(raw: &str) -> Result<String, ProviderError> {
    let mut hex = String::with_capacity(32);
    for c in raw.chars() {
        if c == '-' {
            continue;
        }
        if !c.is_ascii_hexdigit() {
            return Err(ProviderError::invalid(SOURCE, "uuid không phải hex"));
        }
        hex.push(c.to_ascii_lowercase());
    }
    if hex.len() != 32 {
        return Err(ProviderError::invalid(SOURCE, "uuid không đủ 32 ký tự hex"));
    }
    // Toàn bộ là hex ASCII nên cắt theo byte vẫn đúng biên ký tự.
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

#[cfg(test)]
mod tests {
    use super::same_origin;
    use crate::mojang::SKIN_ORIGIN;

    #[test]
    fn skin_url_matches_scheme_and_host_exactly() {
        let allowed = SKIN_ORIGIN;
        assert!(same_origin(
            "https://textures.minecraft.net/texture/abc",
            allowed
        ));
        assert!(same_origin(
            "https://textures.minecraft.net:443/texture/abc",
            allowed
        ));
        assert!(same_origin(
            "https://TEXTURES.minecraft.net/texture/abc",
            allowed
        ));
        assert!(!same_origin(
            "http://textures.minecraft.net/texture/abc",
            allowed
        ));
        assert!(!same_origin(
            "https://textures.minecraft.net:444/texture/abc",
            allowed
        ));
        assert!(!same_origin(
            "https://textures.minecraft.net.evil.com/texture/abc",
            allowed
        ));
        assert!(!same_origin(
            "https://evil.com/textures.minecraft.net",
            allowed
        ));
        assert!(!same_origin(
            "https://user@textures.minecraft.net/a",
            allowed
        ));
        assert!(!same_origin("not a url", allowed));
    }
}
