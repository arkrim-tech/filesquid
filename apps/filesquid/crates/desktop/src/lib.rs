//! `filesquid-desktop`: the FileSquid desktop host (Tauri 2).
//!
//! Serves the single FileSquid API to the webview through the `filesquid://` custom URI
//! scheme ([`api_protocol`]), ranged reads included, and pushes API events to the app's own
//! webviews as Tauri events named [`EVENT_NAME`] ([`event_channel`]). There is no TCP listener
//! and no per-feature IPC command
//! ([ADR 030](../../../../../resources/architecture/030-filesquid-api-and-transports.md)).
//! Activity is recorded in the activity journal in the app data directory and the host's
//! health is tracked by the process registry
//! ([ADR 033](../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md)).
//! Logs go to stdout as JSON; no log files.
//!
//! The host describes itself to the API as `desktop` at every launch
//! ([`describe_desktop_host`]), so `GET /api/v1/host` tells the UI where it runs.
//!
//! Every launch generates a new per-launch token ([`install_launch_token`]). The API refuses
//! any request without it (deny-by-default, API-S-004), and the host hands it only to its
//! own pages through a webview initialization script ([`api_protocol::launch_token_script`]).
//! The same plugin keeps every webview on the app's own origins, so the token and the events
//! reach app pages only.
//!
//! The webview's Tauri capabilities are deny-by-default ([`capability_policy`]): the main
//! window may only listen for the API's events. The host defines no `#[tauri::command]`, and
//! the app ACL manifest (`permissions/app-acl.toml`) makes Tauri refuse any custom command no
//! capability grants. `build.rs` fails the build when a capability file widens that set.
//!
//! Every refusal in the shell is journaled (Task DE-0.9): the host owns Tauri's IPC scheme and
//! journals every IPC call it or Tauri refuses ([`ipc_gate`]); a `tracing` layer journals the
//! refusals Tauri reports only through its own trace ([`ipc_observer`]); the host builds the
//! main window itself and refuses and journals new windows, permission requests and foreign
//! downloads ([`app_window`]); navigation off the app's origins is refused and journaled by the
//! app-pages plugin; and a panic on any host thread is journaled ([`panic_journal`]).
//!
//! The test-instrumented build (`--features test-webdriver`, never shipped) adds the embedded
//! WebDriver server for the packaged-window tests and a 100 MB range fixture (`test_fixture`).

pub mod api_protocol;
pub mod app_window;
pub mod backfill;
pub mod capability_policy;
pub mod event_channel;
pub mod folder_choice;
pub mod imports;
pub mod ipc_gate;
pub mod ipc_observer;
pub mod libraries;
pub mod navigation;
pub mod panic_journal;
pub mod rebuilds;
pub mod recently_deleted;
pub mod stdout_log;
#[cfg(feature = "test-webdriver")]
pub mod test_fixture;

pub use event_channel::EVENT_NAME;
pub use stdout_log::LOGGING_OPERATION;

use backfill::{BackfillTask, DesktopBackfill, BACKFILL_STOP_WAIT};
use event_channel::{Forwarding, TauriSink, FORWARDER_HEARTBEAT};
use filesquid_api::auth::{record_token_issue, Authoriser, LaunchToken, Principal};
use filesquid_api::history::backfill::Backfill;
use filesquid_api::history::{HostFacts, OcflRootsHistory, ProvenanceContext};
use filesquid_api::host::{HostIdentity, NotEnrolled};
use filesquid_api::jobs::Jobs;
use filesquid_api::journal::{JournalEntry, Outcome};
use filesquid_api::journal_writer::JournalSender;
use filesquid_api::local_library::{
    LocalLibraryService, RebuildRunner, RepairLedger, REBUILD_JOB_KIND,
};
use filesquid_api::ocfl_validation::ValidationService;
use filesquid_api::registry::ProcessHandle;
use filesquid_api::trace_layer::JournalLayer;
use filesquid_api::ApiState;
use filesquid_core::{InstallationId, RepositoryId};
use filesquid_native::import::items::open_import_stores;
use filesquid_native::import::plans::SqliteImportPlans;
use filesquid_native::installation::{
    self, InstallationError, InstallationSource, INSTALLATION_CREATED,
};
use filesquid_native::workers::history_backfill;
use filesquid_native::workers::media::{self, MediaWorkers};
use filesquid_native::{
    CatalogRoots, HostLocations, HostMonitor, HostRoots, OpenedRoots, SqliteCatalog,
    SqlitePreferences, SqliteRepairLedger, CATALOG_DIR, JOURNAL_FILE,
};
use filesquid_ocfl::storage::FsBackend;
use ipc_observer::TauriIpcObserver;
use libraries::{DesktopLibraries, LibraryServing};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tauri::{Manager, RunEvent};
use tracing::Subscriber;
use tracing_subscriber::filter::{filter_fn, EnvFilter, FilterExt};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

/// Interval between monitor ticks (host heartbeat, stalled checks, health).
const TICK: Duration = Duration::from_secs(1);

/// Desktop monitor, the event forwarder's process, the history backfill's thread, and the
/// flag that ends all three.
struct DesktopMonitor {
    host: Arc<HostMonitor>,
    forwarder: Arc<ProcessHandle>,
    stop: Arc<AtomicBool>,
    backfill: Arc<DesktopBackfill>,
    imports: Arc<filesquid_native::import::worker::ImportWorker>,
}

/// The journal sinks the host attaches once the journal opens: the journal layer, the Tauri
/// IPC observer and the journal the panic hook flushes.
#[derive(Clone)]
struct HostJournal {
    layer: JournalLayer,
    observer: TauriIpcObserver,
    panic_flush: Arc<OnceLock<JournalSender>>,
}

