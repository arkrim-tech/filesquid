//! Task DE-0.5: bounded media workers (DE-S-016; NFR-002, NFR-003, ADR 006, ADR 007). Runs the
//! worker logic ([`vips::serve`]) on files on disk with the real libvips, and the pool against
//! stand-in worker commands for the concurrency bound, crash isolation and the timeout. The
//! desktop crate runs the real worker out of process (`integration_media_workers`).

use filesquid_api::journal::{JournalQuery, JournalStore, MemoryJournal};
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::registry::ProcessState;
use filesquid_api::EventBus;
use filesquid_core::DEFAULT_CPU_HEAVY_WORKERS;
use filesquid_native::workers::media::{
    is_worker_invocation, vips, MediaError, MediaWorkers, ThumbnailRequest, MEDIA_PROCESS_KIND,
    WORKER_ARG,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A fresh, empty directory for one test (filesystem I/O).
fn dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fs-native-media-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

/// A monitor over an in-memory journal: the registry the workers register in.
fn monitor() -> (Monitor, Arc<MemoryJournal>) {
    let journal = Arc::new(MemoryJournal::new("desktop"));
    let monitor = Monitor::start(
        journal.clone(),
        Arc::new(StaticHostChecks::healthy()),
        MonitorConfig::new("desktop"),
        EventBus::new(),
    );
    (monitor, journal)
}

/// The worker's answer to `request`, run in this process.
fn serve(request: &Value) -> Value {
    serde_json::from_str(&vips::serve(&request.to_string())).expect("answer json")
}

/// The worker's answer to a thumbnail of `source` into `dest`.
fn serve_thumbnail(source: &Path, dest: &Path, max_edge: u32) -> Value {
    serve(&json!({ "op": "thumbnail", "source": source, "dest": dest, "max_edge": max_edge }))
}

/// A pool whose worker is `sh -c script` (a stand-in for the bound, crash and timeout tests).
fn stand_in(monitor: &Monitor, script: &str) -> MediaWorkers {
    MediaWorkers::new(PathBuf::from("sh"), monitor.registry().clone())
        .with_args(vec!["-c".into(), script.into()])
}

/// Write a `width` x `height` binary PPM (P6) still at `path` (filesystem I/O).
fn write_ppm(path: &Path, width: usize, height: usize) {
    let mut bytes = format!("P6\n{width} {height}\n255\n").into_bytes();
    for n in 0..width * height {
        bytes.extend_from_slice(&[(n % 256) as u8, 128, 64]);
    }
    std::fs::write(path, bytes).expect("ppm");
}

/// Write a 2 x 2, 24-bit BMP at `path`: a format libvips decodes only through its ImageMagick
/// loader (filesystem I/O).
fn write_bmp(path: &Path) {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"BM");
    bytes.extend_from_slice(&70u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&54u32.to_le_bytes());
    bytes.extend_from_slice(&40u32.to_le_bytes());
    bytes.extend_from_slice(&2i32.to_le_bytes());
    bytes.extend_from_slice(&2i32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&24u16.to_le_bytes());
    bytes.extend_from_slice(&[0u8; 24]);
    bytes.extend_from_slice(&[255, 0, 0, 0, 255, 0, 0, 0]);
    bytes.extend_from_slice(&[0, 0, 255, 255, 255, 255, 0, 0]);
    std::fs::write(path, bytes).expect("bmp");
}

/// The width and height a PNG's IHDR chunk states (filesystem I/O).
fn png_size(path: &Path) -> (u32, u32) {
    let bytes = std::fs::read(path).expect("png");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "a PNG");
    let width = u32::from_be_bytes(bytes[16..20].try_into().expect("width"));
    let height = u32::from_be_bytes(bytes[20..24].try_into().expect("height"));
    (width, height)
}

/// Whether libvips loads on this machine; the libvips tests assert the refusal instead when it
/// does not.
fn libvips_present() -> bool {
    let probe = serve(&json!({ "op": "probe" }));
    if probe["ok"] == true {
        return true;
    }
    assert_eq!(probe["error_code"], "libvips_unavailable", "{probe}");
    false
}

