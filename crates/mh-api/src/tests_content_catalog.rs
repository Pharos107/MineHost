//! Catalog mod/plugin giả và helper tạo file JAR cho test route mod/plugin.

use std::collections::HashMap;
use std::path::Path;

use mh_app::AppError;
use mh_app::content::{
    ContentCatalog, Dependency, DependencyType, FileHashes, ModrinthVersion, Project, ProjectLinks,
    SearchFilters, SearchHit, SearchResult, SideSupport, VersionFile, modrinth_loaders,
};
use mh_app::sources::{BoxFut, CancellationToken, ProgressFn};
use mh_core::{ErrorCode, LoaderKind, McVersion};

pub(super) struct FakeModCatalog {
    projects: Vec<Project>,
    versions: Vec<ModrinthVersion>,
    files: HashMap<String, Vec<u8>>,
}

impl FakeModCatalog {
    pub(super) fn sample() -> Self {
        let mut files = HashMap::new();
        let make_file =
            |files: &mut HashMap<String, Vec<u8>>, ver: &str, name: &str, bytes: Vec<u8>| {
                let url = format!("https://cdn.modrinth.com/{ver}/{name}");
                let vf = VersionFile {
                    url: url.clone(),
                    filename: name.to_owned(),
                    size: bytes.len() as u64,
                    hashes: FileHashes {
                        sha1: "0".repeat(40),
                        sha512: "0".repeat(128),
                    },
                    primary: true,
                };
                files.insert(url, bytes);
                vf
            };

        let dep_jar = store_zip(
            "fabric.mod.json",
            br#"{"schemaVersion":1,"id":"fabric-api","version":"0.110.0","name":"Fabric API","environment":"*"}"#,
        );
        let sodium_jar = store_zip(
            "fabric.mod.json",
            br#"{"schemaVersion":1,"id":"sodium","version":"0.6.1","name":"Sodium","environment":"*"}"#,
        );
        let iris_jar = store_zip(
            "fabric.mod.json",
            br#"{"schemaVersion":1,"id":"iris","version":"1.8.0","name":"Iris","environment":"client"}"#,
        );
        let fapi_file = make_file(&mut files, "fapi-1", "fabric-api-0.110.0.jar", dep_jar);
        let sodium_new = make_file(&mut files, "sod-2", "sodium-0.6.1.jar", sodium_jar.clone());
        let sodium_old = make_file(&mut files, "sod-1", "sodium-0.6.0.jar", sodium_jar);
        let iris_file = make_file(&mut files, "iris-1", "iris-1.8.0.jar", iris_jar);

        let projects = vec![
            project(
                "fabric-api",
                "mod",
                "Fabric API",
                &["fabric"],
                SideSupport::Required,
            ),
            project(
                "sodium",
                "mod",
                "Sodium",
                &["fabric"],
                SideSupport::Optional,
            ),
            project("iris", "mod", "Iris", &["fabric"], SideSupport::Unsupported),
            project(
                "luckperms",
                "plugin",
                "LuckPerms",
                &["paper"],
                SideSupport::Required,
            ),
        ];
        let req_fapi = vec![Dependency {
            project_id: Some("fabric-api".to_owned()),
            version_id: None,
            dependency_type: DependencyType::Required,
        }];
        let versions = vec![
            version(
                "fapi-1",
                "fabric-api",
                "0.110.0",
                &["fabric"],
                time::macros::datetime!(2024-12-01 0:00 UTC),
                fapi_file,
                Vec::new(),
            ),
            version(
                "sod-2",
                "sodium",
                "0.6.1",
                &["fabric"],
                time::macros::datetime!(2024-12-10 0:00 UTC),
                sodium_new,
                req_fapi.clone(),
            ),
            version(
                "sod-1",
                "sodium",
                "0.6.0",
                &["fabric"],
                time::macros::datetime!(2024-12-02 0:00 UTC),
                sodium_old,
                req_fapi,
            ),
            version(
                "iris-1",
                "iris",
                "1.8.0",
                &["fabric"],
                time::macros::datetime!(2024-12-05 0:00 UTC),
                iris_file,
                Vec::new(),
            ),
        ];
        Self {
            projects,
            versions,
            files,
        }
    }
}