impl HostJournal {
    /// Sinks with no journal yet.
    fn new() -> Self {
        Self {
            layer: JournalLayer::new(),
            observer: TauriIpcObserver::new(),
            panic_flush: Arc::new(OnceLock::new()),
        }
    }

    /// Attach the observer and the panic hook to the opened journal (`HostMonitor::open`
    /// attaches the layer itself).
    fn attach(&self, sender: JournalSender) {
        self.observer.attach(sender.clone());
        let _ = self.panic_flush.set(sender);
    }
}

/// The host's `tracing` stack: JSON lines to `stdout` filtered by `filter`, each with an RFC
/// 3339 UTC millisecond `timestamp`, `level`, `target` and fields ([`stdout_log`]), the journal
/// layer, and the Tauri IPC observer ([`ipc_observer`]). The stdout and journal layers
/// skip Tauri's IPC trace and webview errors, which only the observer handles, so neither a
/// request body nor the invoke key Tauri records there reaches either. Pure: builds the
/// subscriber without installing it.
pub fn host_subscriber<W>(
    journal: JournalLayer,
    observer: TauriIpcObserver,
    stdout: W,
    filter: EnvFilter,
) -> impl Subscriber + Send + Sync + 'static
where
    W: for<'w> MakeWriter<'w> + Clone + Send + Sync + 'static,
{
    let observer = observer.with_stdout(line_sink(stdout.clone()));
    let kept = filter_fn(ipc_observer::outside_observed);
    let stdout = tracing_subscriber::fmt::layer()
        .json()
        .with_timer(stdout_log::UtcMillis)
        .with_writer(stdout)
        .with_filter(filter.and(kept.clone()));
    tracing_subscriber::registry()
        .with(stdout)
        .with(journal.with_filter(kept))
        .with(observer.with_filter(filter_fn(ipc_observer::observed)))
}

/// A sink that writes one line through `stdout` and flushes it. Performs I/O when called; a write error is
/// reported on stderr, since the stdout stream itself failed.
fn line_sink<W>(stdout: W) -> ipc_observer::StdoutSink
where
    W: for<'w> MakeWriter<'w> + Send + Sync + 'static,
{
    Arc::new(move |line: &str| {
        let mut writer = stdout.make_writer();
        if let Err(err) = writeln!(writer, "{line}").and_then(|()| writer.flush()) {
            eprintln!("stdout line not written: {err}");
        }
    })
}

/// Install the host's `tracing` stack ([`host_subscriber`]) as the process's global
/// subscriber, which also routes `log` records (Tauri's own) into it, so they reach the same
/// stdout JSON stream. Installs global state.
///
/// # Errors
/// A global subscriber or `log` logger is already installed; nothing is changed.
pub fn install_tracing<W>(
    journal: JournalLayer,
    observer: TauriIpcObserver,
    stdout: W,
    filter: EnvFilter,
) -> Result<(), String>
where
    W: for<'w> MakeWriter<'w> + Clone + Send + Sync + 'static,
{
    host_subscriber(journal, observer, stdout, filter)
        .try_init()
        .map_err(|err| format!("tracing not installed: {err}"))
}

/// Install the host's `tracing` stack with stdout filtered by `RUST_LOG` (default `info`),
/// writing through [`stdout_log::stdout`] (flushed per record). A failure is reported, never
/// swallowed: [`stdout_log::report_logging_failure`] now, and a journal entry once the
/// journal opens ([`start_monitor`]). Without the stack, `tracing` events (IPC refusals read
/// from Tauri's trace, warnings and errors) are lost for this launch; the host keeps
/// running. Answers whether the stack was installed.
fn init_tracing(journal: &HostJournal) -> bool {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let installed = install_tracing(
        journal.layer.clone(),
        journal.observer.clone(),
        stdout_log::stdout(),
        filter,
    );
    if let Err(err) = &installed {
        stdout_log::report_logging_failure(&mut std::io::stdout(), err);
    }
    installed.is_ok()
}

/// Name of the plugin that gives app pages the per-launch token and keeps every webview on
/// the app's own origins.
const APP_PAGES_PLUGIN: &str = "filesquid-app-pages";

/// Generate this launch's token and install it as the only credential `state` accepts.
/// Fails when the random source fails or an authoriser is already installed.
pub fn install_launch_token(state: &ApiState) -> Result<LaunchToken, String> {
    let token = LaunchToken::generate()?;
    if !state.install_authoriser(Authoriser::desktop_owner(&token)) {
        return Err("an authoriser was already installed".to_string());
    }
    Ok(token)
}

/// Describe this host to the API (API-S-020): host type `desktop`, no operator name, and not
/// enrolled with the hosted service until enrolment exists (API-F4.1). Fails when the state
/// already describes a host.
pub fn describe_desktop_host(state: &ApiState) -> Result<(), String> {
    if !state.describe_host(HostIdentity::desktop(), Arc::new(NotEnrolled)) {
        return Err("the host was already described".to_string());
    }
    Ok(())
}

/// A plugin with no commands that guards the app's pages: its initialization script gives
/// app pages the per-launch token, and its navigation hook refuses any navigation off the
/// app's own origins ([`event_channel::navigation_allowed`]), so the token and the event
/// channel reach app pages only.
fn app_pages_plugin(token: &LaunchToken) -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new(APP_PAGES_PLUGIN)
        .js_init_script(api_protocol::launch_token_script(token))
        .on_navigation(|webview, url| event_channel::navigation_allowed(webview.label(), url))
        .build()
}

/// Add the embedded WebDriver server of the packaged-window tests. Test-instrumented builds
/// only: it listens on `127.0.0.1`, on the port in `TAURI_WEBDRIVER_PORT`.
#[cfg(feature = "test-webdriver")]
fn with_test_driver(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder.plugin(tauri_plugin_wdio_webdriver::init())
}

