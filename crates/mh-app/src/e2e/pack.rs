//! Dữ liệu modpack giả cho chế độ e2e (spec P3, mục 5.9).

use std::collections::HashMap;
use std::io::{Cursor, Write};
use std::path::Path;

use mh_core::ErrorCode;
use mh_net::Checksum;
use mh_providers::modpack::PackFile;
use serde_json::json;
use sha2::{Digest, Sha512};
use time::OffsetDateTime;
use time::macros::datetime;
use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

use super::content::GAME_VERSIONS;
use crate::AppError;
use crate::content::{
    CatalogSource, ContentDependency, ContentFileInfo, ContentProject, ContentVersion,
    ProjectLinks, SideSupport,
};
use crate::sources::{CancellationToken, ProgressFn};

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
    build_zip_entries(&[
        ("modrinth.index.json", index.as_bytes()),
        ("overrides/config/e2e-pack.txt", b"e2e-override=true\n"),
    ])
}

pub(super) fn build_zip_entries(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, data) in entries {
        if zip.start_file(*name, opts).is_err() || zip.write_all(data).is_err() {
            return Vec::new();
        }
    }
    zip.finish().map(|cur| cur.into_inner()).unwrap_or_default()
}

pub(super) fn download_e2e_pack_file(
    files: &HashMap<String, Vec<u8>>,
    file: &PackFile,
    dest: &Path,
    progress: ProgressFn<'_>,
    cancel: &CancellationToken,
) -> Result<(), AppError> {
    if cancel.is_cancelled() {
        return Err(AppError::cancelled());
    }
    let bytes = file
        .urls
        .iter()
        .find_map(|url| files.get(url))
        .ok_or_else(|| AppError::new(ErrorCode::ContentNotFound, &file.path))?;
    if !verify_bytes_checksum(bytes, &file.checksum) {
        return Err(AppError::new(
            ErrorCode::ChecksumMismatch,
            format!("sai checksum file `{}`", file.path),
        ));
    }
    let size = bytes.len() as u64;
    progress(size, Some(size));
    mh_platform::fs::write_atomic(dest, bytes)?;
    Ok(())
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

pub(super) fn add_e2e_cf_modpacks(
    projects: &mut Vec<ContentProject>,
    versions: &mut Vec<ContentVersion>,
    files: &mut HashMap<String, Vec<u8>>,
    server_packs: &mut HashMap<String, ContentFileInfo>,
    cf_fapi_bytes: &[u8],
) {
    let manifest_restricted = json!({
        "minecraft": { "version": "1.21.4", "modLoaders": [{ "id": "fabric-0.16.9", "primary": true }] },
        "manifestType": "minecraftModpack",
        "manifestVersion": 1,
        "name": "E2E CF Restricted Pack",
        "version": "1.0.0",
        "files": [
            { "projectID": 306612, "fileID": 5800000, "required": true },
            { "projectID": 999001, "fileID": 5800999, "required": true }
        ],
        "overrides": "overrides"
    })
    .to_string();
    let zip_restricted = build_zip_entries(&[
        ("manifest.json", manifest_restricted.as_bytes()),
        ("overrides/config/cf-restricted.cfg", b"restricted=true\n"),
    ]);
    let restricted_pack_file = register_cf_zip(
        files,
        "5900001",
        "cf-restricted-pack-1.0.0.zip",
        zip_restricted,
    );
    projects.push(cf_project(
        "910001",
        "cf-restricted-pack",
        "modpack",
        "E2E CF Restricted Pack",
        "Modpack CurseForge có một mod cấm phân phối.",
        180_000,
    ));
    versions.push(cf_version(
        "5900001",
        "910001",
        "cf-restricted-pack-1.0.0",
        datetime!(2024-12-16 0:00 UTC),
        restricted_pack_file,
        Vec::new(),
    ));

    let manifest_sp = json!({
        "minecraft": { "version": "1.21.4", "modLoaders": [{ "id": "fabric-0.16.9", "primary": true }] },
        "manifestType": "minecraftModpack",
        "manifestVersion": 1,
        "name": "E2E CF Server Pack",
        "version": "1.0.0",
        "files": [],
        "overrides": "overrides"
    })
    .to_string();
    let client_zip = build_zip_entries(&[("manifest.json", manifest_sp.as_bytes())]);
    let client_pack_file =
        register_cf_zip(files, "5900002", "cf-server-pack-1.0.0.zip", client_zip);
    let sp_zip = build_zip_entries(&[
        (
            "CFServerPack-1.0/mods/cf-fabric-api-0.110.0.jar",
            cf_fapi_bytes,
        ),
        (
            "CFServerPack-1.0/config/server-pack.toml",
            b"mode = \"e2e\"\n",
        ),
        ("CFServerPack-1.0/start.sh", b"#!/bin/sh\njava"),
        ("CFServerPack-1.0/run.bat", b"@echo off\njava"),
        ("CFServerPack-1.0/libraries/net/fabric/dummy.jar", b"lib"),
        ("CFServerPack-1.0/fabric-server-launch.jar", b"launch"),
    ]);
    let sp_file = register_cf_zip(files, "5900003", "cf-server-pack-1.0.0-server.zip", sp_zip);
    server_packs.insert("5900002".to_owned(), sp_file);

    projects.push(cf_project(
        "910002",
        "cf-server-pack",
        "modpack",
        "E2E CF Server Pack",
        "Modpack CurseForge có server pack lồng một tầng.",
        220_000,
    ));
    versions.push(cf_version(
        "5900002",
        "910002",
        "cf-server-pack-1.0.0",
        datetime!(2024-12-17 0:00 UTC),
        client_pack_file,
        Vec::new(),
    ));
}

fn register_cf_zip(
    files: &mut HashMap<String, Vec<u8>>,
    ver_id: &str,
    filename: &str,
    bytes: Vec<u8>,
) -> ContentFileInfo {
    let url = format!("e2e://curseforge/{ver_id}/{filename}");
    let info = ContentFileInfo {
        url: Some(url.clone()),
        filename: filename.to_owned(),
        size: bytes.len() as u64,
        sha1: Some(mh_store::sha1_hex(&bytes)),
        sha512: None,
        manual_download: false,
        download_page_url: Some(format!(
            "https://www.curseforge.com/minecraft/modpacks/e2e/download/{ver_id}"
        )),
    };
    files.insert(url, bytes);
    info
}

pub(super) fn cf_project(
    id: &str,
    slug: &str,
    project_type: &str,
    title: &str,
    description: &str,
    downloads: u64,
) -> ContentProject {
    let segment = if project_type == "modpack" {
        "modpacks"
    } else {
        "mc-mods"
    };
    ContentProject {
        source: CatalogSource::Curseforge,
        id: id.to_owned(),
        slug: slug.to_owned(),
        project_type: project_type.to_owned(),
        title: title.to_owned(),
        description: description.to_owned(),
        body: None,
        icon_url: None,
        downloads,
        loaders: Vec::new(),
        game_versions: Vec::new(),
        client_side: None,
        server_side: None,
        page_url: format!("https://www.curseforge.com/minecraft/{segment}/{slug}"),
        links: ProjectLinks {
            source_url: None,
            issues_url: None,
            wiki_url: None,
            discord_url: None,
        },
    }
}

pub(super) fn cf_version(
    id: &str,
    project_id: &str,
    name: &str,
    date_published: OffsetDateTime,
    file: ContentFileInfo,
    dependencies: Vec<ContentDependency>,
) -> ContentVersion {
    ContentVersion {
        id: id.to_owned(),
        project_id: project_id.to_owned(),
        name: name.to_owned(),
        version_number: None,
        version_type: "release".to_owned(),
        game_versions: GAME_VERSIONS.iter().map(|s| (*s).to_owned()).collect(),
        loaders: vec![
            "fabric".to_owned(),
            "neoforge".to_owned(),
            "forge".to_owned(),
            "quilt".to_owned(),
        ],
        date_published,
        dependencies,
        files: vec![file],
    }
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
