//! Bounded media workers (Task DE-0.5; DE-S-016; NFR-002, NFR-003, ADR 006, ADR 007).
//!
//! Stills are decoded and resized by libvips in **out-of-process** workers: each run is a
//! separate OS process (the desktop executable started with [`WORKER_ARG`], which runs
//! [`worker_main`] and exits before any window or catalog opens), so a decoder crash fails
//! that run only and never takes down the catalog or the UI. [`MediaWorkers`] is the
//! host side:
//!
//! - at most [`DEFAULT_CPU_HEAVY_WORKERS`] (2) workers run at once, and at most
//!   [`MAX_QUEUED`] callers wait for a slot, each no longer than the run time limit; the rest
//!   are refused `busy`, so the queue is finite;
//! - each run has a time limit ([`DEFAULT_WORKER_TIMEOUT`]); a worker past it is killed;
//! - each run is a process in the registry ([`MEDIA_PROCESS_KIND`]), completed or failed with
//!   an error code, which the registry journals; no path reaches the journal or the answer.
//!
//! The worker side is [`vips`]: it loads libvips at run time, blocks every ImageMagick loader
//! and saver libvips may have been built with (ADR 006: ImageMagick is never a fallback), and
//! answers one JSON request on stdin with one JSON line on stdout. Interactive media (zoom,
//! scrubbing, slideshow timing) stays in the client; these workers make derivatives only.

pub mod vips;

use filesquid_api::registry::{ProcessRegistry, ProcessSpec};
use filesquid_core::DEFAULT_CPU_HEAVY_WORKERS;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// Process kind of one worker run in the registry.
pub const MEDIA_PROCESS_KIND: &str = "media.thumbnail";

/// Longest one worker run may take before it is killed.
pub const DEFAULT_WORKER_TIMEOUT: Duration = Duration::from_secs(60);

/// Most callers that may wait for a worker slot at once.
pub const MAX_QUEUED: usize = 64;

/// Largest thumbnail edge a request may ask for, in pixels.
pub const MAX_EDGE: u32 = 4096;

/// Largest answer read from a worker, in bytes.
const MAX_ANSWER_BYTES: u64 = 4096;

/// How often a running worker is checked for its exit.
const POLL: Duration = Duration::from_millis(10);

/// The argument that makes the desktop executable run as one media worker process
/// ([`worker_main`]) instead of the app: the worker ships inside the host binary, so the
/// installer carries no second program and the workspace builds one CLI only (F-050).
pub const WORKER_ARG: &str = "--filesquid-media-worker";

/// Whether the process arguments `args` (program name first) ask for a media worker run.
/// Pure.
pub fn is_worker_invocation(mut args: impl Iterator<Item = String>) -> bool {
    args.nth(1).as_deref() == Some(WORKER_ARG)
}

/// One media worker run: read the request from stdin (at most
/// [`vips::MAX_REQUEST_BYTES`]), answer it with [`vips::serve`], print the answer line on
/// stdout, and answer the process exit code (0). Writes nothing else: no log file, no
/// stderr protocol. Reads stdin, loads libvips, reads and writes the request's files.
pub fn worker_main() -> i32 {
    let mut request = String::new();
    let read = std::io::stdin()
        .take(vips::MAX_REQUEST_BYTES)
        .read_to_string(&mut request);
    let answer = match read {
        Ok(_) => vips::serve(&request),
        Err(_) => json!({ "ok": false, "error_code": "invalid_request" }).to_string(),
    };
    println!("{answer}");
    0
}

/// A thumbnail to make: the still at `source`, scaled to fit `max_edge` pixels on its longer
/// edge, written to `dest` (its extension names the format: `png`, `jpg`, `jpeg` or `webp`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThumbnailRequest {
    /// The still to read.
    pub source: PathBuf,
    /// Where to write the thumbnail.
    pub dest: PathBuf,
    /// The longest edge of the thumbnail, 1 to [`MAX_EDGE`].
    pub max_edge: u32,
}

/// A thumbnail the worker wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rendered {
    /// Its width in pixels.
    pub width: u32,
    /// Its height in pixels.
    pub height: u32,
}

/// What a worker reports about its libvips.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Probe {
    /// The libvips version string.
    pub libvips_version: String,
    /// Whether every ImageMagick loader and saver is blocked.
    pub magick_blocked: bool,
}