/// Shipping builds add no WebDriver server and open no TCP listener.
#[cfg(not(feature = "test-webdriver"))]
fn with_test_driver(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder
}

/// Build the Tauri application with the per-launch token, the API scheme, event forwarding
/// and the monitor. Fails when the per-launch token cannot be generated.
pub fn builder() -> Result<tauri::Builder<tauri::Wry>, String> {
    let journal = HostJournal::new();
    let logging = init_tracing(&journal);
    panic_journal::install_panic_hook(journal.panic_flush.clone());
    let state = ApiState::new();
    let token = install_launch_token(&state)?;
    describe_desktop_host(&state)?;
    let files = filesquid_native::install_file_service(&state)?;
    let scheme_state = state.clone();
    let builder = with_test_driver(tauri::Builder::default())
        .plugin(app_pages_plugin(&token))
        .manage(state)
        .manage(DesktopFiles(files))
        .register_asynchronous_uri_scheme_protocol(
            api_protocol::SCHEME,
            move |_ctx, request, responder| {
                let state = scheme_state.clone();
                tauri::async_runtime::spawn(async move {
                    responder.respond(api_protocol::handle(state, request).await);
                });
            },
        )
        .register_asynchronous_uri_scheme_protocol(
            ipc_gate::IPC_SCHEME,
            |ctx, request, responder| {
                let label = ctx.webview_label().to_string();
                let reply: ipc_gate::Reply = Box::new(move |answer| responder.respond(answer));
                ipc_gate::handle(ctx.app_handle(), &label, request, reply);
            },
        )
        .setup(move |app| {
            start_monitor(app.handle(), &journal, logging)?;
            forward_events(app.handle());
            open_main_window(app.handle())
        });
    Ok(builder)
}

/// Build the main window ([`app_window::create_main_window`]). When it cannot be built, the
/// failure is already journaled; stop the monitor so the journal is flushed before the
/// process exits. Performs I/O.
fn open_main_window(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    match app_window::create_main_window(app) {
        Ok(_) => Ok(()),
        Err(err) => {
            stop_monitor(app);
            Err(Box::new(err))
        }
    }
}

/// The desktop's launch journaling: open the journal in `data_dir`, install the monitor in
/// `state` (attaching `layer` when given, so warnings and errors are journaled), journal
/// `host.started`, and journal `api.auth.token_issued` for the owner, naming the principal
/// only, never the secret. Performs filesystem and database I/O.
///
/// # Errors
/// The journal could not be opened in `data_dir`.
pub fn open_host_monitor(
    state: &ApiState,
    data_dir: &std::path::Path,
    layer: Option<&JournalLayer>,
) -> Result<HostMonitor, String> {
    let host = HostMonitor::open(data_dir, "filesquid-desktop", state, layer)?;
    record_token_issue(host.monitor(), &Principal::Owner);
    Ok(host)
}

/// The host's OCFL validation (API-S-022), held in Tauri's managed state. Opening a library
/// (DE-1.1) opens its storage root here with
/// [`ValidationService::open_library`](filesquid_api::ocfl_validation::ValidationService::open_library);
/// the monitor ticker starts the scheduled runs.
pub struct DesktopValidation(pub ValidationService<FsBackend>);

/// The durable job service of this launch (Task DE-0.4; `filesquid_native::workers`), held in Tauri's
/// managed state for the jobs that later tasks (import, upload, migration) run.
pub struct DesktopJobs(pub Jobs);

/// Whether this launch reconciled its durable jobs (DE-S-014): only then does it hand an
/// opened library's unfinished rebuild to the runner (Task DE-1.8).
pub struct DesktopRecovered(pub bool);

/// The import plans and their per-file progress (Tasks DE-1.2, DE-1.8), opened with the job
/// service, which answers the job items routes from the second.
pub struct DesktopImportStores(
    pub Arc<SqliteImportPlans>,
    pub Arc<filesquid_native::import::items::SqliteJobItems>,
);

/// The host's out-of-process libvips media workers (Task DE-0.5; DE-S-016), held in Tauri's
/// managed state for the thumbnail and preview routes. At most two run at once.
pub struct DesktopMedia(pub MediaWorkers);

/// The media worker pool of this host: this executable started with
/// [`media::WORKER_ARG`], one process per run, each registered in `host`'s process registry.
/// Reads the executable's path; starts no process.
pub fn media_workers(host: &HostMonitor) -> MediaWorkers {
    let exe = std::env::current_exe().unwrap_or_else(|err| {
        tracing::warn!(error = %err, "the host executable path is unknown; media workers will be refused");
        std::path::PathBuf::new()
    });
    MediaWorkers::new(exe, host.monitor().registry().clone())
        .with_args(vec![media::WORKER_ARG.to_string()])
}

/// Journal operation of an installation UUID copy restored from the other copy.
pub const INSTALLATION_RESTORED: &str = "host.installation.restored";

/// Journal operation of an installation UUID that could not be read or stored.
pub const INSTALLATION_FAILED: &str = "host.installation";

/// The installation UUID this launch read (DE-0.10; DE-S-012): the owner
/// agent of PROV-O records and the address of every OCFL version `user`
/// the host writes. Held in Tauri's managed state for the API.
pub struct DesktopInstallation(pub InstallationId);