impl ContentCatalog for FakeModCatalog {
    fn search<'a>(
        &'a self,
        query: &'a str,
        filters: &'a SearchFilters,
    ) -> BoxFut<'a, Result<SearchResult, AppError>> {
        Box::pin(async move {
            let expected = filters.loader.map(modrinth_loaders);
            let hits: Vec<SearchHit> = self
                .projects
                .iter()
                .filter(|p| {
                    filters
                        .project_type
                        .is_none_or(|k| p.project_type == k.as_str())
                        && expected
                            .is_none_or(|ls| p.loaders.iter().any(|l| ls.contains(&l.as_str())))
                        && filters
                            .minecraft
                            .as_ref()
                            .is_none_or(|mc| p.game_versions.iter().any(|g| g == mc.as_str()))
                        && (!filters.server_only || !p.server_side.is_unsupported())
                        && (query.is_empty()
                            || p.title.to_lowercase().contains(&query.to_lowercase()))
                })
                .map(|p| SearchHit {
                    project_id: p.id.clone(),
                    slug: p.slug.clone(),
                    project_type: p.project_type.clone(),
                    title: p.title.clone(),
                    description: p.description.clone(),
                    icon_url: p.icon_url.clone(),
                    downloads: p.downloads,
                    categories: p.loaders.clone(),
                    versions: p.game_versions.clone(),
                    client_side: p.client_side,
                    server_side: p.server_side,
                })
                .collect();
            let total_hits = hits.len() as u32;
            Ok(SearchResult {
                hits,
                offset: filters.offset,
                limit: filters.limit,
                total_hits,
            })
        })
    }

    fn project<'a>(&'a self, id_or_slug: &'a str) -> BoxFut<'a, Result<Project, AppError>> {
        Box::pin(async move {
            self.projects
                .iter()
                .find(|p| p.id == id_or_slug || p.slug == id_or_slug)
                .cloned()
                .ok_or_else(|| AppError::new(ErrorCode::ContentNotFound, id_or_slug))
        })
    }

    fn versions<'a>(
        &'a self,
        project: &'a str,
        loader: LoaderKind,
        mc: &'a McVersion,
    ) -> BoxFut<'a, Result<Vec<ModrinthVersion>, AppError>> {
        Box::pin(async move {
            let expected = modrinth_loaders(loader);
            let mut list: Vec<ModrinthVersion> = self
                .versions
                .iter()
                .filter(|v| {
                    v.project_id == project
                        && v.game_versions.iter().any(|g| g == mc.as_str())
                        && v.loaders.iter().any(|l| expected.contains(&l.as_str()))
                })
                .cloned()
                .collect();
            list.sort_by_key(|v| std::cmp::Reverse(v.date_published));
            Ok(list)
        })
    }

    fn version<'a>(&'a self, id: &'a str) -> BoxFut<'a, Result<ModrinthVersion, AppError>> {
        Box::pin(async move {
            self.versions
                .iter()
                .find(|v| v.id == id)
                .cloned()
                .ok_or_else(|| AppError::new(ErrorCode::ContentNotFound, id))
        })
    }

    fn download<'a>(
        &'a self,
        file: &'a VersionFile,
        dest: &'a Path,
        _: ProgressFn<'a>,
        _: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(async move {
            let bytes = self
                .files
                .get(&file.url)
                .ok_or_else(|| AppError::new(ErrorCode::ContentNotFound, &file.url))?;
            std::fs::write(dest, bytes).unwrap();
            Ok(())
        })
    }
}

fn project(
    id: &str,
    kind: &str,
    title: &str,
    loaders: &[&str],
    server_side: SideSupport,
) -> Project {
    Project {
        id: id.to_owned(),
        slug: id.to_owned(),
        project_type: kind.to_owned(),
        title: title.to_owned(),
        description: format!("{title} desc"),
        body: format!("# {title}"),
        icon_url: Some("https://cdn.modrinth.com/icon.png".to_owned()),
        downloads: 100,
        loaders: loaders.iter().map(|s| (*s).to_owned()).collect(),
        game_versions: vec!["1.21.4".to_owned()],
        client_side: SideSupport::Required,
        server_side,
        links: ProjectLinks {
            source_url: Some("https://example.invalid/src".to_owned()),
            issues_url: None,
            wiki_url: None,
            discord_url: None,
        },
    }
}

fn version(
    id: &str,
    pid: &str,
    num: &str,
    loaders: &[&str],
    date_published: time::OffsetDateTime,
    file: VersionFile,
    dependencies: Vec<Dependency>,
) -> ModrinthVersion {
    ModrinthVersion {
        id: id.to_owned(),
        project_id: pid.to_owned(),
        name: format!("v{num}"),
        version_number: num.to_owned(),
        version_type: "release".to_owned(),
        game_versions: vec!["1.21.4".to_owned()],
        loaders: loaders.iter().map(|s| (*s).to_owned()).collect(),
        date_published,
        files: vec![file],
        dependencies,
    }
}

/// Dựng file ZIP uncompressed (method 0) hợp lệ cho một entry, không cần thêm crate `zip` vào `mh-api`.
pub(super) fn store_zip(name: &str, data: &[u8]) -> Vec<u8> {
    let crc = crc32(data);
    let n = name.as_bytes();
    let (nl, dl) = (n.len() as u16, data.len() as u32);
    let mut out = Vec::new();
    out.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&dl.to_le_bytes());
    out.extend_from_slice(&dl.to_le_bytes());
    out.extend_from_slice(&nl.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(n);
    out.extend_from_slice(data);
    let cd_off = out.len() as u32;
    out.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02, 20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&dl.to_le_bytes());
    out.extend_from_slice(&dl.to_le_bytes());
    out.extend_from_slice(&nl.to_le_bytes());
    out.extend_from_slice(&[0; 12]);
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(n);
    let cd_size = out.len() as u32 - cd_off;
    out.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0, 1, 0, 1, 0]);
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_off.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}