/// Why a worker run did not produce its result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaError {
    /// Every worker slot is taken and the queue is full, or the wait ran out.
    Busy,
    /// The worker program could not be started.
    Unavailable,
    /// The worker process ended without an answer (a crash or a kill).
    Crashed,
    /// The worker ran past its time limit and was killed.
    TimedOut,
    /// The worker answered something that is not its protocol.
    Protocol,
    /// The worker refused the request with this code (`invalid_request`,
    /// `source_unreadable`, `unsupported_format`, `write_failed`, `libvips_unavailable`).
    Refused(String),
}

impl MediaError {
    /// The journal and API error code.
    pub fn error_code(&self) -> &str {
        match self {
            Self::Busy => "busy",
            Self::Unavailable => "worker_unavailable",
            Self::Crashed => "worker_crashed",
            Self::TimedOut => "worker_timeout",
            Self::Protocol => "worker_protocol",
            Self::Refused(code) => code,
        }
    }
}

impl std::fmt::Display for MediaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "media worker: {}", self.error_code())
    }
}

impl std::error::Error for MediaError {}

/// Running and waiting runs.
#[derive(Debug, Default)]
struct Slots {
    running: usize,
    waiting: usize,
}

/// The host's pool of out-of-process media workers.
pub struct MediaWorkers {
    program: PathBuf,
    args: Vec<String>,
    max: usize,
    timeout: Duration,
    registry: ProcessRegistry,
    slots: Mutex<Slots>,
    freed: Condvar,
    peak: AtomicUsize,
}

