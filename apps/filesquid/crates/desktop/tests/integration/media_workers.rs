//! Task DE-0.5: the desktop host's media workers (DE-S-016; ADR 006, ADR 007). The real
//! worker runs out of process: this crate's own executable started with the worker argument,
//! one process per run, at most two at once.

use filesquid_api::ApiState;
use filesquid_core::DEFAULT_CPU_HEAVY_WORKERS;
use filesquid_desktop_lib::{media_workers, open_host_monitor};
use filesquid_native::workers::media::{MediaError, MediaWorkers, ThumbnailRequest, WORKER_ARG};
use filesquid_native::HostMonitor;
use std::path::{Path, PathBuf};

/// The desktop executable Cargo built for these tests.
const DESKTOP: &str = env!("CARGO_BIN_EXE_filesquid-desktop");

/// A fresh, empty directory for one test (filesystem I/O).
fn dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fs-desktop-media-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

/// A host monitor in `dir`, and the pool over the built desktop executable.
fn pool(dir: &Path) -> (HostMonitor, MediaWorkers) {
    let host = open_host_monitor(&ApiState::new(), &dir.join("data"), None).expect("monitor");
    let pool = MediaWorkers::new(PathBuf::from(DESKTOP), host.monitor().registry().clone())
        .with_args(vec![WORKER_ARG.to_string()]);
    (host, pool)
}

/// Write a `width` x `height` binary PPM still at `path` (filesystem I/O).
fn write_ppm(path: &Path, width: usize, height: usize) {
    let mut bytes = format!("P6\n{width} {height}\n255\n").into_bytes();
    bytes.resize(bytes.len() + width * height * 3, 77);
    std::fs::write(path, bytes).expect("ppm");
}

#[test]
fn real_workers_from_the_desktop_executable_run_at_most_two_at_once() {
    let dir = dir("pool");
    let (host, pool) = pool(&dir);
    assert_eq!(
        media_workers(&host).max_workers(),
        DEFAULT_CPU_HEAVY_WORKERS as usize
    );
    assert_eq!(pool.max_workers(), 2);
    write_ppm(&dir.join("still.ppm"), 32, 32);
    let pool = std::sync::Arc::new(pool);
    let runs: Vec<_> = (0..4)
        .map(|n| {
            let (pool, dir) = (pool.clone(), dir.clone());
            std::thread::spawn(move || {
                pool.thumbnail(&ThumbnailRequest {
                    source: dir.join("still.ppm"),
                    dest: dir.join(format!("thumb-{n}.png")),
                    max_edge: 8,
                })
            })
        })
        .collect();
    for run in runs {
        match run.join().expect("thread") {
            Ok(rendered) => assert_eq!((rendered.width, rendered.height), (8, 8)),
            Err(MediaError::Refused(code)) => assert_eq!(code, "libvips_unavailable"),
            Err(other) => panic!("{other:?}"),
        }
    }
    assert!(
        (1..=2).contains(&pool.peak_running()),
        "{}",
        pool.peak_running()
    );
    host.stop();
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn the_desktop_executable_makes_a_thumbnail_out_of_process() {
    let dir = dir("thumb");
    let (host, pool) = pool(&dir);
    write_ppm(&dir.join("still.ppm"), 64, 48);
    let made = pool.thumbnail(&ThumbnailRequest {
        source: dir.join("still.ppm"),
        dest: dir.join("thumb.png"),
        max_edge: 16,
    });
    match made {
        Ok(rendered) => {
            assert_eq!((rendered.width, rendered.height), (16, 12));
            assert!(dir.join("thumb.png").exists());
            let probe = pool.probe().expect("probe");
            assert!(probe.magick_blocked, "{probe:?}");
        }
        Err(MediaError::Refused(code)) => assert_eq!(code, "libvips_unavailable"),
        Err(other) => panic!("{other:?}"),
    }
    host.stop();
    std::fs::remove_dir_all(dir).expect("cleanup");
}
