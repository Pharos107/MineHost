//! Dựng lệnh chạy Java (spec P1, mục 5.5).

use std::path::PathBuf;

use mh_core::MemoryLimits;
use mh_core::instance::JarName;

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
    pub jar: JarName,
    /// Tham số sau `-jar`, ví dụ `--nogui`.
    pub args: Vec<String>,
}

impl JavaLaunch {
    /// `<java> -Xms -Xmx <encoding> <jvm_args> -jar <jar> <args>`.
    pub fn into_spec(self) -> ProcessSpec {
        let mut args = vec![
            format!("-Xms{}M", self.memory.min_mb()),
            format!("-Xmx{}M", self.memory.max_mb()),
        ];
        args.extend(ENCODING_FLAGS.map(str::to_owned));
        args.extend(self.jvm_args);
        args.push("-jar".to_owned());
        args.push(self.jar.as_str().to_owned());
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
    use super::*;

    #[test]
    fn java_command_order() {
        let spec = JavaLaunch {
            java_bin: PathBuf::from("java/21/bin/java"),
            work_dir: PathBuf::from("servers/x/server"),
            memory: MemoryLimits::new(1024, 4096).unwrap(),
            jvm_args: vec!["-XX:+UseG1GC".to_owned()],
            jar: JarName::server_jar(),
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
}
