//! Dữ liệu modpack giả cho chế độ e2e (spec P3, mục 5.9).

use std::collections::HashMap;
use std::io::{Cursor, Write};

use mh_net::Checksum;
use serde_json::json;
use sha2::{Digest, Sha512};
use time::OffsetDateTime;
use time::macros::datetime;
use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

use super::content::GAME_VERSIONS;
use crate::content::{
    CatalogSource, ContentDependency, ContentFileInfo, ContentProject, ContentVersion,
    ProjectLinks, SideSupport,
};

pub(super) fn add_e2e_modpack(
    projects: &mut Vec<ContentProject>,
    versions: &mut Vec<ContentVersion>,
    files: &mut HashMap<String, Vec<u8>>,
    fabric_api: &ContentFileInfo,
    lithium: &ContentFileInfo,
    iris: &ContentFileInfo,
) {
    let entries = [
        ("e2e-fabric-api", "e2e-fabric-api-0.110.0", fabric_api, None),
        ("e2e-lithium", "e2e-lithium-0.14.2", lithium, None),
        ("e2e-iris", "e2e-iris-1.8.0", iris, Some("unsupported")),
    ];
    let mut index_files = Vec::new();
    for (pid, vid, vf, server_env) in entries {
        let bytes = vf
            .url
            .as_deref()
            .and_then(|u| files.get(u))
            .cloned()
            .unwrap_or_default();
        let url = format!(
            "https://cdn.modrinth.com/data/{pid}/versions/{vid}/{}",
            vf.filename
        );
        let mut obj = json!({
            "path": format!("mods/{}", vf.filename),
            "hashes": {
                "sha1": mh_store::sha1_hex(&bytes),
                "sha512": sha512_hex(&bytes),
            },
            "downloads": [url],
            "fileSize": bytes.len() as u64,
        });
        if let Some(env) = server_env {
            obj["env"] = json!({ "client": "required", "server": env });
        } else {
            files.insert(url, bytes);
        }
        index_files.push(obj);
    }

    let mrpack = build_mrpack_bytes(&index_files);
    let pack_url = "e2e://modrinth/e2e-fabric-pack-1.0.0/e2e-pack-1.0.0.mrpack".to_owned();
    let pack_file = ContentFileInfo {
        url: Some(pack_url.clone()),
        filename: "e2e-pack-1.0.0.mrpack".to_owned(),
        size: mrpack.len() as u64,
        sha1: Some(mh_store::sha1_hex(&mrpack)),
        sha512: Some(sha512_hex(&mrpack)),
        manual_download: false,
        download_page_url: None,
    };
    files.insert(pack_url, mrpack);

    projects.push(make_project(
        "e2e-fabric-pack",
        "fabric-pack",
        "modpack",
        "E2E Fabric Pack",
        "Modpack Fabric mẫu cho kiểm thử tự động.",
        250_000,
        &["fabric"],
        SideSupport::Required,
        SideSupport::Required,
    ));
    versions.push(make_version(
        "e2e-fabric-pack-1.0.0",
        "e2e-fabric-pack",
        "1.0.0",
        &["fabric"],
        datetime!(2024-12-15 0:00 UTC),
        pack_file,
        Vec::new(),
    ));
}

fn build_mrpack_bytes(index_files: &[serde_json::Value]) -> Vec<u8> {
    let index = json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "1.0.0",
        "name": "E2E Fabric Pack",
        "files": index_files,
        "dependencies": {
            "minecraft": "1.21.4",
            "fabric-loader": "0.16.9",
        },
    })
    .to_string();
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    if zip.start_file("modrinth.index.json", opts).is_ok()
        && zip.write_all(index.as_bytes()).is_ok()
        && zip
            .start_file("overrides/config/e2e-pack.txt", opts)
            .is_ok()
        && zip.write_all(b"e2e-override=true\n").is_ok()
        && let Ok(cur) = zip.finish()
    {
        return cur.into_inner();
    }
    Vec::new()
}