/// Read the installation UUID of `data_dir`, generating it on the first launch that finds
/// none (UUID v4 from the OS random source) and restoring a missing copy from the other.
/// A new UUID is journaled as [`INSTALLATION_CREATED`], a restored copy as
/// [`INSTALLATION_RESTORED`], and a failure as [`INSTALLATION_FAILED`] `failed` with an
/// error code only: never the UUID or a path. Performs filesystem and database I/O.
///
/// # Errors
/// A copy is malformed, the copies disagree, a copy cannot be read or written, or the
/// random source failed.
pub fn load_installation(
    data_dir: &std::path::Path,
    host: &HostMonitor,
) -> Result<InstallationId, String> {
    let journal = host.monitor().journal();
    match installation::load_or_create(data_dir) {
        Ok(found) => {
            let operation = match found.source {
                InstallationSource::Created => Some(INSTALLATION_CREATED),
                InstallationSource::RestoredFile | InstallationSource::RestoredStore => {
                    Some(INSTALLATION_RESTORED)
                }
                InstallationSource::Stored => None,
            };
            if let Some(operation) = operation {
                journal.send(JournalEntry::new(operation, Outcome::Succeeded));
            }
            Ok(found.id)
        }
        Err(err) => {
            let code = installation_error_code(&err);
            journal.send(JournalEntry::new(INSTALLATION_FAILED, Outcome::Failed).error_code(code));
            Err(format!("the installation ID could not be read ({code})"))
        }
    }
}

/// The journal error code of an installation failure. Pure.
fn installation_error_code(err: &InstallationError) -> &'static str {
    match err {
        InstallationError::Malformed { .. } => "malformed",
        InstallationError::Mismatch => "mismatch",
        InstallationError::Random => "random_source",
        InstallationError::Io { .. } => "io",
    }
}

/// Journal operation of opening the library catalogs at launch.
pub const CATALOG_OPEN_OPERATION: &str = "library.catalog.open";

/// The file service of the API's file and content routes (Task API-F1.3), over the OCFL
/// storage roots of the libraries this launch opens. DE-1.1 opens each library with
/// [`filesquid_native::open_fs_library`]; until then every file route answers `not_found`.
pub struct DesktopFiles(pub filesquid_native::HostFiles);

/// Open the packaged-window tests' media library (test-instrumented builds only): the OCFL
/// storage root named by [`test_fixture::E2E_LIBRARY_VARIABLE`], when it is set, and register
/// their library folders ([`test_fixture::register_e2e_locations`]). A root or folder that
/// cannot be opened is logged; the app starts without it.
#[cfg(feature = "test-webdriver")]
fn open_test_library(app: &tauri::AppHandle, data_dir: &std::path::Path) {
    let files = &app.state::<DesktopFiles>().0;
    if let Err(message) = test_fixture::open_e2e_library(files, data_dir) {
        tracing::error!(%message, "the packaged-window media library was not opened");
    }
    let locations = &app.state::<DesktopLocations>().0;
    if let Err(message) = test_fixture::register_e2e_locations(locations) {
        tracing::error!(%message, "the packaged-window library folders were not registered");
    }
}

/// Shipping builds open no library and register no folder at launch; the owner's libraries
/// open through the library routes (DE-1.1).
#[cfg(not(feature = "test-webdriver"))]
fn open_test_library(_app: &tauri::AppHandle, _data_dir: &std::path::Path) {}

/// The library catalogs the host opened at launch: one SQLite file per library under
/// `<data dir>/libraries/` ([`CATALOG_DIR`], Task DE-0.2). Held in Tauri's managed state for
/// the API's library routes; the webview never receives a path or SQL (DE-S-006).
pub struct DesktopCatalog(pub Arc<SqliteCatalog>);

/// Open the per-library SQLite catalogs in `<data_dir>/libraries/`, forwarding their journal
/// entries to `host`'s journal store through its announcing store, so outbox entries raise
/// `journal.entry` events and count in the metrics like every other entry, and recording the
/// PROV-O history of every commit as `installation` (DE-0.10), and sending refusals that
/// come before any library is open through `host`'s bounded journal queue. A failure is journaled as
/// [`CATALOG_OPEN_OPERATION`] `failed` with its error code only (no path). Performs
/// filesystem and database I/O.
///
/// # Errors
/// The catalog directory or a catalog file could not be opened.
pub fn open_library_catalog(
    data_dir: &std::path::Path,
    host: &HostMonitor,
    installation: &InstallationId,
) -> Result<SqliteCatalog, String> {
    let journal = host.monitor().announcing_store();
    let context = provenance_context(installation);
    let opened = SqliteCatalog::open(&data_dir.join(CATALOG_DIR), journal);
    opened
        .map(|catalog| {
            catalog
                .with_provenance(context)
                .with_failure_journal(host.monitor().journal())
        })
        .map_err(|err| {
            let code = catalog_error_code(&err);
            host.monitor()
                .journal()
                .send(JournalEntry::new(CATALOG_OPEN_OPERATION, Outcome::Failed).error_code(code));
            format!("library catalogs could not be opened ({code})")
        })
}

/// Who this host's PROV-O records name: the installation agent `installation` (DE-S-012)
/// and this build's software agent ([`host_facts`]). Pure.
pub fn provenance_context(installation: &InstallationId) -> ProvenanceContext {
    ProvenanceContext {
        installation: installation.clone(),
        host: host_facts(),
    }
}

/// Install the PROV-O history source of the history routes in `state` (API-S-029): every
/// record of every library `catalog` serves and of every library opened in a folder the owner
/// chose (`opened`, Task DE-1.1), read from the libraries' OCFL storage roots for each request
/// ([`HostRoots`]), so a library opened later is included. Until it is installed the history
/// routes answer `503 unavailable`.
///
/// # Errors
/// A history source is already installed; the first one is kept.
pub fn install_history_source(
    state: &ApiState,
    catalog: Arc<SqliteCatalog>,
    opened: Arc<OpenedRoots>,
) -> Result<(), String> {
    let roots = HostRoots {
        catalog: CatalogRoots(catalog),
        opened,
    };
    let history = OcflRootsHistory::new(roots);
    if !state.install_history(Arc::new(history)) {
        return Err("a history source was already installed".to_string());
    }
    Ok(())
}

