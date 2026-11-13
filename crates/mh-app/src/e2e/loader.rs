//! Mod loader giả cho chế độ e2e (spec P3, PR-09): installer là `fake-mc --install`, tạo đúng file
//! chạy server mà loader thật sẽ tạo. Server sau đó chạy bằng `fake-mc` như Paper giả.

use mh_core::instance::JarName;
use mh_core::{ArgsFilePath, ErrorCode, Launch, LoaderKind, LoaderVersion, McVersion};
use mh_net::hosts;

use crate::AppError;
use crate::sources::{Checksum, InstallPlan, InstallerRun, LoaderInstall, LoaderRelease, PlannedFile};

const INSTALLER: &str = ".minehost-install/e2e-installer.jar";
/// Đường dẫn file tham số giả của Forge, NeoForge.
const ARGS_FILE: &str = "libraries/e2e/{os}_args.txt";

/// Hai bản cho mỗi loader, bản ổn định đứng đầu, giống thứ tự của provider thật.
pub(super) fn versions(loader: LoaderKind) -> Result<Vec<LoaderRelease>, AppError> {
    let names: [(&str, bool); 2] = match loader {
        LoaderKind::Fabric => [("0.16.9", true), ("0.17.0-beta.1", false)],
        LoaderKind::Quilt => [("0.27.1", true), ("0.28.0-beta.2", false)],
        LoaderKind::Forge => [("54.1.0", true), ("54.1.2", false)],
        LoaderKind::NeoForge => [("21.4.140", true), ("21.4.141-beta", false)],
        _ => return Err(crate::sources::unsupported(loader)),
    };
    names
        .into_iter()
        .map(|(v, stable)| {
            Ok(LoaderRelease {
                version: LoaderVersion::parse(v)?,
                stable,
            })
        })
        .collect()
}

pub(super) fn plan(
    loader: LoaderKind,
    minecraft: &McVersion,
    wanted: Option<&LoaderVersion>,
    java_major: u16,
) -> Result<LoaderInstall, AppError> {
    let all = versions(loader)?;
    let pick = match wanted {
        Some(v) => all.iter().find(|r| &r.version == v),
        None => all.first(),
    };
    let Some(release) = pick else {
        return Err(AppError::new(
            ErrorCode::VersionNotFound,
            format!("không có {loader} bản đó trong catalog e2e"),
        ));
    };
    let (launch, writes) = launch_of(loader)?;
    let mut args = vec!["--install".to_owned()];
    for file in writes {
        args.extend(["--write".to_owned(), file]);
    }
    let checksum = Checksum::sha256(&"0".repeat(64))
        .ok_or_else(|| AppError::new(ErrorCode::Internal, "checksum giả không hợp lệ"))?;
    let plan = InstallPlan {
        minecraft: minecraft.clone(),
        loader_version: release.version.clone(),
        files: vec![PlannedFile {
            url: format!("e2e://{loader}/installer.jar"),
            checksum,
            size: None,
            dest: INSTALLER.to_owned(),
            hosts: hosts::LOADERS,
        }],
        installer: Some(InstallerRun {
            jar: INSTALLER.to_owned(),
            args,
        }),
        launch,
    };
    Ok(LoaderInstall { plan, java_major })
}

/// Cách chạy giả và các file installer giả phải tạo cho cách chạy đó.
fn launch_of(loader: LoaderKind) -> Result<(Launch, Vec<String>), AppError> {
    let jar = |name: &str| -> Result<(Launch, Vec<String>), AppError> {
        Ok((Launch::Jar(JarName::parse(name)?), vec![name.to_owned()]))
    };
    match loader {
        LoaderKind::Fabric => jar("fabric-server-launch.jar"),
        LoaderKind::Quilt => jar("quilt-server-launch.jar"),
        LoaderKind::Forge | LoaderKind::NeoForge => {
            let path = ArgsFilePath::parse(ARGS_FILE)?;
            let writes = vec![
                ARGS_FILE.replace("{os}", "unix"),
                ARGS_FILE.replace("{os}", "win"),
            ];
            Ok((Launch::ArgsFile(path), writes))
        }
        _ => Err(crate::sources::unsupported(loader)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mc() -> McVersion {
        McVersion::parse("1.21.4").unwrap()
    }

    #[test]
    fn every_mod_loader_has_a_fake_plan() {
        for loader in [
            LoaderKind::Fabric,
            LoaderKind::Quilt,
            LoaderKind::Forge,
            LoaderKind::NeoForge,
        ] {
            let install = plan(loader, &mc(), None, 21).unwrap();
            let run = install.plan.installer.unwrap();
            assert_eq!(run.args[0], "--install");
            match install.plan.launch {
                Launch::Jar(jar) => assert!(run.args.contains(&jar.as_str().to_owned())),
                Launch::ArgsFile(path) => {
                    for os in [mh_core::ArgsOs::Unix, mh_core::ArgsOs::Windows] {
                        assert!(run.args.contains(&path.for_os(os)), "{loader}");
                    }
                }
            }
        }
        assert!(plan(LoaderKind::Paper, &mc(), None, 21).is_err());
    }

    #[test]
    fn unknown_version_is_not_found() {
        let wanted = LoaderVersion::parse("9.9.9").unwrap();
        let err = plan(LoaderKind::Fabric, &mc(), Some(&wanted), 21).unwrap_err();
        assert_eq!(err.code, ErrorCode::VersionNotFound);
    }
}
