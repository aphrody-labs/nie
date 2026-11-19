//! Log files rotated by size, and the UTC timestamp of supervisor lines.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// `<dir>/<name>.log`, renamed to `.log.1` .. `.log.<keep>` when it would exceed `max_bytes`.
pub struct RotatingLog {
    path: PathBuf,
    max_bytes: u64,
    keep: u32,
    inner: Mutex<Open>,
}

struct Open {
    file: File,
    size: u64,
}

impl RotatingLog {
    /// Open (append) the log of `name` under `dir`.
    pub fn open(dir: &Path, name: &str, max_bytes: u64, keep: u32) -> io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(format!("{name}.log"));
        let inner = Mutex::new(Self::reopen(&path)?);
        Ok(Self { path, max_bytes, keep, inner })
    }

    fn reopen(path: &Path) -> io::Result<Open> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let size = file.metadata()?.len();
        Ok(Open { file, size })
    }

    fn rotated(&self, n: u32) -> PathBuf {
        let mut name = self.path.clone().into_os_string();
        name.push(format!(".{n}"));
        PathBuf::from(name)
    }

    fn rotate(&self, open: &mut Open) -> io::Result<()> {
        if self.keep == 0 {
            std::fs::remove_file(&self.path)?;
        } else {
            let _ = std::fs::remove_file(self.rotated(self.keep));
            for n in (1..self.keep).rev() {
                let _ = std::fs::rename(self.rotated(n), self.rotated(n + 1));
            }
            std::fs::rename(&self.path, self.rotated(1))?;
        }
        *open = Self::reopen(&self.path)?;
        Ok(())
    }

    /// Append `bytes`, rotating first when the file would pass `max_bytes`.
    pub fn write(&self, bytes: &[u8]) -> io::Result<()> {
        let mut open = self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if open.size > 0 && open.size + bytes.len() as u64 > self.max_bytes {
            self.rotate(&mut open)?;
        }
        open.file.write_all(bytes)?;
        open.size += bytes.len() as u64;
        Ok(())
    }

    /// One `timestamp [source] message` line.
    pub fn line(&self, source: &str, message: &str) {
        let _ = self.write(format!("{} [{source}] {message}\n", timestamp(now_secs())).as_bytes());
    }
}

/// Seconds since the Unix epoch.
#[must_use]
pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// `2026-11-19T12:11:32Z`.
#[must_use]
pub fn timestamp(secs: u64) -> String {
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // civil-from-days (Howard Hinnant), proleptic Gregorian, days since 1970-01-01
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_are_utc_iso() {
        assert_eq!(timestamp(0), "1970-01-01T00:00:00Z");
        assert_eq!(timestamp(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(timestamp(1_795_090_292), "2026-11-19T12:11:32Z");
    }

    #[test]
    fn rotates_by_size_and_keeps_the_newest() {
        let dir = tempfile::tempdir().unwrap();
        let log = RotatingLog::open(dir.path(), "svc", 10, 2).unwrap();
        for i in 0..5 {
            log.write(format!("line-{i}\n").as_bytes()).unwrap();
        }
        let read = |n: &str| std::fs::read_to_string(dir.path().join(n)).unwrap();
        assert_eq!(read("svc.log"), "line-4\n");
        assert_eq!(read("svc.log.1"), "line-3\n");
        assert_eq!(read("svc.log.2"), "line-2\n");
        assert!(!dir.path().join("svc.log.3").exists());
    }
}
