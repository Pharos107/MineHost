//! Nguồn CurseForge giả cho chế độ e2e (không gọi mạng): mod có dependency, mod cấm phân phối,
//! một modpack chứa mod cấm phân phối và một modpack có server pack lồng một tầng.

use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::Path;

use mh_core::{CfManifestFile, ErrorCode, LoaderKind, McVersion};
use mh_net::{Checksum, HostAllowlist, hosts};
use mh_providers::curseforge::mod_loader_type;
use mh_providers::modpack::{BlockedPackFile, PackFile};
use time::macros::datetime;

use super::content::build_jar;
use super::pack::{add_e2e_cf_modpacks, cf_project, cf_version, download_e2e_pack_file};
use super::stored_zip::build_stored_jar;
use crate::AppError;
use crate::content::{
    CatalogSource, CfResolvedFiles, ContentCatalog, ContentDependency, ContentFileInfo,
    ContentProject, ContentSearchHit, ContentSearchResult, ContentVersion, DependencyType,
    ProjectType, SearchFilters,
};
use crate::sources::{BoxFut, CancellationToken, ProgressFn};

#[derive(Debug)]
pub(super) struct E2eCurseforge {
    projects: Vec<ContentProject>,
    versions: Vec<ContentVersion>,
    files: HashMap<String, Vec<u8>>,
    server_packs: HashMap<String, ContentFileInfo>,
}

impl E2eCurseforge {
    pub(super) fn new() -> Self {
        let mut files = HashMap::new();
        let mut register = |ver_id: &str, filename: &str, mod_id: &str, name: &str| {
            let body = format!(
                r#"{{"schemaVersion":1,"id":"{mod_id}","version":"1.0.0","name":"{name}","environment":"*"}}"#
            );
            let bytes = build_jar("fabric.mod.json", &body);
            let url = format!("e2e://curseforge/{ver_id}/{filename}");
            let sha1 = mh_store::sha1_hex(&bytes);
            let info = ContentFileInfo {
                url: Some(url.clone()),
                filename: filename.to_owned(),
                size: bytes.len() as u64,
                sha1: Some(sha1),
                sha512: None,
                manual_download: false,
                download_page_url: Some(format!(
                    "https://www.curseforge.com/minecraft/mc-mods/cf-mod/download/{ver_id}"
                )),
            };
            files.insert(url, bytes);
            info
        };

        let cf_fapi_file = register(
            "5800000",
            "cf-fabric-api-0.110.0.jar",
            "cf-fabric-api",
            "Fabric API (CF)",
        );
        let cf_jei_file = register("5800001", "cf-jei-19.21.0.jar", "jei", "Just Enough Items");
        let restricted_bytes = build_stored_jar(
            "fabric.mod.json",
            br#"{"schemaVersion":1,"id":"restricted-mod","version":"1.0.0","name":"Restricted Mod","environment":"*"}"#,
        );
        let cf_fapi_bytes = cf_fapi_file
            .url
            .as_deref()
            .and_then(|u| files.get(u))
            .cloned()
            .unwrap_or_default();
        let restricted_file = ContentFileInfo {
            url: None,
            filename: "cf-restricted-1.0.0.jar".to_owned(),
            size: restricted_bytes.len() as u64,
            sha1: Some(mh_store::sha1_hex(&restricted_bytes)),
            sha512: None,
            manual_download: true,
            download_page_url: Some(
                "https://www.curseforge.com/minecraft/mc-mods/restricted-mod/download/5800999"
                    .to_owned(),
            ),
        };

        let mut projects = vec![
            cf_project(
                "306612",
                "fabric-api",
                "mod",
                "Fabric API (CF)",
                "Thư viện nền CF cho Fabric.",
                2_000_000,
            ),
            cf_project(
                "238222",
                "jei",
                "mod",
                "Just Enough Items (JEI)",
                "Xem công thức chế tạo (có dep).",
                1_500_000,
            ),
            cf_project(
                "999001",
                "restricted-mod",
                "mod",
                "Restricted Mod",
                "Mod cấm tải tự động qua API.",
                300_000,
            ),
        ];

        let mut versions = vec![
            cf_version(
                "5800000",
                "306612",
                "cf-fabric-api-0.110.0",
                datetime!(2024-12-10 0:00 UTC),
                cf_fapi_file,
                Vec::new(),
            ),
            cf_version(
                "5800001",
                "238222",
                "jei-fabric-19.21.0",
                datetime!(2024-12-12 0:00 UTC),
                cf_jei_file,
                vec![ContentDependency {
                    project_id: Some("306612".to_owned()),
                    version_id: None,
                    dependency_type: DependencyType::Required,
                }],
            ),
            cf_version(
                "5800999",
                "999001",
                "restricted-1.0.0",
                datetime!(2024-12-11 0:00 UTC),
                restricted_file,
                Vec::new(),
            ),
        ];

        let mut server_packs = HashMap::new();
        add_e2e_cf_modpacks(
            &mut projects,
            &mut versions,
            &mut files,
            &mut server_packs,
            &cf_fapi_bytes,
        );

        Self {
            projects,
            versions,
            files,
            server_packs,
        }
    }

