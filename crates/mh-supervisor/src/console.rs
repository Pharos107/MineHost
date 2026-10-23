//! Bộ đệm console và file log theo ngày.

use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use mh_core::{ConsoleLine, ConsoleStream};
use time::{Date, OffsetDateTime};

/// Bộ đệm vòng các dòng console, đọc được từ nhiều nơi mà không cần đi qua actor.
#[derive(Debug)]
pub struct Console {
    inner: Mutex<Inner>,
}

#[derive(Debug)]
struct Inner {
    lines: VecDeque<ConsoleLine>,
    capacity: usize,
    next_seq: u64,
    log: Option<DailyLog>,
}

impl Console {
    pub(crate) fn new(capacity: usize, logs_dir: Option<PathBuf>, keep_days: i64) -> Self {
        // Bắt đầu từ số micro giây hiện tại thay vì 1, để `seq` không lặp lại cả khi controller
        // khởi động lại và client vẫn giữ `seq` cũ (chỉ cần đồng hồ máy không chạy lùi).
        let now_us = OffsetDateTime::now_utc().unix_timestamp_nanos() / 1000;
        Self {
            inner: Mutex::new(Inner {
                lines: VecDeque::with_capacity(capacity.min(1024)),
                capacity: capacity.max(1),
                next_seq: u64::try_from(now_us).unwrap_or(1),
                log: logs_dir.map(|dir| DailyLog::new(dir, keep_days)),
            }),
        }
    }

    pub(crate) fn push(&self, stream: ConsoleStream, text: String) -> ConsoleLine {
        let mut inner = self.lock();
        let line = ConsoleLine {
            seq: inner.next_seq,
            stream,
            text,
        };
        inner.next_seq += 1;
        if let Some(log) = inner.log.as_mut() {
            log.write(&line);
        }
        if inner.lines.len() == inner.capacity {
            inner.lines.pop_front();
        }
        inner.lines.push_back(line.clone());
        line
    }

    /// Các dòng có `seq` lớn hơn `after`; `None` là lấy cả bộ đệm.
    pub fn after(&self, after: Option<u64>) -> Vec<ConsoleLine> {
        let inner = self.lock();
        inner
            .lines
            .iter()
            .filter(|l| after.is_none_or(|seq| l.seq > seq))
            .cloned()
            .collect()
    }

    /// `n` dòng cuối, dùng cho thông tin crash.
    pub fn tail(&self, n: usize) -> Vec<String> {
        let inner = self.lock();
        let skip = inner.lines.len().saturating_sub(n);
        inner
            .lines
            .iter()
            .skip(skip)
            .map(|l| l.text.clone())
            .collect()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        // Chỉ bị poison khi một luồng panic lúc đang giữ; dữ liệu bên trong vẫn dùng được.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// `logs/console-YYYY-MM-DD.log` theo ngày UTC, xóa file cũ hơn `keep_days` khi sang ngày mới.
#[derive(Debug)]
struct DailyLog {
    dir: PathBuf,
    keep_days: i64,
    current: Option<(Date, BufWriter<File>)>,
}

impl DailyLog {
    fn new(dir: PathBuf, keep_days: i64) -> Self {
        Self {
            dir,
            keep_days,
            current: None,
        }
    }

    fn write(&mut self, line: &ConsoleLine) {
        let today = OffsetDateTime::now_utc().date();
        if self.current.as_ref().is_none_or(|(day, _)| *day != today) {
            self.current = self.open(today).map(|file| (today, file));
            prune_old_logs(&self.dir, today, self.keep_days);
        }
        let Some((_, file)) = self.current.as_mut() else {
            return;
        };
        let prefix = match line.stream {
            ConsoleStream::Stdin => "> ",
            ConsoleStream::Stdout | ConsoleStream::Stderr => "",
        };
        // Lỗi ghi log không được làm hỏng server; chỉ ghi lại một lần rồi bỏ file này.
        let written = writeln!(file, "{prefix}{}", line.text).and_then(|()| file.flush());
        if let Err(e) = written {
            tracing::warn!(error = %e, dir = %self.dir.display(), "không ghi được log console");
            self.current = None;
        }
    }

    fn open(&self, day: Date) -> Option<BufWriter<File>> {
        let path = self.dir.join(log_name(day));
        let opened = fs::create_dir_all(&self.dir)
            .and_then(|()| OpenOptions::new().create(true).append(true).open(&path));
        match opened {
            Ok(file) => Some(BufWriter::new(file)),
            Err(e) => {
                tracing::warn!(error = %e, path = %path.display(), "không mở được file log console");
                None
            }
        }
    }
}

fn log_name(day: Date) -> String {
    format!(
        "console-{:04}-{:02}-{:02}.log",
        day.year(),
        u8::from(day.month()),
        day.day()
    )
}

/// Xóa `console-*.log` cũ hơn `keep_days` ngày so với `today`. File tên lạ thì để yên.
pub(crate) fn prune_old_logs(dir: &Path, today: Date, keep_days: i64) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(day) = parse_log_name(&name) else {
            continue;
        };
        if (today - day).whole_days() >= keep_days
            && let Err(e) = fs::remove_file(entry.path())
        {
            tracing::warn!(error = %e, name, "không xóa được log console cũ");
        }
    }
}

fn parse_log_name(name: &str) -> Option<Date> {
    let date = name.strip_prefix("console-")?.strip_suffix(".log")?;
    let mut parts = date.splitn(3, '-');
    let year = parts.next()?.parse().ok()?;
    let month: u8 = parts.next()?.parse().ok()?;
    let day = parts.next()?.parse().ok()?;
    Date::from_calendar_date(year, month.try_into().ok()?, day).ok()
}

#[cfg(test)]
mod tests {
    use time::macros::date;

    use super::*;

    #[test]
    fn ring_buffer_keeps_last_lines_and_seq_grows() {
        let console = Console::new(3, None, 14);
        let seqs: Vec<u64> = (0..5)
            .map(|i| console.push(ConsoleStream::Stdout, format!("l{i}")).seq)
            .collect();
        assert!(seqs.windows(2).all(|w| w[1] == w[0] + 1));
        let all = console.after(None);
        assert_eq!(
            all.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(),
            ["l2", "l3", "l4"]
        );
        assert_eq!(console.after(Some(seqs[3])).len(), 1);
        assert_eq!(console.tail(2), ["l3", "l4"]);
    }

    #[test]
    fn writes_daily_log_file() {
        let tmp = tempfile::tempdir().unwrap();
        let console = Console::new(10, Some(tmp.path().to_path_buf()), 14);
        console.push(ConsoleStream::Stdout, "xin chào".to_owned());
        console.push(ConsoleStream::Stdin, "say hi".to_owned());
        let name = log_name(OffsetDateTime::now_utc().date());
        let text = fs::read_to_string(tmp.path().join(name)).unwrap();
        assert_eq!(text, "xin chào\n> say hi\n");
    }

    #[test]
    fn prunes_logs_older_than_keep_days() {
        let tmp = tempfile::tempdir().unwrap();
        for name in [
            "console-2026-10-01.log",
            "console-2026-10-09.log",
            "console-2026-10-10.log",
            "console-2026-10-23.log",
            "notes.txt",
        ] {
            fs::write(tmp.path().join(name), "x").unwrap();
        }
        prune_old_logs(tmp.path(), date!(2026 - 10 - 23), 14);
        let mut left: Vec<String> = fs::read_dir(tmp.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(
            left,
            [
                "console-2026-10-10.log",
                "console-2026-10-23.log",
                "notes.txt"
            ]
        );
    }
}