/// The local library service of this host (CL-1.1) with the repair provenance follow-up
/// (API-S-028): after each repaired object it commits a provenance-only version whose PROV-O
/// record names `installation`, shares the repair run's ID and names a journal entry ID the
/// host journal allocates before the commit. Journals through `host`'s bounded queue and
/// records repairs in `ledger`, the persistent ledger DE-1.1 injects with the repair route.
pub fn local_library_service(
    host: &HostMonitor,
    installation: &InstallationId,
    ledger: Arc<dyn RepairLedger>,
) -> LocalLibraryService {
    LocalLibraryService::new(host.monitor().journal(), ledger).with_provenance(
        provenance_context(installation),
        host.monitor().announcing_store(),
    )
}

/// Journal operation of the library routes' installation at launch.
pub const LIBRARY_ROUTES_OPERATION: &str = "library.routes.install";

/// The folders the owner registered for the library routes (Task DE-1.1), held in Tauri's
/// managed state so the host can register a folder the owner picks.
pub struct DesktopLocations(pub Arc<HostLocations>);

/// Install the library and repair routes' host side in `state` (API-S-025, Task DE-1.1): the
/// library commands over the folders the owner registers ([`HostLocations`] in `data_dir`,
/// which keeps each library's name, location and last opened time across launches in the
/// operational store, Task DE-1.8), with the repair service of [`local_library_service`] over the persistent repair ledger
/// ([`SqliteRepairLedger`] in the operational store), serving each bound library with
/// `serving`. Answers the folder registry. Opens database connections.
///
/// # Errors
/// The repair ledger or the library registry could not be opened, or library commands are
/// already installed.
pub fn install_library_routes(
    state: &ApiState,
    host: &HostMonitor,
    installation: &InstallationId,
    serving: LibraryServing,
) -> Result<Arc<HostLocations>, String> {
    let path = serving.data_dir.join(JOURNAL_FILE);
    let ledger = SqliteRepairLedger::open(&path, "filesquid-desktop").map_err(|_| {
        tracing::error!(
            operation = LIBRARY_ROUTES_OPERATION,
            error_code = "unavailable",
            "the repair ledger could not be opened"
        );
        "the repair ledger could not be opened".to_string()
    })?;
    let service = local_library_service(host, installation, Arc::new(ledger));
    let store = filesquid_native::registry_sqlite::SqliteLibraryRegistry::open(
        &serving.data_dir.join(JOURNAL_FILE),
    )?;
    let locations = Arc::new(HostLocations::persistent(&serving.data_dir, store)?);
    let libraries = DesktopLibraries::new(service, locations.clone(), serving);
    if !state.install_libraries(Arc::new(libraries)) {
        return Err("library commands were already installed".to_string());
    }
    Ok(locations)
}

/// Install this launch's durable job service `jobs` for the job routes (API-S-037) and the
/// runner of rebuild jobs over the installed library commands, run on `runtime`, the app's
/// long-lived runtime (Task API-F2.17; API-S-025 **Rebuild as a job**). Setting `stop` (the
/// app's exit) makes a running rebuild give its lease back with its checkpoint (Task DE-1.8,
/// [`rebuilds`]).
///
/// # Errors
/// The library commands are not installed yet, or a job service or a rebuild runner is
/// already installed.
pub fn install_job_routes(
    state: &ApiState,
    jobs: &Jobs,
    runtime: tokio::runtime::Handle,
    stop: Arc<AtomicBool>,
) -> Result<(), String> {
    let libraries = state
        .libraries()
        .ok_or_else(|| "the library commands are not installed".to_string())?;
    if !state.install_jobs(Arc::new(jobs.clone())) {
        return Err("a job service was already installed".to_string());
    }
    let runner = RebuildRunner::new(jobs.clone(), libraries, Principal::Owner.actor())
        .with_runtime(runtime)
        .with_stop(stop);
    if !state.install_job_runner(REBUILD_JOB_KIND, Arc::new(runner)) {
        return Err("a rebuild runner was already installed".to_string());
    }
    Ok(())
}

/// Install where this desktop keeps each user's UI preferences (API-S-011, API-F2.2): the
/// operational store in `data_dir` ([`SqlitePreferences`]). Opens a database connection.
///
/// # Errors
/// The store could not be opened, or one is already installed.
pub fn install_host_preferences(
    state: &ApiState,
    data_dir: &std::path::Path,
) -> Result<(), String> {
    let store = SqlitePreferences::open(&data_dir.join(JOURNAL_FILE))
        .map_err(|_| "the preference store could not be opened".to_string())?;
    if !state.install_preferences(Arc::new(store)) {
        return Err("a preference store was already installed".to_string());
    }
    Ok(())
}

/// Tell the file service what the listing by navigation node lists (API-F2.2, DE-1.7): the
/// folders and registered files of a collection or folder node from `catalog`'s index
/// ([`navigation::CatalogScopes`]).
///
/// # Errors
/// The file service already has its scope objects.
pub fn install_file_scopes(
    files: &filesquid_native::HostFiles,
    catalog: &Arc<SqliteCatalog>,
) -> Result<(), String> {
    if !files.install_scope_objects(Arc::new(navigation::CatalogScopes(catalog.clone()))) {
        return Err("the file service's scope objects were already installed".to_string());
    }
    Ok(())
}

/// Install `catalog` as the home collections the upload and copy routes register their new
/// files in (API-F2.14, API-S-011, API-S-026): the same registration, numbering and snapshot
/// version as an import. Pure.
///
/// # Errors
/// Home collections are already installed.
pub fn install_home_collections(
    state: &ApiState,
    catalog: &Arc<SqliteCatalog>,
) -> Result<(), String> {
    match state.install_home_collections(catalog.clone()) {
        true => Ok(()),
        false => Err("the home collections were already installed".to_string()),
    }
}

