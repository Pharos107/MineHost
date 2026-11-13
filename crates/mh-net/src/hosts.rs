//! Danh sách host được phép tải file (spec P3, mục 3.7).
//!
//! URL của mod và modpack đến từ dữ liệu bên ngoài, kể cả file `.mrpack` người dùng tải lên. Tải
//! có danh sách thì từng bước chuyển hướng cũng phải nằm trong danh sách.

use reqwest::Url;

use crate::NetError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostAllowlist(&'static [&'static str]);

/// Danh sách của Modrinth cho `.mrpack`, thêm host mà `github.com` chuyển hướng file release tới.
pub const MODRINTH: HostAllowlist = HostAllowlist::new(&[
    "cdn.modrinth.com",
    "github.com",
    "raw.githubusercontent.com",
    "gitlab.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
]);

pub const CURSEFORGE: HostAllowlist =
    HostAllowlist::new(&["edge.forgecdn.net", "mediafilez.forgecdn.net"]);

/// Meta và maven chính thức của bốn mod loader.
pub const LOADERS: HostAllowlist = HostAllowlist::new(&[
    "meta.fabricmc.net",
    "maven.fabricmc.net",
    "meta.quiltmc.org",
    "maven.quiltmc.org",
    "maven.minecraftforge.net",
    "files.minecraftforge.net",
    "maven.neoforged.net",
]);

impl HostAllowlist {
    /// Host so khớp nguyên văn, không tính tên miền con.
    pub const fn new(hosts: &'static [&'static str]) -> Self {
        Self(hosts)
    }

    pub fn hosts(&self) -> &'static [&'static str] {
        self.0
    }

    /// Chỉ nhận `https`. `http` chỉ được với địa chỉ loopback có trong danh sách, cho server giả
    /// trong test và chế độ e2e.
    pub fn check(&self, url: &Url) -> Result<(), NetError> {
        let host = url.host_str().unwrap_or_default();
        let listed = self.0.iter().any(|h| h.eq_ignore_ascii_case(host));
        let scheme_ok = match url.scheme() {
            "https" => true,
            "http" => is_loopback(host),
            _ => false,
        };
        if listed && scheme_ok {
            return Ok(());
        }
        Err(NetError::HostNotAllowed {
            url: url.to_string(),
            host: host.to_owned(),
        })
    }
}

impl HostAllowlist {
    /// Như [`HostAllowlist::check`] với URL dạng chuỗi; chuỗi không phải URL cũng bị từ chối.
    pub fn check_str(&self, url: &str) -> Result<(), NetError> {
        let parsed = Url::parse(url).map_err(|_| NetError::HostNotAllowed {
            url: url.to_owned(),
            host: String::new(),
        })?;
        self.check(&parsed)
    }
}

fn is_loopback(host: &str) -> bool {
    host == "localhost"
        || host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn exact_host_over_https_only() {
        assert!(
            MODRINTH
                .check(&url("https://cdn.modrinth.com/data/x.jar"))
                .is_ok()
        );
        assert!(
            MODRINTH
                .check(&url("https://CDN.modrinth.com/data/x.jar"))
                .is_ok()
        );
        for bad in [
            "http://cdn.modrinth.com/data/x.jar",
            "https://evil.cdn.modrinth.com/x.jar",
            "https://cdn.modrinth.com.evil.net/x.jar",
            "https://example.com/x.jar",
            "ftp://cdn.modrinth.com/x.jar",
            "file:///etc/passwd",
        ] {
            assert!(MODRINTH.check(&url(bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn plain_http_only_for_listed_loopback() {
        let local = HostAllowlist::new(&["127.0.0.1", "localhost"]);
        assert!(local.check(&url("http://127.0.0.1:8080/x")).is_ok());
        assert!(local.check(&url("http://localhost:8080/x")).is_ok());
        assert!(local.check(&url("http://[::1]:8080/x")).is_err());
        assert!(CURSEFORGE.check(&url("http://127.0.0.1/x")).is_err());
    }
}
