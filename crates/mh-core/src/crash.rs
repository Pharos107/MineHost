//! Đoán nguyên nhân server crash từ các dòng log cuối (spec P1, mục 5.6).

use serde::{Deserialize, Serialize};

/// Nguyên nhân crash đoán được, gửi kèm `SERVER_CRASHED` để UI gợi ý cách sửa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum CrashHint {
    /// Gợi ý tăng RAM cho server.
    NotEnoughMemory,
    /// Java sai phiên bản. Không nên xảy ra vì MineHost tự chọn Java; gặp thì là lỗi của MineHost.
    WrongJava,
    PortInUse,
    EulaNotAccepted,
    /// Lỗi trong game hoặc plugin; người dùng cần xem log.
    GameError,
}

/// Dấu hiệu trong log, xếp theo độ ưu tiên: dấu hiệu cụ thể đứng trước, vì một lần crash
/// thường kéo theo nhiều dòng lỗi (ví dụ hết RAM rồi mới tới `Exception in server tick loop`).
const MARKERS: [(&str, CrashHint); 5] = [
    ("You need to agree to the EULA", CrashHint::EulaNotAccepted),
    ("FAILED TO BIND TO PORT", CrashHint::PortInUse),
    ("UnsupportedClassVersionError", CrashHint::WrongJava),
    ("java.lang.OutOfMemoryError", CrashHint::NotEnoughMemory),
    ("Exception in server tick loop", CrashHint::GameError),
];

/// Tìm dấu hiệu trong các dòng log cuối. `None` nếu không nhận ra.
pub fn diagnose<S: AsRef<str>>(lines: &[S]) -> Option<CrashHint> {
    MARKERS.iter().find_map(|(marker, hint)| {
        let found = lines.iter().any(|l| l.as_ref().contains(marker));
        found.then_some(*hint)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const OOM: &[&str] = &[
        "[10:01:02 ERROR]: Encountered an unexpected exception",
        "java.lang.OutOfMemoryError: Java heap space",
        "[10:01:02 ERROR]: Exception in server tick loop",
    ];
    const WRONG_JAVA: &[&str] = &[
        "Error: LinkageError occurred while loading main class io.papermc.paperclip.Main",
        "\tjava.lang.UnsupportedClassVersionError: io/papermc/paperclip/Main has been compiled \
         by a more recent version of the Java Runtime (class file version 65.0)",
    ];
    const PORT: &[&str] = &[
        "[10:00:05 WARN]: **** FAILED TO BIND TO PORT!",
        "[10:00:05 WARN]: The exception was: java.net.BindException: Address already in use",
    ];
    const EULA: &[&str] = &[
        "[10:00:01 INFO]: Loaded 1290 recipes",
        "[10:00:01 WARN]: Failed to load eula.txt",
        "[10:00:01 INFO]: You need to agree to the EULA in order to run the server. \
         Go to eula.txt for more info.",
    ];
    const TICK: &[&str] = &[
        "[10:30:00 ERROR]: Exception in server tick loop",
        "java.lang.NullPointerException: Cannot invoke \"Object.toString()\"",
    ];

    #[test]
    fn each_sample_log_gives_its_hint() {
        let cases = [
            (OOM, CrashHint::NotEnoughMemory),
            (WRONG_JAVA, CrashHint::WrongJava),
            (PORT, CrashHint::PortInUse),
            (EULA, CrashHint::EulaNotAccepted),
            (TICK, CrashHint::GameError),
        ];
        for (lines, hint) in cases {
            assert_eq!(diagnose(lines), Some(hint), "{lines:?}");
        }
    }

    #[test]
    fn unknown_log_gives_none() {
        let lines = [
            "[10:00:00 INFO]: Stopping server",
            "[10:00:01 INFO]: Saving worlds",
        ];
        assert_eq!(diagnose(&lines), None);
        assert_eq!(diagnose::<&str>(&[]), None);
    }

    #[test]
    fn works_with_owned_lines() {
        let lines: Vec<String> = PORT.iter().map(|s| (*s).to_owned()).collect();
        assert_eq!(diagnose(&lines), Some(CrashHint::PortInUse));
    }

    #[test]
    fn serde_names() {
        let json = serde_json::to_string(&CrashHint::NotEnoughMemory).unwrap();
        assert_eq!(json, "\"NOT_ENOUGH_MEMORY\"");
    }
}