    fn find_project(&self, id: &str) -> Result<&ContentProject, AppError> {
        self.projects
            .iter()
            .find(|p| p.id == id || p.slug == id)
            .ok_or_else(|| AppError::new(ErrorCode::ContentNotFound, id))
    }

    fn search_sync(&self, query: &str, filters: &SearchFilters) -> ContentSearchResult {
        let limit = if filters.limit == 0 {
            20
        } else {
            filters.limit
        };
        if matches!(filters.project_type, Some(ProjectType::Plugin))
            || filters.loader.is_some_and(|l| mod_loader_type(l).is_none())
        {
            return ContentSearchResult {
                hits: Vec::new(),
                offset: filters.offset,
                limit,
                total_hits: 0,
            };
        }
        let target_type = filters.project_type.map_or("mod", ProjectType::as_str);
        let needle = query.trim().to_ascii_lowercase();
        let mut matched: Vec<ContentSearchHit> = self
            .projects
            .iter()
            .filter(|p| {
                p.project_type == target_type
                    && (needle.is_empty()
                        || p.title.to_ascii_lowercase().contains(&needle)
                        || p.slug.to_ascii_lowercase().contains(&needle)
                        || p.description.to_ascii_lowercase().contains(&needle))
            })
            .map(|p| ContentSearchHit {
                source: CatalogSource::Curseforge,
                project_id: p.id.clone(),
                slug: p.slug.clone(),
                project_type: p.project_type.clone(),
                title: p.title.clone(),
                description: p.description.clone(),
                icon_url: None,
                downloads: p.downloads,
                categories: Vec::new(),
                game_versions: Vec::new(),
                server_side: None,
            })
            .collect();
        matched.sort_by_key(|h| Reverse(h.downloads));
        let total_hits = matched.len() as u32;
        let hits = matched
            .into_iter()
            .skip(filters.offset as usize)
            .take(limit as usize)
            .collect();
        ContentSearchResult {
            hits,
            offset: filters.offset,
            limit,
            total_hits,
        }
    }
}