#[test]
fn the_worker_writes_a_thumbnail_within_the_requested_bound() {
    if !libvips_present() {
        return;
    }
    let dir = dir("thumb");
    write_ppm(&dir.join("still.ppm"), 64, 48);
    let answer = serve_thumbnail(&dir.join("still.ppm"), &dir.join("thumb.png"), 16);
    assert_eq!(answer, json!({ "ok": true, "width": 16, "height": 12 }));
    assert_eq!(png_size(&dir.join("thumb.png")), (16, 12));
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn the_worker_blocks_the_imagemagick_loaders_and_refuses_a_magick_only_format() {
    if !libvips_present() {
        return;
    }
    let probe = serve(&json!({ "op": "probe" }));
    assert_eq!(probe["magick_blocked"], true, "{probe}");
    assert!(
        probe["libvips"]
            .as_str()
            .unwrap_or_default()
            .starts_with('8'),
        "{probe}"
    );
    let dir = dir("magick");
    write_bmp(&dir.join("still.bmp"));
    let answer = serve_thumbnail(&dir.join("still.bmp"), &dir.join("thumb.png"), 16);
    assert_eq!(
        answer,
        json!({ "ok": false, "error_code": "unsupported_format" })
    );
    assert!(
        !dir.join("thumb.png").exists(),
        "nothing decoded through ImageMagick"
    );
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn a_missing_source_an_unknown_output_format_or_a_bad_request_is_refused() {
    let dir = dir("refused");
    let missing = serve_thumbnail(&dir.join("absent.ppm"), &dir.join("thumb.png"), 16);
    assert_eq!(missing["error_code"], "source_unreadable");
    write_ppm(&dir.join("still.ppm"), 4, 4);
    let unknown = serve_thumbnail(&dir.join("still.ppm"), &dir.join("thumb.exe"), 16);
    assert_eq!(unknown["error_code"], "invalid_request");
    let oversized = serve_thumbnail(&dir.join("still.ppm"), &dir.join("thumb.png"), 5000);
    assert_eq!(oversized["error_code"], "invalid_request");
    let garbage: Value = serde_json::from_str(&vips::serve("not json")).expect("json");
    assert_eq!(garbage["error_code"], "invalid_request");
    let (monitor, _) = monitor();
    let never = stand_in(&monitor, "exit 1");
    let refused = never.thumbnail(&ThumbnailRequest {
        source: dir.join("still.ppm"),
        dest: dir.join("thumb.exe"),
        max_edge: 16,
    });
    assert_eq!(
        refused.err(),
        Some(MediaError::Refused("invalid_request".into())),
        "refused before any process starts"
    );
    let registered = monitor.registry().snapshot();
    assert!(registered.iter().all(|p| p.kind != MEDIA_PROCESS_KIND));
    monitor.shutdown();
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn at_most_two_workers_run_at_once() {
    let (monitor, _) = monitor();
    let script = r#"cat >/dev/null; sleep 0.3; echo '{"ok":true,"width":1,"height":1}'"#;
    let pool = Arc::new(stand_in(&monitor, script));
    assert_eq!(pool.max_workers(), DEFAULT_CPU_HEAVY_WORKERS as usize);
    let started = Instant::now();
    let runs: Vec<_> = (0..6)
        .map(|n| {
            let pool = pool.clone();
            std::thread::spawn(move || {
                pool.thumbnail(&ThumbnailRequest {
                    source: PathBuf::from(format!("in-{n}.ppm")),
                    dest: PathBuf::from(format!("out-{n}.png")),
                    max_edge: 8,
                })
            })
        })
        .collect();
    for run in runs {
        run.join().expect("thread").expect("rendered");
    }
    assert_eq!(pool.peak_running(), 2, "two at a time, never more");
    assert!(
        started.elapsed() >= Duration::from_millis(850),
        "three waves of two"
    );
    monitor.shutdown();
}

#[test]
fn a_crashed_worker_fails_its_run_only_and_is_journaled_without_paths() {
    let (monitor, journal) = monitor();
    let crashing = stand_in(&monitor, "kill -9 $$");
    let request = ThumbnailRequest {
        source: PathBuf::from("/Users/someone/Pictures/secret.ppm"),
        dest: PathBuf::from("/tmp/out.png"),
        max_edge: 8,
    };
    assert_eq!(
        crashing.thumbnail(&request).err(),
        Some(MediaError::Crashed)
    );
    let view = monitor
        .registry()
        .snapshot()
        .into_iter()
        .find(|p| p.kind == MEDIA_PROCESS_KIND)
        .expect("registered");
    assert_eq!(view.state, ProcessState::Failed);
    assert_eq!(view.error_code.as_deref(), Some("worker_crashed"));
    let healthy = stand_in(
        &monitor,
        r#"cat >/dev/null; echo '{"ok":true,"width":2,"height":3}'"#,
    );
    let rendered = healthy.thumbnail(&request).expect("the host goes on");
    assert_eq!((rendered.width, rendered.height), (2, 3));
    monitor.journal().flush();
    let page = journal.query(&JournalQuery::default()).expect("journal");
    let text = serde_json::to_string(&page.entries).expect("json");
    assert!(text.contains("worker_crashed"), "{text}");
    assert!(
        !text.contains("someone") && !text.contains("secret"),
        "{text}"
    );
    monitor.shutdown();
}

#[test]
fn a_worker_past_its_time_limit_is_stopped() {
    let (monitor, _) = monitor();
    let slow = stand_in(&monitor, "sleep 5").with_timeout(Duration::from_millis(300));
    let started = Instant::now();
    let result = slow.thumbnail(&ThumbnailRequest {
        source: PathBuf::from("in.ppm"),
        dest: PathBuf::from("out.png"),
        max_edge: 8,
    });
    assert_eq!(result.err(), Some(MediaError::TimedOut));
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "killed, not waited out"
    );
    monitor.shutdown();
}

#[test]
fn no_imagemagick_crate_is_in_the_dependency_graph() {
    let lock = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock");
    let text = std::fs::read_to_string(lock).expect("lockfile");
    let names: Vec<&str> = text
        .lines()
        .filter_map(|line| line.strip_prefix("name = \""))
        .filter(|name| name.to_ascii_lowercase().contains("magick"))
        .collect();
    assert!(names.is_empty(), "{names:?}");
}

#[test]
fn only_the_worker_argument_starts_a_worker_run() {
    let args = |list: &[&str]| {
        list.iter()
            .map(|a| a.to_string())
            .collect::<Vec<_>>()
            .into_iter()
    };
    assert!(is_worker_invocation(args(&[
        "filesquid-desktop",
        WORKER_ARG
    ])));
    assert!(!is_worker_invocation(args(&["filesquid-desktop"])));
    assert!(!is_worker_invocation(args(&[
        "filesquid-desktop",
        "--other",
        WORKER_ARG
    ])));
    assert!(!is_worker_invocation(args(&[WORKER_ARG])));
}
