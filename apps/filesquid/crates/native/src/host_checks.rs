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

/// Prove `dir` is writable by exclusively creating a uniquely named probe file and removing
/// it (filesystem I/O). `create_new` means an existing file is never truncated or removed.
fn probe_write(dir: &Path) -> bool {
    let probe = dir.join(format!(
        ".filesquid-write-probe-{}-{}",
        std::process::id(),
        PROBE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let Ok(mut file) = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    else {
        return false;
    };
    let ok = file.write_all(b"ok").is_ok();
    drop(file);
    if let Err(err) = std::fs::remove_file(&probe) {
        tracing::warn!(%err, error_code = "probe_cleanup_failed", "write probe not removed");
    }
    ok
}