impl std::fmt::Debug for MediaWorkers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaWorkers")
            .field("max", &self.max)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl MediaWorkers {
    /// A pool that runs `program` (the desktop executable, with [`WORKER_ARG`] given through
    /// [`with_args`](Self::with_args)) at most
    /// [`DEFAULT_CPU_HEAVY_WORKERS`] at a time and registers each run in `registry`.
    pub fn new(program: PathBuf, registry: ProcessRegistry) -> Self {
        Self {
            program,
            args: Vec::new(),
            max: DEFAULT_CPU_HEAVY_WORKERS as usize,
            timeout: DEFAULT_WORKER_TIMEOUT,
            registry,
            slots: Mutex::new(Slots::default()),
            freed: Condvar::new(),
            peak: AtomicUsize::new(0),
        }
    }

    /// Run the program with `args` before the request is written to it.
    #[must_use]
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Kill a run that takes longer than `timeout` (at least one millisecond).
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout.max(Duration::from_millis(1));
        self
    }

    /// The most workers that run at once.
    pub fn max_workers(&self) -> usize {
        self.max
    }

    /// The most workers that ever ran at once.
    pub fn peak_running(&self) -> usize {
        self.peak.load(Ordering::SeqCst)
    }

    /// Make the thumbnail `request` asks for in a worker process (process I/O).
    ///
    /// # Errors
    /// [`MediaError`]; a request past [`MAX_EDGE`] or without a supported output format is
    /// refused `invalid_request` before any process starts.
    pub fn thumbnail(&self, request: &ThumbnailRequest) -> Result<Rendered, MediaError> {
        if request.max_edge == 0 || request.max_edge > MAX_EDGE || !vips::writable(&request.dest) {
            return Err(MediaError::Refused("invalid_request".into()));
        }
        let answer = self.run(&json!({
            "op": "thumbnail",
            "source": request.source,
            "dest": request.dest,
            "max_edge": request.max_edge,
        }))?;
        let size = |key: &str| answer[key].as_u64().and_then(|n| u32::try_from(n).ok());
        match (size("width"), size("height")) {
            (Some(width), Some(height)) => Ok(Rendered { width, height }),
            _ => Err(MediaError::Protocol),
        }
    }

    /// Ask a worker which libvips it loads and whether ImageMagick is blocked (process I/O).
    ///
    /// # Errors
    /// [`MediaError`], `libvips_unavailable` when no libvips can be loaded.
    pub fn probe(&self) -> Result<Probe, MediaError> {
        let answer = self.run(&json!({ "op": "probe" }))?;
        let version = answer["libvips"].as_str().ok_or(MediaError::Protocol)?;
        Ok(Probe {
            libvips_version: version.to_string(),
            magick_blocked: answer["magick_blocked"].as_bool() == Some(true),
        })
    }

    /// Run one worker for `request` in a slot, registered as a process (process I/O).
    fn run(&self, request: &Value) -> Result<Value, MediaError> {
        let _slot = self.acquire()?;
        let handle = self.registry.register(ProcessSpec::new(
            MEDIA_PROCESS_KIND,
            i64::try_from(self.timeout.as_millis()).unwrap_or(i64::MAX),
        ));
        let result = self.spawn_and_wait(request);
        match &result {
            Ok(_) => handle.complete(),
            Err(err) => handle.fail(err.error_code()),
        }
        result
    }

    /// Take a worker slot, waiting (at most the run time limit) while all are taken.
    fn acquire(&self) -> Result<SlotGuard<'_>, MediaError> {
        let mut slots = self.lock();
        if slots.running >= self.max && slots.waiting >= MAX_QUEUED {
            return Err(MediaError::Busy);
        }
        let deadline = Instant::now() + self.timeout;
        slots.waiting += 1;
        while slots.running >= self.max {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                slots.waiting -= 1;
                return Err(MediaError::Busy);
            }
            slots = self
                .freed
                .wait_timeout(slots, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        slots.waiting -= 1;
        slots.running += 1;
        self.peak.fetch_max(slots.running, Ordering::SeqCst);
        Ok(SlotGuard { pool: self })
    }

    /// The slots, locked.
    fn lock(&self) -> MutexGuard<'_, Slots> {
        self.slots.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Start the worker, hand it `request`, and wait for its answer within the time limit
    /// (process I/O).
    fn spawn_and_wait(&self, request: &Value) -> Result<Value, MediaError> {
        let mut child = Command::new(&self.program)
            .args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| MediaError::Unavailable)?;
        if let Some(mut stdin) = child.stdin.take() {
            // A worker that exits without reading its request answers below; a broken pipe
            // here is not an error of its own.
            let _ = stdin.write_all(request.to_string().as_bytes());
        }
        self.wait(&mut child)?;
        read_answer(&mut child)
    }

    /// Wait for `child` to exit, killing it past the time limit (process I/O).
    fn wait(&self, child: &mut Child) -> Result<(), MediaError> {
        let deadline = Instant::now() + self.timeout;
        while Instant::now() < deadline {
            match child.try_wait() {
                Ok(Some(_)) => return Ok(()),
                Ok(None) => std::thread::sleep(POLL),
                Err(_) => break,
            }
        }
        if let Err(err) = child.kill() {
            tracing::warn!(error = %err, "a media worker could not be killed");
        }
        // Reap the killed worker; its status is not needed.
        let _ = child.wait();
        Err(MediaError::TimedOut)
    }
}

/// A taken worker slot, given back when dropped.
struct SlotGuard<'a> {
    pool: &'a MediaWorkers,
}

impl Drop for SlotGuard<'_> {
    fn drop(&mut self) {
        self.pool.lock().running -= 1;
        self.pool.freed.notify_one();
    }
}

/// The answer of an exited worker: its one JSON line (process I/O). No answer is a crash.
fn read_answer(child: &mut Child) -> Result<Value, MediaError> {
    let mut text = String::new();
    let read = match child.stdout.take() {
        Some(stdout) => stdout.take(MAX_ANSWER_BYTES).read_to_string(&mut text),
        None => return Err(MediaError::Crashed),
    };
    if read.is_err() || text.trim().is_empty() {
        return Err(MediaError::Crashed);
    }
    let answer: Value = serde_json::from_str(text.trim()).map_err(|_| MediaError::Protocol)?;
    match answer["ok"].as_bool() {
        Some(true) => Ok(answer),
        Some(false) => Err(MediaError::Refused(refusal_code(&answer))),
        None => Err(MediaError::Protocol),
    }
}

/// The refusal code of a worker answer, kept to lowercase letters and underscores so a
/// worker can never put anything else into the journal. Pure.
fn refusal_code(answer: &Value) -> String {
    let code = answer["error_code"].as_str().unwrap_or_default();
    let clean = !code.is_empty() && code.len() <= 32;
    match clean && code.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
        true => code.to_string(),
        false => "worker_error".to_string(),
    }
}
