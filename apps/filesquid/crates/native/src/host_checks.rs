//! Data-directory host checks (SRS-F-061 Health: "storage roots reachable and writable, free
//! space against thresholds"), the native [`HostChecks`] a host injects into its monitor.

use filesquid_api::monitor::HostChecks;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Checks the directory that holds the host's operational store.
#[derive(Clone, Debug)]
pub struct DataDirChecks {
    data_dir: PathBuf,
}

impl DataDirChecks {
    /// Checks for `data_dir`. Performs no I/O until a check runs.
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }

    /// The checked directory.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }
}

impl HostChecks for DataDirChecks {
    /// Prove the directory is writable with a probe file (filesystem I/O).
    fn data_dir_writable(&self) -> bool {
        probe_write(&self.data_dir)
    }

    /// Free bytes on the directory's volume, when measurable (filesystem I/O).
    fn free_bytes(&self) -> Option<u64> {
        fs4::available_space(&self.data_dir).ok()
    }
}

/// Distinguishes concurrent probes in one process.
static PROBE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Most probe names tried before the directory is reported as not writable (AGENTS.md §22).
pub const MAX_PROBE_ATTEMPTS: usize = 32;

/// The probe file name for sequence number `sequence` of this process.
pub fn probe_name(sequence: u64) -> String {
    format!(".filesquid-write-probe-{}-{sequence}", std::process::id())
}

/// Prove `dir` is writable by exclusively creating a uniquely named probe file and removing
/// it (filesystem I/O). `create_new` means an existing file is never truncated or removed:
/// when a name is taken (for example a probe left by a crashed process whose process id was
/// reused), the next sequence number is tried, at most [`MAX_PROBE_ATTEMPTS`] times.
fn probe_write(dir: &Path) -> bool {
    let Some((probe, mut file)) = create_probe_file(dir) else {
        return false;
    };
    let ok = file.write_all(b"ok").is_ok();
    drop(file);
    if let Err(err) = std::fs::remove_file(&probe) {
        tracing::warn!(%err, error_code = "probe_cleanup_failed", "write probe not removed");
    }
    ok
}

/// Exclusively create a fresh probe file in `dir` (filesystem I/O), skipping names that
/// already exist. Returns its path and handle, or `None` on any other error or when every
/// attempt found its name taken.
fn create_probe_file(dir: &Path) -> Option<(PathBuf, std::fs::File)> {
    for _ in 0..MAX_PROBE_ATTEMPTS {
        let probe = dir.join(probe_name(PROBE_SEQUENCE.fetch_add(1, Ordering::Relaxed)));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe)
        {
            Ok(file) => return Some((probe, file)),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return None,
        }
    }
    None
}
