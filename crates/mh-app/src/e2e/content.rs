//! Nguồn Modrinth giả cho chế độ e2e (spec P3, mục 5.9). Không gọi mạng.

use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::Path;

use mh_core::{ErrorCode, LoaderKind, McVersion};
use mh_providers::modpack::PackFile;
use time::macros::datetime;

use super::pack::{
    add_e2e_modpack, build_jar, make_project, make_version, sha512_hex, verify_bytes_checksum,
};
use crate::AppError;
use crate::content::{
    ContentCatalog, Dependency, DependencyType, FileHashes, ModrinthVersion, Project,
    SearchFilters, SearchHit, SearchResult, SideSupport, VersionFile, modrinth_loaders,
};
use crate::sources::{BoxFut, CancellationToken, ProgressFn};

pub(super) const GAME_VERSIONS: &[&str] = &["1.21.4", "1.20.4"];

#[derive(Debug)]
pub(super) struct E2eContent {
    projects: Vec<Project>,
    versions: Vec<ModrinthVersion>,
    files: HashMap<String, Vec<u8>>,
}

impl E2eContent {
    pub(super) fn new() -> Self {
        let mut files = HashMap::new();
        let mut register = |ver_id: &str, filename: &str, entry: &str, body: &str| -> VersionFile {
            let bytes = build_jar(entry, body);
            let url = format!("e2e://modrinth/{ver_id}/{filename}");
            let file = VersionFile {
                url: url.clone(),
                filename: filename.to_owned(),
                size: bytes.len() as u64,
                hashes: FileHashes {
                    sha1: "0".repeat(40),
                    sha512: sha512_hex(&bytes),
                },
                primary: true,
            };
            files.insert(url, bytes);
            file
        };

        let fabric_api_file = register(
            "e2e-fabric-api-0.110.0",
            "fabric-api-0.110.0.jar",
            "fabric.mod.json",
            r#"{"schemaVersion":1,"id":"fabric-api","version":"0.110.0","name":"Fabric API","environment":"*"}"#,
        );
        let lithium_old_file = register(
            "e2e-lithium-0.14.0",
            "lithium-0.14.0.jar",
            "fabric.mod.json",
            r#"{"schemaVersion":1,"id":"lithium","version":"0.14.0","name":"Lithium","environment":"*","depends":{"fabric-api":"*"}}"#,
        );
        let lithium_new_file = register(
            "e2e-lithium-0.14.2",
            "lithium-0.14.2.jar",
            "fabric.mod.json",
            r#"{"schemaVersion":1,"id":"lithium","version":"0.14.2","name":"Lithium","environment":"*","depends":{"fabric-api":"*"}}"#,
        );
        let iris_file = register(
            "e2e-iris-1.8.0",
            "iris-1.8.0.jar",
            "fabric.mod.json",
            r#"{"schemaVersion":1,"id":"iris","version":"1.8.0","name":"Iris Shaders","environment":"client"}"#,
        );
        let luckperms_file = register(
            "e2e-luckperms-5.4.0",
            "luckperms-5.4.0.jar",
            "plugin.yml",
            "name: LuckPerms\nversion: 5.4.0\nmain: me.lucko.luckperms.LP\n",
        );

        let mut projects = vec![
            make_project(
                "e2e-fabric-api",
                "fabric-api",
                "mod",
                "Fabric API",
                "Thư viện nền cho mod Fabric.",
                1_000_000,
                &["fabric"],
                SideSupport::Required,
                SideSupport::Required,
            ),
            make_project(
                "e2e-lithium",
                "lithium",
                "mod",
                "Lithium",
                "Tối ưu hiệu năng server Fabric.",
                500_000,
                &["fabric"],
                SideSupport::Optional,
                SideSupport::Optional,
            ),
            make_project(
                "e2e-iris",
                "iris",
                "mod",
                "Iris Shaders",
                "Mod shader chỉ chạy phía client.",
                400_000,
                &["fabric"],
                SideSupport::Required,
                SideSupport::Unsupported,
            ),
            make_project(
                "e2e-luckperms",
                "luckperms",
                "plugin",
                "LuckPerms",
                "Plugin phân quyền cho Paper.",
                800_000,
                &["paper", "purpur", "spigot", "bukkit"],
                SideSupport::Unsupported,
                SideSupport::Required,
            ),
        ];

        let fabric_dep = vec![Dependency {
            project_id: Some("e2e-fabric-api".to_owned()),
            version_id: None,
            dependency_type: DependencyType::Required,
        }];
        let mut versions = vec![
            make_version(
                "e2e-fabric-api-0.110.0",
                "e2e-fabric-api",
                "0.110.0",
                &["fabric"],
                datetime!(2024-12-10 0:00 UTC),
                fabric_api_file.clone(),
                Vec::new(),
            ),
            make_version(
                "e2e-lithium-0.14.2",
                "e2e-lithium",
                "0.14.2",
                &["fabric"],
                datetime!(2024-12-12 0:00 UTC),
                lithium_new_file.clone(),
                fabric_dep.clone(),
            ),
            make_version(
                "e2e-lithium-0.14.0",
                "e2e-lithium",
                "0.14.0",
                &["fabric"],
                datetime!(2024-12-05 0:00 UTC),
                lithium_old_file,
                fabric_dep,
            ),
            make_version(
                "e2e-iris-1.8.0",
                "e2e-iris",
                "1.8.0",
                &["fabric"],
                datetime!(2024-12-08 0:00 UTC),
                iris_file.clone(),
                Vec::new(),
            ),
            make_version(
                "e2e-luckperms-5.4.0",
                "e2e-luckperms",
                "5.4.0",
                &["paper", "purpur", "spigot", "bukkit"],
                datetime!(2024-12-09 0:00 UTC),
                luckperms_file.clone(),
                Vec::new(),
            ),
        ];

        add_e2e_modpack(
            &mut projects,
            &mut versions,
            &mut files,
            &fabric_api_file,
            &lithium_new_file,
            &iris_file,
        );

        Self {
            projects,
            versions,
            files,
        }
    }

