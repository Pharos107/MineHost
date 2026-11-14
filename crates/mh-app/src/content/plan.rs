//! Lập kế hoạch cài mod/plugin kèm dependency bắt buộc, không tải file.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use mh_core::{ContentKind, ErrorCode, LoaderKind, McVersion};
use mh_store::{ContentFile, ProjectId};

use super::catalog::{
    ContentCatalog, Dependency, DependencyType, ModrinthVersion, Project, VersionFile,
    modrinth_loaders,
};
use crate::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedContent {
    pub project_id: String,
    pub project_name: String,
    pub version_id: String,
    pub version_number: String,
    pub file: VersionFile,
    pub client_only: bool,
    pub replaces_existing: bool,
    pub is_dependency: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentInstallPlan {
    pub kind: ContentKind,
    pub items: Vec<PlannedContent>,
}

pub(crate) type InstalledMap = HashMap<String, Vec<Option<String>>>;

struct QueueItem {
    project_id: Option<String>,
    version_id: Option<String>,
    is_root: bool,
}

pub(crate) fn installed_projects(files: &[ContentFile]) -> InstalledMap {
    let mut map: InstalledMap = HashMap::new();
    for file in files {
        let Some(rec) = file.record.as_ref() else {
            continue;
        };
        if let Some(ProjectId::Str(pid)) = rec.project_id.as_ref() {
            map.entry(pid.clone())
                .or_default()
                .push(rec.version_id.clone());
        }
    }
    map
}

pub(crate) async fn build_plan(
    catalog: &dyn ContentCatalog,
    kind: ContentKind,
    loader: LoaderKind,
    mc: &McVersion,
    installed: &InstalledMap,
    root_project: &str,
    root_version: Option<&str>,
) -> Result<ContentInstallPlan, AppError> {
    let mut queue = VecDeque::from([QueueItem {
        project_id: Some(root_project.to_owned()),
        version_id: root_version.map(str::to_owned),
        is_root: true,
    }]);
    let mut visited = HashSet::new();
    let mut items = Vec::new();
    let mut incompatibles: Vec<(String, Dependency)> = Vec::new();

    while let Some(next) = queue.pop_front() {
        let is_dependency = !next.is_root;
        let Some((project, preloaded_ver)) = resolve_target(catalog, next, &visited).await? else {
            continue;
        };
        if !visited.insert(project.id.clone()) {
            continue;
        }
        // Mod chính vẫn được lập kế hoạch để thay bản cũ nếu đã cài; dependency đã có thì bỏ qua.
        if is_dependency && installed.contains_key(&project.id) {
            continue;
        }
        let ver = match preloaded_ver {
            Some(v) if is_compatible(&v, &project.id, loader, mc) => v,
            Some(_) => return Err(incompatible(&project.title)),
            None => catalog
                .versions(&project.id, loader, mc)
                .await?
                .into_iter()
                .next()
                .ok_or_else(|| incompatible(&project.title))?,
        };
        let file = primary_jar(&ver, &project.title)?;
        for dep in &ver.dependencies {
            match dep.dependency_type {
                DependencyType::Required => queue.push_back(QueueItem {
                    project_id: dep.project_id.clone(),
                    version_id: dep.version_id.clone(),
                    is_root: false,
                }),
                DependencyType::Incompatible => {
                    incompatibles.push((project.id.clone(), dep.clone()))
                }
                DependencyType::Optional | DependencyType::Embedded => {}
            }
        }
        items.push(PlannedContent {
            replaces_existing: installed.contains_key(&project.id),
            client_only: project.server_side.is_unsupported(),
            project_id: project.id,
            project_name: project.title,
            version_id: ver.id,
            version_number: ver.version_number,
            file,
            is_dependency,
        });
    }

    check_incompatibilities(&items, &incompatibles, installed)?;
    Ok(ContentInstallPlan { kind, items })
}

async fn resolve_target(
    catalog: &dyn ContentCatalog,
    next: QueueItem,
    visited: &HashSet<String>,
) -> Result<Option<(Project, Option<ModrinthVersion>)>, AppError> {
    match (next.project_id, next.version_id) {
        (Some(pid), vid) => {
            if visited.contains(&pid) {
                return Ok(None);
            }
            let project = catalog.project(&pid).await?;
            let ver = match vid {
                Some(id) => Some(catalog.version(&id).await?),
                None => None,
            };
            Ok(Some((project, ver)))
        }
        (None, Some(vid)) => {
            let ver = catalog.version(&vid).await?;
            if visited.contains(&ver.project_id) {
                return Ok(None);
            }
            let project = catalog.project(&ver.project_id).await?;
            Ok(Some((project, Some(ver))))
        }
        (None, None) if next.is_root => {
            Err(AppError::new(ErrorCode::InvalidInput, "thiếu project_id"))
        }
        (None, None) => Ok(None),
    }
}

fn is_compatible(ver: &ModrinthVersion, pid: &str, loader: LoaderKind, mc: &McVersion) -> bool {
    let expected = modrinth_loaders(loader);
    ver.project_id == pid
        && ver.game_versions.iter().any(|g| g == mc.as_str())
        && ver.loaders.iter().any(|l| expected.contains(&l.as_str()))
}

fn primary_jar(ver: &ModrinthVersion, name: &str) -> Result<VersionFile, AppError> {
    let file = ver
        .primary_file()
        .ok_or_else(|| incompatible(name))?
        .clone();
    crate::files::validate_file_name(&file.filename)?;
    if !Path::new(&file.filename)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("jar"))
    {
        return Err(incompatible(name));
    }
    Ok(file)
}

fn check_incompatibilities(
    items: &[PlannedContent],
    constraints: &[(String, Dependency)],
    installed: &InstalledMap,
) -> Result<(), AppError> {
    let planned_pids: HashSet<&str> = items.iter().map(|i| i.project_id.as_str()).collect();
    for (declarer, dep) in constraints {
        for item in items.iter().filter(|i| &i.project_id != declarer) {
            if matches_dep(dep, &item.project_id, Some(&item.version_id)) {
                return Err(incompatible(&item.project_name));
            }
        }
        for (inst_pid, inst_vers) in installed {
            if planned_pids.contains(inst_pid.as_str()) {
                continue;
            }
            let hit = inst_vers
                .iter()
                .any(|v| matches_dep(dep, inst_pid, v.as_deref()));
            if hit {
                return Err(incompatible(inst_pid));
            }
        }
    }
    Ok(())
}

fn matches_dep(dep: &Dependency, pid: &str, vid: Option<&str>) -> bool {
    let pid_ok = dep.project_id.as_deref().is_none_or(|p| p == pid);
    let vid_ok = dep.version_id.as_deref().is_none_or(|v| vid == Some(v));
    (dep.project_id.is_some() || dep.version_id.is_some()) && pid_ok && vid_ok
}

pub(crate) fn incompatible(what: &str) -> AppError {
    AppError::new(
        ErrorCode::ContentIncompatible,
        format!("`{what}` không tương thích với server"),
    )
}
