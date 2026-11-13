//! Dựng lệnh chạy Java (spec P1, mục 5.5).

use std::path::PathBuf;

use mh_core::{ArgsOs, Launch, MemoryLimits};

use crate::ProcessSpec;

/// Java trên Windows ghi ra pipe theo code page của hệ thống; thiếu các cờ này là tiếng Việt
/// trong log bị vỡ.
pub const ENCODING_FLAGS: [&str; 5] = [
    "-Dfile.encoding=UTF-8",
    "-Dstdout.encoding=UTF-8",
    "-Dstderr.encoding=UTF-8",
    "-Dsun.stdout.encoding=UTF-8",
    "-Dsun.stderr.encoding=UTF-8",
];

#[derive(Debug, Clone)]
pub struct JavaLaunch {
    pub java_bin: PathBuf,
    /// `servers/<id>/server/`.
    pub work_dir: PathBuf,
    pub memory: MemoryLimits,
    pub jvm_args: Vec<String>,
    pub launch: Launch,
    /// Tham số sau jar hoặc file tham số, ví dụ `--nogui`.
    pub args: Vec<String>,
}

impl JavaLaunch {
    /// `<java> -Xms -Xmx <encoding> <jvm_args> -jar <jar> <args>`, hoặc `@<file>` thay cho
    /// `-jar <jar>` với Forge, NeoForge từ 1.17 (spec P3, mục 3.2). File tham số lấy theo OS đang
    /// chạy, nên server tạo trên Linux chép sang Windows vẫn bật được.
    pub fn into_spec(self) -> ProcessSpec {
        self.into_spec_for(ArgsOs::current())
    }

    fn into_spec_for(self, os: ArgsOs) -> ProcessSpec {
        let mut args = vec![
            format!("-Xms{}M", self.memory.min_mb()),
            format!("-Xmx{}M", self.memory.max_mb()),
        ];
        args.extend(ENCODING_FLAGS.map(str::to_owned));
        args.extend(self.jvm_args);
        match self.launch {
            Launch::Jar(jar) => {
                args.push("-jar".to_owned());
                args.push(jar.as_str().to_owned());
            }
            // Java đọc `@file` tương đối với thư mục làm việc, tức thư mục server.
            Launch::ArgsFile(path) => args.push(format!("@{}", path.for_os(os))),
        }
        args.extend(self.args);
        ProcessSpec {
            program: self.java_bin,
            args,
            work_dir: self.work_dir,
        }
    }
}

#[cfg(test)]
mod tests {
    use mh_core::ArgsFilePath;
    use mh_core::instance::JarName;

    use super::*;

    #[test]
    fn java_command_order() {
        let spec = JavaLaunch {
            java_bin: PathBuf::from("java/21/bin/java"),
            work_dir: PathBuf::from("servers/x/server"),
            memory: MemoryLimits::new(1024, 4096).unwrap(),
            jvm_args: vec!["-XX:+UseG1GC".to_owned()],
            launch: Launch::Jar(JarName::server_jar()),
            args: vec!["--nogui".to_owned()],
        }
        .into_spec();
        assert_eq!(spec.program, PathBuf::from("java/21/bin/java"));
        assert_eq!(
            spec.args,
            [
                "-Xms1024M",
                "-Xmx4096M",
                "-Dfile.encoding=UTF-8",
                "-Dstdout.encoding=UTF-8",
                "-Dstderr.encoding=UTF-8",
                "-Dsun.stdout.encoding=UTF-8",
                "-Dsun.stderr.encoding=UTF-8",
                "-XX:+UseG1GC",
                "-jar",
                "server.jar",
                "--nogui",
            ]
        );
    }

    #[test]
    fn args_file_replaces_jar_per_os() {
        let path = ArgsFilePath::parse("libraries/net/neoforged/neoforge/21.4.140/{os}_args.txt")
            .unwrap();
        let launch = |os| {
            JavaLaunch {
                java_bin: PathBuf::from("java"),
                work_dir: PathBuf::from("server"),
                memory: MemoryLimits::new(1024, 2048).unwrap(),
                jvm_args: Vec::new(),
                launch: Launch::ArgsFile(path.clone()),
                args: vec!["nogui".to_owned()],
            }
            .into_spec_for(os)
            .args
        };
        let unix = launch(ArgsOs::Unix);
        assert_eq!(
            unix[unix.len() - 2..],
            [
                "@libraries/net/neoforged/neoforge/21.4.140/unix_args.txt",
                "nogui"
            ]
        );
        assert!(!unix.iter().any(|a| a == "-jar"));
        let win = launch(ArgsOs::Windows);
        assert_eq!(
            win[win.len() - 2],
            "@libraries/net/neoforged/neoforge/21.4.140/win_args.txt"
        );
    }
}