/// Install the folder choice of the library routes (`POST /api/v1/libraries/location-choices`)
/// in `state`: `dialog` shows the native folder dialog and the picked folder is registered in
/// `locations` ([`folder_choice::DesktopChooser`]). Pure.
///
/// # Errors
/// A folder chooser is already installed.
pub fn install_folder_choice(
    state: &ApiState,
    locations: &Arc<HostLocations>,
    dialog: Box<dyn folder_choice::FolderDialog>,
) -> Result<(), String> {
    let chooser = folder_choice::DesktopChooser::new(locations.clone(), dialog);
    if !state.install_location_chooser(Arc::new(chooser)) {
        return Err("a folder chooser was already installed".to_string());
    }
    Ok(())
}

/// Give the file service's restore and label commits their PROV-O record (API-F2.5): they
/// name `installation` and a journal entry ID `host`'s journal store allocates.
///
/// # Errors
/// The file service already has a provenance context.
pub fn install_file_provenance(
    files: &filesquid_native::HostFiles,
    host: &HostMonitor,
    installation: &InstallationId,
) -> Result<(), String> {
    let context = provenance_context(installation);
    if !files.install_provenance(context, host.monitor().announcing_store()) {
        return Err("the file service's provenance was already installed".to_string());
    }
    Ok(())
}

/// The history backfill job of this host (API-S-029; ADR 040 Decision 7): it commits as
/// `installation`, allocates journal IDs from `host`'s journal store, journals through its
/// bounded queue and registers each run in its process registry.
pub fn history_backfill(host: &HostMonitor, installation: &InstallationId) -> Backfill {
    Backfill::new(
        provenance_context(installation),
        host.monitor().announcing_store(),
        host.monitor().journal(),
        host.monitor().registry().clone(),
    )
}

/// The software agent of this host's PROV-O records: the product, its distribution and this
/// build's version, never a host name. Pure.
pub fn host_facts() -> HostFacts {
    HostFacts {
        product: "filesquid".into(),
        distribution: "filesquid".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    }
}

/// The journal error code of a catalog failure. Pure.
fn catalog_error_code(err: &filesquid_api::library::CatalogError) -> &'static str {
    use filesquid_api::library::CatalogError;
    match err {
        CatalogError::Conflict => "conflict",
        CatalogError::Unavailable(_) => "unavailable",
        CatalogError::Failed(_) => "internal",
    }
}

/// Open the journal in the app data directory, attach every journal sink, install the
/// monitor, journal a logging stack that could not be installed (`logging` false), read the
/// installation UUID, open the library catalogs, install the PROV-O history source over their
/// storage roots and the folders the owner opens, open the durable job store and reconcile
/// its jobs, install the navigation listing, install the library routes and the file provenance
/// ([`install_library_services`]), register the event forwarder's process, start the ticker
/// and start the history backfill job. When a step fails the failure is journaled and the
/// monitor stopped (flushing the journal) before the launch fails. Performs filesystem and
/// database I/O.
fn start_monitor(
    app: &tauri::AppHandle,
    journal: &HostJournal,
    logging: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = app.path().app_data_dir()?;
    let state = app.state::<ApiState>();
    let host = Arc::new(open_host_monitor(&state, &data_dir, Some(&journal.layer))?);
    journal.attach(host.monitor().journal());
    if !logging {
        host.monitor()
            .journal()
            .send(stdout_log::logging_failure_entry());
    }
    let installation = load_installation(&data_dir, &host).inspect_err(|_| host.stop())?;
    let catalog = Arc::new(
        open_library_catalog(&data_dir, &host, &installation).inspect_err(|_| host.stop())?,
    );
    let opened = Arc::new(OpenedRoots::new());
    install_history_source(&state, catalog.clone(), opened.clone()).inspect_err(|_| host.stop())?;
    let (task, recovered) =
        prepare_backfill(app, &data_dir, &host, &installation, &catalog, &opened)
            .inspect_err(|_| host.stop())?;
    app.manage(DesktopMedia(media_workers(&host)));
    navigation::install_navigation(&state, catalog.clone(), &installation)
        .inspect_err(|_| host.stop())?;
    app.manage(DesktopCatalog(catalog.clone()));
    let trash = recently_deleted::install_recently_deleted(&state, &host, catalog.clone(), &data_dir)
        .inspect_err(|_| host.stop())?;
    install_library_services(app, &host, &installation, &data_dir, (opened, catalog.clone()))
        .inspect_err(|_| host.stop())?;
    let runtime = tauri::async_runtime::handle().inner().clone();
    let stop = Arc::new(AtomicBool::new(false));
    let jobs = app.state::<DesktopJobs>().0.clone();
    install_job_routes(&state, &jobs, runtime, stop.clone()).inspect_err(|_| host.stop())?;
    let expiry = recently_deleted::ExpiryTask::new(trash, catalog, jobs, &installation);
    recently_deleted::spawn_expiry(expiry, stop.clone());
    app.manage(DesktopInstallation(installation));
    open_test_library(app, &data_dir);
    let forwarder = Arc::new(event_channel::register_forwarder(host.monitor().registry()));
    spawn_ticker(host.clone(), stop.clone());
    let backfill = Arc::new(DesktopBackfill::new(task, stop.clone()));
    install_backfill_route(&state, &backfill, recovered).inspect_err(|_| host.stop())?;
    let imports =
        start_imports(app, &host, &data_dir, &stop, recovered).inspect_err(|_| host.stop())?;
    app.manage(DesktopMonitor {
        host,
        forwarder,
        stop,
        backfill,
        imports,
    });
    Ok(())
}

/// Longest the app's exit waits for the import worker to suspend its job.
const IMPORT_STOP_WAIT: Duration = Duration::from_secs(10);

