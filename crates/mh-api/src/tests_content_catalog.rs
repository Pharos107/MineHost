//! Catalog mod/plugin giả và helper tạo file JAR cho test route mod/plugin.

use std::collections::HashMap;
use std::path::Path;

use mh_app::AppError;
use mh_app::content::{
    CatalogSource, ContentCatalog, ContentDependency, ContentFileInfo, ContentProject,
    ContentSearchHit, ContentSearchResult, ContentVersion, DependencyType, ProjectLinks,
    SearchFilters, SideSupport, modrinth_loaders,
};
use mh_app::sources::{BoxFut, CancellationToken, ProgressFn};
use mh_core::{ErrorCode, LoaderKind, McVersion};

pub(super) struct FakeModCatalog {
    source: CatalogSource,
    projects: Vec<ContentProject>,
    versions: Vec<ContentVersion>,
    files: HashMap<String, Vec<u8>>,
}

impl FakeModCatalog {
    pub(super) fn sample() -> Self {
        let mut files = HashMap::new();
        let mut make_file = |ver: &str, name: &str, bytes: Vec<u8>| {
            let url = format!("https://cdn.modrinth.com/{ver}/{name}");
            let vf = file_info(Some(url.clone()), name, bytes.len() as u64, false, None);
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
        let fapi_file = make_file("fapi-1", "fabric-api-0.110.0.jar", dep_jar);
        let sodium_new = make_file("sod-2", "sodium-0.6.1.jar", sodium_jar.clone());
        let sodium_old = make_file("sod-1", "sodium-0.6.0.jar", sodium_jar);
        let iris_file = make_file("iris-1", "iris-1.8.0.jar", iris_jar);

        let projects = vec![
            project(
                CatalogSource::Modrinth,
                "fabric-api",
                "mod",
                "Fabric API",
                &["fabric"],
                Some(SideSupport::Required),
            ),
            project(
                CatalogSource::Modrinth,
                "sodium",
                "mod",
                "Sodium",
                &["fabric"],
                Some(SideSupport::Optional),
            ),
            project(
                CatalogSource::Modrinth,
                "iris",
                "mod",
                "Iris",
                &["fabric"],
                Some(SideSupport::Unsupported),
            ),
            project(
                CatalogSource::Modrinth,
                "luckperms",
                "plugin",
                "LuckPerms",
                &["paper"],
                Some(SideSupport::Required),
            ),
        ];
        let req_fapi = vec![req_dep("fabric-api")];
        let versions = vec![
            version(
                "fapi-1",
                "fabric-api",
                Some("0.110.0"),
                &["fabric"],
                time::macros::datetime!(2024-12-01 0:00 UTC),
                fapi_file,
                Vec::new(),
            ),
            version(
                "sod-2",
                "sodium",
                Some("0.6.1"),
                &["fabric"],
                time::macros::datetime!(2024-12-10 0:00 UTC),
                sodium_new,
                req_fapi.clone(),
            ),
            version(
                "sod-1",
                "sodium",
                Some("0.6.0"),
                &["fabric"],
                time::macros::datetime!(2024-12-02 0:00 UTC),
                sodium_old,
                req_fapi,
            ),
            version(
                "iris-1",
                "iris",
                Some("1.8.0"),
                &["fabric"],
                time::macros::datetime!(2024-12-05 0:00 UTC),
                iris_file,
                Vec::new(),
            ),
        ];
        Self {
            source: CatalogSource::Modrinth,
            projects,
            versions,
            files,
        }
    }

    pub(super) fn curseforge() -> Self {
        let mut files = HashMap::new();
        let dep_jar = store_zip(
            "fabric.mod.json",
            br#"{"schemaVersion":1,"id":"cf-fabric-api","version":"0.110.0","name":"Fabric API CF","environment":"*"}"#,
        );
        let jei_jar = store_zip(
            "fabric.mod.json",
            br#"{"schemaVersion":1,"id":"jei","version":"19.21.0","name":"JEI","environment":"*"}"#,
        );
        let fapi_url = "https://edge.forgecdn.net/files/5800/0/cf-fapi.jar".to_owned();
        let jei_url = "https://edge.forgecdn.net/files/5800/1/cf-jei.jar".to_owned();
        let fapi_file = file_info(
            Some(fapi_url.clone()),
            "cf-fapi.jar",
            dep_jar.len() as u64,
            false,
            None,
        );
        let jei_file = file_info(
            Some(jei_url.clone()),
            "cf-jei.jar",
            jei_jar.len() as u64,
            false,
            None,
        );
        files.insert(fapi_url, dep_jar);
        files.insert(jei_url, jei_jar);
        let restricted_file = file_info(
            None,
            "restricted.jar",
            2048,
            true,
            Some(
                "https://www.curseforge.com/minecraft/mc-mods/restricted/download/7770001"
                    .to_owned(),
            ),
        );

        let projects = vec![
            project(
                CatalogSource::Curseforge,
                "306612",
                "mod",
                "Fabric API CF",
                &["fabric"],
                None,
            ),
            project(
                CatalogSource::Curseforge,
                "238222",
                "mod",
                "JEI",
                &["fabric"],
                None,
            ),
            project(
                CatalogSource::Curseforge,
                "777001",
                "mod",
                "Restricted Mod",
                &["fabric"],
                None,
            ),
        ];
        let versions = vec![
            version(
                "5800000",
                "306612",
                None,
                &["fabric"],
                time::macros::datetime!(2024-12-01 0:00 UTC),
                fapi_file,
                Vec::new(),
            ),
            version(
                "5800001",
                "238222",
                None,
                &["fabric"],
                time::macros::datetime!(2024-12-10 0:00 UTC),
                jei_file,
                vec![req_dep("306612")],
            ),
            version(
                "7770001",
                "777001",
                None,
                &["fabric"],
                time::macros::datetime!(2024-12-11 0:00 UTC),
                restricted_file,
                Vec::new(),
            ),
        ];
        Self {
            source: CatalogSource::Curseforge,
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
    ) -> BoxFut<'a, Result<ContentSearchResult, AppError>> {
        Box::pin(async move {
            let expected = filters.loader.map(modrinth_loaders);
            let hits: Vec<ContentSearchHit> = self
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
                        && (!filters.server_only
                            || !p.server_side.is_some_and(|s| s.is_unsupported()))
                        && (query.is_empty()
                            || p.title.to_lowercase().contains(&query.to_lowercase()))
                })
                .map(|p| ContentSearchHit {
                    source: self.source,
                    project_id: p.id.clone(),
                    slug: p.slug.clone(),
                    project_type: p.project_type.clone(),
                    title: p.title.clone(),
                    description: p.description.clone(),
                    icon_url: p.icon_url.clone(),
                    downloads: p.downloads,
                    categories: p.loaders.clone(),
                    game_versions: p.game_versions.clone(),
                    server_side: p.server_side,
                })
                .collect();
            let total_hits = hits.len() as u32;
            Ok(ContentSearchResult {
                hits,
                offset: filters.offset,
                limit: filters.limit,
                total_hits,
            })
        })
    }

    fn project<'a>(&'a self, id_or_slug: &'a str) -> BoxFut<'a, Result<ContentProject, AppError>> {
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
    ) -> BoxFut<'a, Result<Vec<ContentVersion>, AppError>> {
        Box::pin(async move {
            let expected = modrinth_loaders(loader);
            let mut list: Vec<ContentVersion> = self
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

    fn project_versions<'a>(
        &'a self,
        project: &'a str,
    ) -> BoxFut<'a, Result<Vec<ContentVersion>, AppError>> {
        Box::pin(async move {
            let mut list: Vec<ContentVersion> = self
                .versions
                .iter()
                .filter(|v| v.project_id == project)
                .cloned()
                .collect();
            list.sort_by_key(|v| std::cmp::Reverse(v.date_published));
            Ok(list)
        })
    }

    fn version<'a>(&'a self, id: &'a str) -> BoxFut<'a, Result<ContentVersion, AppError>> {
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
        file: &'a ContentFileInfo,
        dest: &'a Path,
        _: ProgressFn<'a>,
        _: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(async move {
            let url = file
                .url
                .as_deref()
                .ok_or_else(|| AppError::new(ErrorCode::ContentNotFound, &file.filename))?;
            let bytes = self
                .files
                .get(url)
                .ok_or_else(|| AppError::new(ErrorCode::ContentNotFound, url))?;
            std::fs::write(dest, bytes).unwrap();
            Ok(())
        })
    }
}