    fn find_project(&self, id_or_slug: &str) -> Result<&Project, AppError> {
        self.projects
            .iter()
            .find(|p| p.id == id_or_slug || p.slug == id_or_slug)
            .ok_or_else(|| AppError::new(ErrorCode::ContentNotFound, id_or_slug))
    }

    fn search_sync(&self, query: &str, filters: &SearchFilters) -> SearchResult {
        let needle = query.trim().to_ascii_lowercase();
        let expected_loaders = filters.loader.map(modrinth_loaders);
        let mut matched: Vec<SearchHit> = self
            .projects
            .iter()
            .filter(|p| {
                filters
                    .project_type
                    .is_none_or(|k| p.project_type == k.as_str())
                    && expected_loaders.is_none_or(|ls| {
                        !ls.is_empty() && p.loaders.iter().any(|l| ls.contains(&l.as_str()))
                    })
                    && filters
                        .minecraft
                        .as_ref()
                        .is_none_or(|mc| p.game_versions.iter().any(|g| g == mc.as_str()))
                    && (!filters.server_only || !p.server_side.is_unsupported())
                    && (needle.is_empty()
                        || p.title.to_ascii_lowercase().contains(&needle)
                        || p.slug.to_ascii_lowercase().contains(&needle)
                        || p.description.to_ascii_lowercase().contains(&needle))
            })
            .map(project_to_hit)
            .collect();
        matched.sort_by_key(|h| Reverse(h.downloads));
        let total_hits = matched.len() as u32;
        let limit = if filters.limit == 0 {
            20
        } else {
            filters.limit
        };
        let hits = matched
            .into_iter()
            .skip(filters.offset as usize)
            .take(limit as usize)
            .collect();
        SearchResult {
            hits,
            offset: filters.offset,
            limit,
            total_hits,
        }
    }
}