/// This build, as import records name it (ADR 038 Decision 3). Pure.
pub fn import_build() -> filesquid_native::import::host::Build {
    filesquid_native::import::host::Build {
        version: env!("CARGO_PKG_VERSION").into(),
        build: "filesquid-desktop".into(),
        distribution: "filesquid".into(),
    }
}

/// Start the native import worker over the import stores the job service opened (setting `stop`
/// suspends it; it runs the unfinished import jobs when the job store was `recovered`), and install
/// the import route with the native `rfd` dialog (Task DE-1.2) and the worker as the `import`
/// job runner (Task DE-1.9). Starts a thread; database I/O.
///
/// # Errors
/// The plans could not be opened, the worker not started, or the route not installed.
fn start_imports(
    app: &tauri::AppHandle,
    host: &HostMonitor,
    data_dir: &std::path::Path,
    stop: &Arc<AtomicBool>,
    recovered: bool,
) -> Result<Arc<filesquid_native::import::worker::ImportWorker>, String> {
    use filesquid_native::import::worker::{ImportServices, ImportWorker};
    let stores = app.state::<DesktopImportStores>();
    let (plans, items) = (stores.0.clone(), stores.1.clone());
    let jobs = app.state::<DesktopJobs>().0.clone();
    let files = app.state::<DesktopFiles>().0.clone();
    let catalog = app.state::<DesktopCatalog>().0.clone();
    let worker = Arc::new(ImportWorker::spawn(ImportServices {
        jobs: jobs.clone(),
        files: files.clone(),
        index: catalog.clone(),
        plans: plans.clone(),
        items: items.clone(),
        journal: host.monitor().journal(),
        build: import_build(),
        actor: Principal::Owner.actor().to_string(),
        staging_dir: data_dir.to_path_buf(),
        stop: stop.clone(),
        retry_unit: filesquid_native::import::job::RETRY_UNIT,
        recover: recovered,
    })?);
    let registry = app.state::<DesktopLocations>().0.clone();
    let starting = imports::ImportStarting {
        files,
        jobs,
        plans,
        items,
        index: catalog,
        registry,
        worker: worker.clone(),
        dialog: Box::new(imports::RfdSources::new(app.clone())),
    };
    install_import_route(&app.state::<ApiState>(), starting)?;
    install_import_runner(&app.state::<ApiState>(), worker.clone())?;
    Ok(worker)
}

/// Install the import worker as the runner of `import` jobs in `state`, so
/// `POST /api/v1/jobs/{job}/resume` hands a resumed import to it (Task DE-1.9, DE-S-020).
///
/// # Errors
/// An import runner is already installed.
pub fn install_import_runner(
    state: &ApiState,
    worker: Arc<filesquid_native::import::worker::ImportWorker>,
) -> Result<(), String> {
    if !state.install_job_runner(filesquid_native::import::job::IMPORT_KIND, worker) {
        return Err("an import runner was already installed".to_string());
    }
    Ok(())
}

/// Install the import route's starter over `starting` in `state` (API-S-036).
///
/// # Errors
/// An import starter is already installed.
pub fn install_import_route(
    state: &ApiState,
    starting: imports::ImportStarting,
) -> Result<(), String> {
    if !state.install_import_starter(Arc::new(imports::DesktopImports::new(starting))) {
        return Err("an import starter was already installed".to_string());
    }
    Ok(())
}

/// Open the durable job store in `data_dir`, reconcile its jobs and keep it in Tauri's
/// managed state, answering the history backfill task of this host over `catalog` and the
/// folders `opened`, and whether the jobs were reconciled (only then does the launch run
/// it). Performs database I/O.
///
/// # Errors
/// The job store could not be opened.
fn prepare_backfill(
    app: &tauri::AppHandle,
    data_dir: &std::path::Path,
    host: &HostMonitor,
    installation: &InstallationId,
    catalog: &Arc<SqliteCatalog>,
    opened: &Arc<OpenedRoots>,
) -> Result<(BackfillTask, bool), String> {
    let events = app.state::<ApiState>().events.clone();
    let stores = open_import_stores(&data_dir.join(JOURNAL_FILE))?;
    let jobs = history_backfill::open_jobs(data_dir, host)?
        .with_items(stores.1.clone())
        .with_events(events);
    let recovered = history_backfill::recover_jobs(&jobs).is_ok();
    let task = BackfillTask {
        jobs: jobs.clone(),
        backfill: history_backfill(host, installation),
        catalog: catalog.clone(),
        source: history_backfill::backfill_source(installation),
        opened: opened.clone(),
    };
    app.manage(DesktopJobs(jobs));
    app.manage(DesktopImportStores(stores.0, stores.1));
    app.manage(DesktopRecovered(recovered));
    Ok((task, recovered))
}

/// Install `backfill` as the launcher of the backfill route (API-S-029) and, when the jobs
/// were `recovered`, start this launch's run (DE-S-011): a run that cannot start is logged by
/// the runner, and the launch goes on.
///
/// # Errors
/// A backfill launcher is already installed.
pub fn install_backfill_route(
    state: &ApiState,
    backfill: &Arc<DesktopBackfill>,
    recovered: bool,
) -> Result<(), String> {
    if !state.install_history_backfill(backfill.clone()) {
        return Err("a history backfill launcher was already installed".to_string());
    }
    if recovered {
        let _ = backfill.start();
    }
    Ok(())
}