impl ContentCatalog for E2eCurseforge {
    fn search<'a>(
        &'a self,
        query: &'a str,
        filters: &'a SearchFilters,
    ) -> BoxFut<'a, Result<ContentSearchResult, AppError>> {
        Box::pin(std::future::ready(Ok(self.search_sync(query, filters))))
    }

    fn project<'a>(&'a self, id_or_slug: &'a str) -> BoxFut<'a, Result<ContentProject, AppError>> {
        Box::pin(std::future::ready(self.find_project(id_or_slug).cloned()))
    }

    fn versions<'a>(
        &'a self,
        project: &'a str,
        loader: LoaderKind,
        minecraft: &'a McVersion,
    ) -> BoxFut<'a, Result<Vec<ContentVersion>, AppError>> {
        Box::pin(std::future::ready((|| {
            if mod_loader_type(loader).is_none() {
                return Ok(Vec::new());
            }
            let pid = self.find_project(project)?.id.as_str();
            let mut list: Vec<ContentVersion> = self
                .versions
                .iter()
                .filter(|v| {
                    v.project_id == pid
                        && v.game_versions.iter().any(|g| g == minecraft.as_str())
                        && v.loaders.iter().any(|l| l == loader.as_str())
                })
                .cloned()
                .collect();
            list.sort_by_key(|v| Reverse(v.date_published));
            Ok(list)
        })()))
    }

    fn version<'a>(&'a self, id: &'a str) -> BoxFut<'a, Result<ContentVersion, AppError>> {
        Box::pin(std::future::ready(
            self.versions
                .iter()
                .find(|v| v.id == id)
                .cloned()
                .ok_or_else(|| AppError::new(ErrorCode::ContentNotFound, id)),
        ))
    }

    fn download<'a>(
        &'a self,
        file: &'a ContentFileInfo,
        dest: &'a Path,
        progress: ProgressFn<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(std::future::ready((|| {
            if cancel.is_cancelled() {
                return Err(AppError::cancelled());
            }
            let url = file
                .url
                .as_deref()
                .ok_or_else(|| AppError::new(ErrorCode::ContentNotFound, &file.filename))?;
            let bytes = self
                .files
                .get(url)
                .ok_or_else(|| AppError::new(ErrorCode::ContentNotFound, url))?;
            let size = bytes.len() as u64;
            progress(size, Some(size));
            mh_platform::fs::write_atomic(dest, bytes)?;
            Ok(())
        })()))
    }

    fn project_versions<'a>(
        &'a self,
        project: &'a str,
    ) -> BoxFut<'a, Result<Vec<ContentVersion>, AppError>> {
        Box::pin(std::future::ready((|| {
            let pid = self.find_project(project)?.id.as_str();
            let mut list: Vec<ContentVersion> = self
                .versions
                .iter()
                .filter(|v| v.project_id == pid)
                .cloned()
                .collect();
            list.sort_by_key(|v| Reverse(v.date_published));
            Ok(list)
        })()))
    }

    fn download_pack_file<'a>(
        &'a self,
        file: &'a PackFile,
        dest: &'a Path,
        progress: ProgressFn<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(std::future::ready(download_e2e_pack_file(
            &self.files,
            file,
            dest,
            progress,
            cancel,
        )))
    }

    fn pack_hosts(&self) -> HostAllowlist {
        hosts::CURSEFORGE
    }

    fn curseforge_server_pack<'a>(
        &'a self,
        file_id: &'a str,
    ) -> BoxFut<'a, Result<Option<ContentFileInfo>, AppError>> {
        Box::pin(std::future::ready(Ok(self
            .server_packs
            .get(file_id)
            .cloned())))
    }

    fn resolve_curseforge_files<'a>(
        &'a self,
        files: &'a [CfManifestFile],
    ) -> BoxFut<'a, Result<CfResolvedFiles, AppError>> {
        Box::pin(std::future::ready((|| {
            let mut pack_files = Vec::new();
            let mut blocked = Vec::new();
            for entry in files {
                let vid = entry.file_id.to_string();
                let Some(ver) = self.versions.iter().find(|v| v.id == vid) else {
                    if entry.required {
                        return Err(AppError::new(ErrorCode::ContentNotFound, vid));
                    }
                    continue;
                };
                let f = &ver.files[0];
                let path = format!("mods/{}", f.filename);
                if let (false, Some(url)) = (f.manual_download, f.url.clone()) {
                    let sha1 = f.sha1.clone().unwrap_or_default();
                    let checksum = Checksum::sha1(&sha1).ok_or_else(|| {
                        AppError::new(ErrorCode::ModpackInvalid, "sha1 không hợp lệ")
                    })?;
                    pack_files.push(PackFile {
                        path,
                        urls: vec![url],
                        checksum,
                        sha1,
                        size: f.size,
                        hosts: hosts::CURSEFORGE,
                        cf_ids: Some((entry.project_id, entry.file_id)),
                    });
                } else {
                    blocked.push(BlockedPackFile {
                        path,
                        project_id: entry.project_id,
                        file_id: entry.file_id,
                        sha1: f.sha1.clone().unwrap_or_default(),
                        page_url: f.download_page_url.clone().unwrap_or_default(),
                    });
                }
            }
            Ok((pack_files, blocked))
        })()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn e2e_curseforge_provides_dependency_and_restricted_mod() {
        let cf = E2eCurseforge::new();
        let mc = McVersion::parse("1.21.4").unwrap();
        let hits = cf
            .search(
                "",
                &SearchFilters {
                    loader: Some(LoaderKind::Fabric),
                    minecraft: Some(mc.clone()),
                    ..SearchFilters::default()
                },
            )
            .await
            .unwrap();
        let ids: Vec<&str> = hits.hits.iter().map(|h| h.project_id.as_str()).collect();
        assert_eq!(ids, ["306612", "238222", "999001"]);

        let packs = cf
            .search(
                "",
                &SearchFilters {
                    project_type: Some(ProjectType::Modpack),
                    ..SearchFilters::default()
                },
            )
            .await
            .unwrap();
        let pack_ids: Vec<&str> = packs.hits.iter().map(|h| h.project_id.as_str()).collect();
        assert_eq!(pack_ids, ["910002", "910001"]);

        let jei_vers = cf
            .versions("238222", LoaderKind::Fabric, &mc)
            .await
            .unwrap();
        assert_eq!(
            jei_vers[0].dependencies[0].project_id.as_deref(),
            Some("306612")
        );

        let restricted = cf
            .versions("999001", LoaderKind::Fabric, &mc)
            .await
            .unwrap();
        assert!(restricted[0].files[0].manual_download);
        assert!(restricted[0].files[0].download_page_url.is_some());

        let tmp = tempfile::tempdir().unwrap();
        let jar_path = tmp.path().join("restricted.jar");
        let jar_bytes = build_stored_jar(
            "fabric.mod.json",
            br#"{"schemaVersion":1,"id":"restricted-mod","version":"1.0.0","name":"Restricted Mod","environment":"*"}"#,
        );
        std::fs::write(&jar_path, &jar_bytes).unwrap();
        let info = mh_store::inspect_jar_file(&jar_path).unwrap();
        assert_eq!(
            restricted[0].files[0].sha1.as_deref(),
            Some(info.sha1.as_str())
        );
        assert_eq!(info.meta.unwrap().id, "restricted-mod");
    }
}
