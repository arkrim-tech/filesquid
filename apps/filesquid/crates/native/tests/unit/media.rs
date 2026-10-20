//! Task DE-0.5: bounded media workers (DE-S-016; NFR-002, NFR-003, ADR 006, ADR 007). Runs the
//! worker logic ([`vips::serve`]) on files on disk with the real libvips, and the pool against
//! stand-in worker commands (`sh`, so Unix only) for the concurrency bound, crash isolation,
//! exit status and the timeout. The desktop crate runs the real worker out of process on every
//! platform (`integration_media_workers`).

use filesquid_api::journal::{JournalQuery, JournalStore, MemoryJournal};
use filesquid_api::monitor::{Monitor, MonitorConfig, StaticHostChecks};
use filesquid_api::EventBus;
use filesquid_native::workers::media::{
    is_worker_invocation, vips, MediaError, MediaWorkers, ThumbnailRequest, MEDIA_PROCESS_KIND,
    WORKER_ARG,
};
use serde_json::{json, Value};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// libvips is initialised once per process; the in-process tests take turns so no two
/// threads of this test binary initialise it at once (the worker runs one request per process).
static IN_PROCESS_VIPS: Mutex<()> = Mutex::new(());

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

/// A pool whose worker is `sh -c script` (a stand-in for the bound, crash and timeout tests).
fn stand_in(monitor: &Monitor, script: &str) -> MediaWorkers {
    MediaWorkers::new(PathBuf::from("sh"), monitor.registry().clone())
        .with_args(vec!["-c".into(), script.into()])
}

/// The worker's answer to `request`, run in this process.
fn serve(request: &Value) -> Value {
    let _turn = IN_PROCESS_VIPS.lock().unwrap_or_else(|p| p.into_inner());
    serde_json::from_str(&vips::serve(&request.to_string())).expect("answer json")
}