pub(super) fn verify_bytes_checksum(bytes: &[u8], checksum: &Checksum) -> bool {
    match checksum {
        Checksum::Sha512(expected) => sha512_hex(bytes) == *expected,
        Checksum::Sha1(expected) => mh_store::sha1_hex(bytes) == *expected,
        Checksum::Sha256(expected) => {
            let actual: String = sha2::Sha256::digest(bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            actual == *expected
        }
        Checksum::Md5(_) => false,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn make_project(
    id: &str,
    slug: &str,
    project_type: &str,
    title: &str,
    description: &str,
    downloads: u64,
    loaders: &[&str],
    client_side: SideSupport,
    server_side: SideSupport,
) -> ContentProject {
    ContentProject {
        source: CatalogSource::Modrinth,
        id: id.to_owned(),
        slug: slug.to_owned(),
        project_type: project_type.to_owned(),
        title: title.to_owned(),
        description: description.to_owned(),
        body: Some(format!("# {title}\n\n{description}")),
        icon_url: None,
        downloads,
        loaders: loaders.iter().map(|s| (*s).to_owned()).collect(),
        game_versions: GAME_VERSIONS.iter().map(|s| (*s).to_owned()).collect(),
        client_side: Some(client_side),
        server_side: Some(server_side),
        page_url: format!("https://modrinth.com/{project_type}/{slug}"),
        links: ProjectLinks {
            source_url: None,
            issues_url: None,
            wiki_url: None,
            discord_url: None,
        },
    }
}

pub(super) fn make_version(
    id: &str,
    project_id: &str,
    version_number: &str,
    loaders: &[&str],
    date_published: OffsetDateTime,
    file: ContentFileInfo,
    dependencies: Vec<ContentDependency>,
) -> ContentVersion {
    ContentVersion {
        id: id.to_owned(),
        project_id: project_id.to_owned(),
        name: format!("v{version_number}"),
        version_number: Some(version_number.to_owned()),
        version_type: "release".to_owned(),
        game_versions: GAME_VERSIONS.iter().map(|s| (*s).to_owned()).collect(),
        loaders: loaders.iter().map(|s| (*s).to_owned()).collect(),
        date_published,
        files: vec![file],
        dependencies,
    }
}

/// Jar giả có bytes cố định trên mọi máy: test E2E ghi sẵn hash của nó trong `.mrpack` dựng tay
/// (`ui/e2e/modpack.spec.ts`). Phải cố định cả cách nén, mốc thời gian lẫn hệ điều hành ghi trong
/// zip (crate `zip` mặc định ghi DOS trên Windows, Unix trên Linux, làm bytes khác nhau).
pub(super) fn build_jar(entry_name: &str, content: &str) -> Vec<u8> {
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(zip::DateTime::default())
        .system(zip::System::Unix)
        .unix_permissions(0o644);
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    if zip.start_file(entry_name, options).is_ok()
        && zip.write_all(content.as_bytes()).is_ok()
        && let Ok(cur) = zip.finish()
    {
        return cur.into_inner();
    }
    Vec::new()
}

pub(super) fn sha512_hex(bytes: &[u8]) -> String {
    Sha512::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::content::ContentCatalog;
    use crate::e2e::content::E2eContent;

    /// Hash của jar `fabric-api` giả được ghi cứng trong `ui/e2e/modpack.spec.ts` (test kéo thả
    /// `.mrpack`). Đổi nội dung jar giả thì phải sửa cả hai chỗ.
    #[tokio::test]
    async fn fake_fabric_api_jar_bytes_are_stable() {
        let cat = E2eContent::new();
        let vers = cat.project_versions("e2e-fabric-api").await.unwrap();
        let file = &vers[0].files[0];
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fabric-api.jar");
        let cancel = crate::sources::CancellationToken::new();
        cat.download(file, &path, &mut |_, _| {}, &cancel)
            .await
            .unwrap();
        let bytes = std::fs::read(&path).unwrap();
        use sha1::Digest as _;
        let sha1: String = sha1::Sha1::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(bytes.len(), 223);
        assert_eq!(sha1, "a4128590aebddb6984d71ac0d89e56c4df143f0f");
        assert_eq!(
            super::sha512_hex(&bytes),
            "6305d5c3619594eec9264dbcc5987847402722871548ed6af51ca84b750daa7b405818b21c3dadcdb5dcbedafde01dc1ce6b402fed75392809a6687ad45fa4ba"
        );
    }

    #[tokio::test]
    async fn e2e_content_provides_valid_mrpack_and_cdn_files() {
        let cat = E2eContent::new();
        let vers = cat.project_versions("e2e-fabric-pack").await.unwrap();
        assert_eq!(vers.len(), 1);
        let dir = tempfile::tempdir().unwrap();
        let pack_path = dir.path().join("pack.mrpack");
        let cancel = crate::sources::CancellationToken::new();
        cat.download(&vers[0].files[0], &pack_path, &mut |_, _| {}, &cancel)
            .await
            .unwrap();
        let plan = mh_providers::plan_mrpack(&pack_path).unwrap();
        assert_eq!(plan.files.len(), 2);
        assert_eq!(plan.skipped_client, ["mods/iris-1.8.0.jar"]);
    }
}