fn file_info(
    url: Option<String>,
    name: &str,
    size: u64,
    manual_download: bool,
    download_page_url: Option<String>,
) -> ContentFileInfo {
    ContentFileInfo {
        url,
        filename: name.to_owned(),
        size,
        sha1: Some("0".repeat(40)),
        sha512: Some("0".repeat(128)),
        manual_download,
        download_page_url,
    }
}

fn req_dep(pid: &str) -> ContentDependency {
    ContentDependency {
        project_id: Some(pid.to_owned()),
        version_id: None,
        dependency_type: DependencyType::Required,
    }
}

fn project(
    source: CatalogSource,
    id: &str,
    kind: &str,
    title: &str,
    loaders: &[&str],
    server_side: Option<SideSupport>,
) -> ContentProject {
    let is_cf = source == CatalogSource::Curseforge;
    ContentProject {
        source,
        id: id.to_owned(),
        slug: id.to_owned(),
        project_type: kind.to_owned(),
        title: title.to_owned(),
        description: format!("{title} desc"),
        body: (!is_cf).then(|| format!("# {title}")),
        icon_url: Some("https://cdn.modrinth.com/icon.png".to_owned()),
        downloads: 100,
        loaders: loaders.iter().map(|s| (*s).to_owned()).collect(),
        game_versions: vec!["1.21.4".to_owned()],
        client_side: (!is_cf).then_some(SideSupport::Required),
        server_side,
        page_url: if is_cf {
            format!("https://www.curseforge.com/minecraft/mc-mods/{id}")
        } else {
            format!("https://modrinth.com/{kind}/{id}")
        },
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
    num: Option<&str>,
    loaders: &[&str],
    date_published: time::OffsetDateTime,
    file: ContentFileInfo,
    dependencies: Vec<ContentDependency>,
) -> ContentVersion {
    ContentVersion {
        id: id.to_owned(),
        project_id: pid.to_owned(),
        name: num.map_or_else(|| id.to_owned(), |n| format!("v{n}")),
        version_number: num.map(str::to_owned),
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