/// The worker's answer to a thumbnail of `source` into `dest`.
fn serve_thumbnail(source: &Path, dest: &Path, max_edge: u32) -> Value {
    serve(&json!({ "op": "thumbnail", "source": source, "dest": dest, "max_edge": max_edge }))
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

/// A request for a thumbnail of the still `source` into `dir`.
fn request(dir: &Path, source: &Path) -> ThumbnailRequest {
    ThumbnailRequest {
        source: source.to_path_buf(),
        dest: dir.join("thumb.png"),
        max_edge: 8,
    }
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
fn the_worker_refuses_bad_requests_before_libvips() {
    let dir = dir("refused");
    write_ppm(&dir.join("still.ppm"), 4, 4);
    let still = dir.join("still.ppm");
    let missing = serve_thumbnail(&dir.join("absent.ppm"), &dir.join("thumb.png"), 16);
    assert_eq!(missing["error_code"], "source_unreadable");
    let unknown = serve_thumbnail(&still, &dir.join("thumb.exe"), 16);
    assert_eq!(unknown["error_code"], "invalid_request");
    let oversized = serve_thumbnail(&still, &dir.join("thumb.png"), 5000);
    assert_eq!(oversized["error_code"], "invalid_request");
    let nul =
        serve(&json!({ "op": "thumbnail", "source": "a\u{0}b", "dest": "t.png", "max_edge": 8 }));
    assert_eq!(
        nul["error_code"], "invalid_request",
        "a NUL is a bad request, not a missing file"
    );
    let garbage: Value = serde_json::from_str(&vips::serve("not json")).expect("json");
    assert_eq!(garbage["error_code"], "invalid_request");
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn a_thumbnail_may_never_overwrite_its_source() {
    let dir = dir("alias");
    write_ppm(&dir.join("still.png"), 4, 4);
    let source = dir.join("still.png");
    let same = dir.join(".").join("still.png");
    let answer = serve_thumbnail(&source, &same, 8);
    assert_eq!(answer["error_code"], "invalid_request");
    let (monitor, _) = monitor();
    let never = MediaWorkers::new(PathBuf::from("no-such-program"), monitor.registry().clone());
    let refused = never.thumbnail(&ThumbnailRequest {
        source: source.clone(),
        dest: same,
        max_edge: 8,
    });
    assert_eq!(
        refused.err(),
        Some(MediaError::Refused("invalid_request".into()))
    );
    assert_eq!(
        std::fs::read(&source).expect("source").len(),
        4 * 4 * 3 + 11
    );
    monitor.shutdown();
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn the_pool_refuses_a_bad_request_before_any_process_starts() {
    let dir = dir("host-checks");
    write_ppm(&dir.join("still.ppm"), 4, 4);
    let still = dir.join("still.ppm");
    let (monitor, _) = monitor();
    let never = MediaWorkers::new(PathBuf::from("no-such-program"), monitor.registry().clone());
    let invalid = Some(MediaError::Refused("invalid_request".into()));
    let mut bad = request(&dir, &still);
    bad.dest = dir.join("thumb.exe");
    assert_eq!(never.thumbnail(&bad).err(), invalid);
    let mut huge = request(&dir, &still);
    huge.source = dir.join("x".repeat(70_000));
    assert_eq!(
        never.thumbnail(&huge).err(),
        invalid,
        "past the request bound"
    );
    let missing = request(&dir, &dir.join("absent.ppm"));
    assert_eq!(
        never.thumbnail(&missing).err(),
        Some(MediaError::Refused("source_unreadable".into()))
    );
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let odd = request(
            &dir,
            Path::new(std::ffi::OsStr::from_bytes(b"still-\xff.ppm")),
        );
        assert_eq!(
            never.thumbnail(&odd).err(),
            invalid,
            "a non-UTF-8 path, without a panic"
        );
    }
    let registered = monitor.registry().snapshot();
    assert!(registered.iter().all(|p| p.kind != MEDIA_PROCESS_KIND));
    monitor.shutdown();
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[cfg(unix)]
#[test]
fn at_most_two_workers_run_at_once() {
    use filesquid_core::DEFAULT_CPU_HEAVY_WORKERS;
    use std::time::{Duration, Instant};
    let dir = dir("bound");
    write_ppm(&dir.join("still.ppm"), 4, 4);
    let (monitor, _) = monitor();
    let script = r#"cat >/dev/null; sleep 0.3; echo '{"ok":true,"width":1,"height":1}'"#;
    let pool = Arc::new(stand_in(&monitor, script));
    assert_eq!(pool.max_workers(), DEFAULT_CPU_HEAVY_WORKERS as usize);
    let started = Instant::now();
    let runs: Vec<_> = (0..6)
        .map(|n| {
            let (pool, dir) = (pool.clone(), dir.clone());
            std::thread::spawn(move || {
                pool.thumbnail(&ThumbnailRequest {
                    source: dir.join("still.ppm"),
                    dest: dir.join(format!("out-{n}.png")),
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
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[cfg(unix)]
#[test]
fn a_crashed_worker_fails_its_run_only_and_the_same_pool_goes_on() {
    use filesquid_api::registry::ProcessState;
    let dir = dir("crash");
    write_ppm(&dir.join("secret.ppm"), 4, 4);
    let marker = dir.join("crashed-once");
    let (monitor, journal) = monitor();
    let script = format!(
        r#"if [ -e '{0}' ]; then cat >/dev/null; echo '{{"ok":true,"width":2,"height":3}}'; else touch '{0}'; kill -9 $$; fi"#,
        marker.display()
    );
    let pool = stand_in(&monitor, &script);
    let request = request(&dir, &dir.join("secret.ppm"));
    assert_eq!(pool.thumbnail(&request).err(), Some(MediaError::Crashed));
    let view = monitor
        .registry()
        .snapshot()
        .into_iter()
        .find(|p| p.kind == MEDIA_PROCESS_KIND)
        .expect("registered");
    assert_eq!(view.state, ProcessState::Failed);
    assert_eq!(view.error_code.as_deref(), Some("worker_crashed"));
    let rendered = pool.thumbnail(&request).expect("the same pool goes on");
    assert_eq!((rendered.width, rendered.height), (2, 3));
    monitor.journal().flush();
    let page = journal.query(&JournalQuery::default()).expect("journal");
    let text = serde_json::to_string(&page.entries).expect("json");
    assert!(text.contains("worker_crashed"), "{text}");
    assert!(!text.contains("secret"), "{text}");
    monitor.shutdown();
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[cfg(unix)]
#[test]
fn a_worker_that_exits_unsuccessfully_has_crashed_whatever_it_printed() {
    let dir = dir("exit");
    write_ppm(&dir.join("still.ppm"), 4, 4);
    let (monitor, _) = monitor();
    let script = r#"cat >/dev/null; echo '{"ok":true,"width":1,"height":1}'; exit 3"#;
    let pool = stand_in(&monitor, script);
    let answer = pool.thumbnail(&request(&dir, &dir.join("still.ppm")));
    assert_eq!(answer.err(), Some(MediaError::Crashed));
    monitor.shutdown();
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[cfg(unix)]
#[test]
fn a_worker_past_its_time_limit_is_stopped() {
    use std::time::{Duration, Instant};
    let dir = dir("timeout");
    write_ppm(&dir.join("still.ppm"), 4, 4);
    let (monitor, _) = monitor();
    let slow = stand_in(&monitor, "exec sleep 5").with_timeout(Duration::from_millis(300));
    let started = Instant::now();
    let result = slow.thumbnail(&request(&dir, &dir.join("still.ppm")));
    assert_eq!(result.err(), Some(MediaError::TimedOut));
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "killed, not waited out"
    );
    monitor.shutdown();
    std::fs::remove_dir_all(dir).expect("cleanup");
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
            .map(OsString::from)
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
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let odd = vec![
            OsString::from("filesquid-desktop"),
            OsString::from_vec(b"\xff\xfe".to_vec()),
        ];
        assert!(
            !is_worker_invocation(odd.into_iter()),
            "no panic on a non-UTF-8 argument"
        );
    }
}