impl ContentCatalog for E2eContent {
    fn search<'a>(
        &'a self,
        query: &'a str,
        filters: &'a SearchFilters,
    ) -> BoxFut<'a, Result<SearchResult, AppError>> {
        Box::pin(std::future::ready(Ok(self.search_sync(query, filters))))
    }

    fn project<'a>(&'a self, id_or_slug: &'a str) -> BoxFut<'a, Result<Project, AppError>> {
        Box::pin(std::future::ready(self.find_project(id_or_slug).cloned()))
    }

    fn versions<'a>(
        &'a self,
        project: &'a str,
        loader: LoaderKind,
        minecraft: &'a McVersion,
    ) -> BoxFut<'a, Result<Vec<ModrinthVersion>, AppError>> {
        Box::pin(std::future::ready((|| {
            let pid = self.find_project(project)?.id.as_str();
            let expected = modrinth_loaders(loader);
            let mut list: Vec<ModrinthVersion> = self
                .versions
                .iter()
                .filter(|v| {
                    v.project_id == pid
                        && v.game_versions.iter().any(|g| g == minecraft.as_str())
                        && v.loaders.iter().any(|l| expected.contains(&l.as_str()))
                })
                .cloned()
                .collect();
            list.sort_by_key(|v| Reverse(v.date_published));
            Ok(list)
        })()))
    }

    fn version<'a>(&'a self, id: &'a str) -> BoxFut<'a, Result<ModrinthVersion, AppError>> {
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
        file: &'a VersionFile,
        dest: &'a Path,
        progress: ProgressFn<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFut<'a, Result<(), AppError>> {
        Box::pin(std::future::ready((|| {
            if cancel.is_cancelled() {
                return Err(AppError::cancelled());
            }
            let bytes = self
                .files
                .get(&file.url)
                .ok_or_else(|| AppError::new(ErrorCode::ContentNotFound, &file.url))?;
            let size = bytes.len() as u64;
            progress(size, Some(size));
            mh_platform::fs::write_atomic(dest, bytes)?;
            Ok(())
        })()))
    }

    fn project_versions<'a>(
        &'a self,
        project: &'a str,
    ) -> BoxFut<'a, Result<Vec<ModrinthVersion>, AppError>> {
        Box::pin(std::future::ready((|| {
            let pid = self.find_project(project)?.id.as_str();
            let mut list: Vec<ModrinthVersion> = self
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
        Box::pin(std::future::ready((|| {
            if cancel.is_cancelled() {
                return Err(AppError::cancelled());
            }
            let bytes = file
                .urls
                .iter()
                .find_map(|url| self.files.get(url))
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
        })()))
    }
}

fn project_to_hit(p: &Project) -> SearchHit {
    SearchHit {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::ProjectType;

    #[tokio::test]
    async fn e2e_content_provides_fabric_dep_client_only_and_paper_plugin() {
        let cat = E2eContent::new();
        let mc = McVersion::parse("1.21.4").unwrap();

        let fabric_hits = cat
            .search(
                "",
                &SearchFilters {
                    project_type: Some(ProjectType::Mod),
                    loader: Some(LoaderKind::Fabric),
                    minecraft: Some(mc.clone()),
                    server_only: true,
                    ..SearchFilters::default()
                },
            )
            .await
            .unwrap();
        let ids: Vec<&str> = fabric_hits
            .hits
            .iter()
            .map(|h| h.project_id.as_str())
            .collect();
        assert_eq!(ids, ["e2e-fabric-api", "e2e-lithium"]);

        // Iris chỉ chạy client: vắng mặt khi `server_only`, nhưng có trong project/search chung.
        let iris = cat.project("e2e-iris").await.unwrap();
        assert!(iris.server_side.is_unsupported());

        let lithium_vers = cat
            .versions("e2e-lithium", LoaderKind::Fabric, &mc)
            .await
            .unwrap();
        assert_eq!(
            lithium_vers
                .iter()
                .map(|v| v.version_number.as_str())
                .collect::<Vec<_>>(),
            ["0.14.2", "0.14.0"]
        );
        assert_eq!(
            lithium_vers[0].dependencies[0].project_id.as_deref(),
            Some("e2e-fabric-api")
        );

        let paper_hits = cat
            .search(
                "",
                &SearchFilters {
                    project_type: Some(ProjectType::Plugin),
                    loader: Some(LoaderKind::Paper),
                    minecraft: Some(mc),
                    server_only: true,
                    ..SearchFilters::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(paper_hits.hits.len(), 1);
        assert_eq!(paper_hits.hits[0].project_id, "e2e-luckperms");

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("lithium.jar");
        let cancel = CancellationToken::new();
        cat.download(&lithium_vers[0].files[0], &dest, &mut |_, _| {}, &cancel)
            .await
            .unwrap();
        let info = mh_store::inspect_jar_file(&dest).unwrap();
        assert_eq!(info.meta.unwrap().id, "lithium");
    }
}