/// Start the OCFL validation, give the file service its PROV-O provenance and its scope
/// objects, install the catalog as the home collections of uploads and copies (API-F2.14),
/// install the preference store, install the library routes and their native folder
/// choice over the folders the owner registers, serving each library they bind
/// (Task DE-1.1); keep the validation and the folder registry in Tauri's managed state.
/// Performs filesystem and database I/O.
///
/// # Errors
/// The validation, the provenance or the library routes could not be installed.
fn install_library_services(
    app: &tauri::AppHandle,
    host: &HostMonitor,
    installation: &InstallationId,
    data_dir: &std::path::Path,
    (opened, catalog): (Arc<OpenedRoots>, Arc<SqliteCatalog>),
) -> Result<(), String> {
    let state = app.state::<ApiState>();
    let validation = host.start_validation::<FsBackend>(data_dir, &state)?;
    let files = app.state::<DesktopFiles>().0.clone();
    install_file_provenance(&files, host, installation)?;
    install_file_scopes(&files, &catalog)?;
    install_home_collections(&state, &catalog)?;
    install_host_preferences(&state, data_dir)?;
    let serving = LibraryServing {
        files,
        validation: Some(validation.clone()),
        opened,
        data_dir: data_dir.to_path_buf(),
        repository: installation_repository(installation)?,
        catalog,
        rebuilds: app
            .try_state::<DesktopRecovered>()
            .filter(|recovered| recovered.0)
            .map(|_| state.inner().clone()),
    };
    let locations = install_library_routes(&state, host, installation, serving)?;
    let dialog = Box::new(folder_choice::RfdDialog::new(app.clone()));
    install_folder_choice(&state, &locations, dialog)?;
    app.manage(DesktopValidation(validation));
    app.manage(DesktopLocations(locations));
    Ok(())
}

/// This installation as the repository its libraries name (a repository is one
/// installation). Pure.
///
/// # Errors
/// The installation UUID is not a valid repository ID (never, for a parsed UUID).
pub fn installation_repository(installation: &InstallationId) -> Result<RepositoryId, String> {
    RepositoryId::new(installation.as_str())
        .map_err(|_| "the installation is not a repository ID".to_string())
}

/// Tick the monitor once per [`TICK`] until `stop` is set at app exit (bounded by the app's lifetime).
fn spawn_ticker(host: Arc<HostMonitor>, stop: Arc<AtomicBool>) {
    tauri::async_runtime::spawn(async move {
        while !stop.load(Ordering::SeqCst) {
            tokio::time::sleep(TICK).await;
            let host = host.clone();
            if tauri::async_runtime::spawn_blocking(move || host.tick())
                .await
                .is_err()
            {
                tracing::error!("monitor tick panicked");
            }
        }
    });
}

/// Start forwarding the API bus to the app's webviews through one counted subscription
/// ([`event_channel::pump`]), reduced to what the owner may read
/// ([`navigation::owner_event_filter`]), until the bus closes or the app exits. When no subscription slot
/// is free, the refusal is journaled, the forwarder's process fails, and the API keeps
/// working without push.
fn forward_events(app: &tauri::AppHandle) {
    let Some(monitor) = app.try_state::<DesktopMonitor>() else {
        tracing::error!(
            operation = "desktop.events.subscription",
            error_code = "internal",
            "event forwarder started before the monitor"
        );
        return;
    };
    let state = app.state::<ApiState>();
    let filter = navigation::owner_event_filter(&state);
    let Some(subscription) = event_channel::open_subscription(&state.events, filter) else {
        monitor.forwarder.fail("unavailable");
        return;
    };
    let forwarding = Forwarding {
        stop: monitor.stop.clone(),
        heartbeat: FORWARDER_HEARTBEAT,
        process: Some(monitor.forwarder.clone()),
    };
    let sink = TauriSink(app.clone());
    tauri::async_runtime::spawn(async move {
        let report = event_channel::pump(subscription, &sink, forwarding).await;
        tracing::info!(
            delivered = report.delivered,
            failed = report.failed,
            end = ?report.end,
            "desktop event forwarder ended"
        );
    });
}

/// Stop the ticker, the event forwarder and the history backfill (waiting for it, bounded),
/// complete the forwarder's process, journal `host.stopped` and flush the journal.
fn stop_monitor(app: &tauri::AppHandle) {
    if let Some(monitor) = app.try_state::<DesktopMonitor>() {
        monitor.stop.store(true, Ordering::SeqCst);
        monitor.backfill.wait(BACKFILL_STOP_WAIT);
        monitor.imports.wait(IMPORT_STOP_WAIT);
        if let Some(jobs) = app.try_state::<DesktopJobs>() {
            rebuilds::wait_for_rebuilds(&jobs.0, rebuilds::REBUILD_STOP_WAIT);
        }
        monitor.forwarder.complete();
        monitor.host.stop();
    }
}

/// Journal operation of a launch that failed before the app could run.
pub const START_OPERATION: &str = "desktop.start";

/// Write a launch failure straight to stdout as a redacted JSON line marked
/// `journal_unavailable`, whatever `RUST_LOG` says: before the journal opens it is the only
/// record, and the only stdout line of the failure (its detail, which may name a path, goes to
/// stderr). Writes to stdout.
fn report_unjournaled(error_code: &str) {
    let line = panic_journal::unjournaled_line(START_OPERATION, error_code);
    panic_journal::write_line(&mut std::io::stdout(), &line);
}

/// Run the desktop event loop. Every configured window is built by the host
/// ([`app_window::host_built_windows`]) so it carries the host's guards.
pub fn run() {
    let builder = match builder() {
        Ok(builder) => builder,
        Err(message) => {
            eprintln!("FileSquid desktop could not secure its API: {message}");
            report_unjournaled("api_not_secured");
            std::process::exit(1);
        }
    };
    let mut context = tauri::generate_context!();
    app_window::host_built_windows(context.config_mut());
    let app = match builder.build(context) {
        Ok(app) => app,
        Err(err) => {
            eprintln!("FileSquid desktop could not start: {err}");
            report_unjournaled("start_failed");
            std::process::exit(1);
        }
    };
    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            stop_monitor(handle);
        }
    });
}
