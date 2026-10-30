//! The single FileSquid API.
//!
//! Every FileSquid feature is reachable through this API, and every host serves it
//! ([ADR 030](../../../../../resources/architecture/030-filesquid-api-and-transports.md)):
//!
//! - `filesquid-desktop` mounts [`router`] behind a Tauri custom URI scheme (no TCP port);
//! - `filesquid-server` mounts it on Axum over HTTP and adds a WebSocket event route.
//!
//! The crate owns the versioned contract ([`CONTRACT`], an OpenAPI 3.1 document with the
//! [`operation`] schemas and the typed [`ApiError`] body) and the
//! in-process [`EventBus`] that hosts forward to UIs ([`events`]: documented topics, bounded
//! subscriptions and resync hints). It holds no transport code of its own, so the same router
//! answers on every host.
//!
//! It is also FileSquid's application layer: [`library`] holds the versioned library
//! commands and queries (collections, virtual collections, membership, the hide and delete
//! policy, paged listing) that every host runs over the catalog it injects.
//!
//! [`files`] serves the file operations of the libraries a host opens (listing, details,
//! history, ranged reads, upload, move, remove and restore) through the `filesquid-ocfl` view
//! API, and [`content`] lets media elements read one file version through a content handle
//! instead of a token: it ends 300 seconds after its last use (so a long playback keeps it
//! alive only while it keeps reading), 3,600 seconds after its mint at the latest, and when
//! its minting credential ends.
//!
//! [`local_library`] creates, opens, copies, rebuilds and repairs the OCFL storage roots a
//! host injects, through the library and repair routes under `/api/v1/libraries`.
//! [`navigation`] lists one level of the navigation tree per call (the repository, its
//! libraries, their collections, virtual collections and folders) with the ancestors a
//! breadcrumb needs, over the same catalog and folder listing, and announces changes with
//! the `navigation.change` event.
//!
//! [`replica`] holds what a client that works from a local copy needs: a permission-aware
//! cache identity, visible coverage and recovery states, and the ledger of pending changes
//! that become canonical only when their fenced head publication succeeds.
//!
//! Each host describes itself once at startup with [`ApiState::describe_host`] (its
//! [`host::HostType`], a server's operator name and its hosted-service enrolment port), and
//! `GET /api/v1/host` answers that [`host::HostDescriptor`] to every authenticated caller.
//!
//! Every routed operation except the public liveness probe (`GET /api/v1/health/live`) and
//! the content route (`GET` and `HEAD /api/v1/content/{handle}`) is authenticated and
//! authorised by the [`auth`] layer before its handler runs. The content route's only
//! credential is the content handle in its path, which its handler accepts, together with
//! the minting credential's permission, before it reads anything. The 404 and 405
//! fallbacks are authentication-only: they answer only a caller with an accepted credential
//! and apply no policy, because no operation runs. All of it is deny-by-default: a host
//! installs its [`auth::Authoriser`] with [`ApiState::install_authoriser`] (the desktop
//! per-launch token, or the server administrator token), and without one every guarded call
//! is refused.

pub mod admin;
pub mod auth;
pub mod canonical;
pub mod client_events;
pub mod content;
pub mod envelope;
pub mod error;
pub mod events;
pub mod files;
pub mod health;
pub mod history;
pub mod home;
pub mod host;
pub mod imports;
pub mod jobs;
pub mod journal;
pub mod journal_writer;
pub mod library;
pub mod local_library;
pub mod metrics;
pub mod monitor;
pub mod navigation;
pub mod ocfl_validation;
pub mod operation;
pub mod preferences;
pub mod registry;
pub mod replica;
mod routes;
pub mod trace_layer;

pub use error::{ApiError, DenialReason, ErrorCode};
pub use events::{ApiEvent, EventBus, EventFilter, EventSubscription};
pub use routes::{route_actions, routed_paths, router, Liveness};

use std::sync::{Arc, OnceLock};

/// Version of the FileSquid API contract. Bumped when the contract changes.
pub const API_VERSION: &str = "0.35.0";

/// URL prefix shared by every API route.
pub const API_PREFIX: &str = "/api/v1";

/// The checked-in OpenAPI 3.1 contract. TypeScript types are generated from this file.
pub const CONTRACT: &str = include_str!("../openapi.json");

/// Shared state handed to every request handler.
///
/// Cloning is cheap: the event bus and the monitor and authoriser slots are
/// reference-counted.
#[derive(Clone)]
pub struct ApiState {
    /// Bus for push events (job progress, change notifications, journal entries).
    pub events: EventBus,
    /// The host's monitor, installed once at startup (admin routes answer 503 until then).
    monitor: Arc<OnceLock<monitor::Monitor>>,
    /// The host's authoriser, installed once at startup (every guarded call is refused until then).
    authoriser: Arc<OnceLock<auth::Authoriser>>,
    /// The host's description of itself, installed once at startup (`GET /api/v1/host`
    /// answers 503 until then).
    host: Arc<OnceLock<host::HostInfo>>,
    /// The host's file service, installed once at startup (file routes answer 503 until
    /// then).
    files: Arc<OnceLock<Arc<dyn files::FileOperations>>>,
    /// The live content handles, in memory only: they end with this state (API-S-018).
    content_handles: Arc<content::ContentHandles>,
    /// The receipts of the file write commands, in memory only: a repeated operation ID
    /// answers its first answer while this state lives (API-S-002, API-S-019).
    file_receipts: Arc<files::CommandReceipts>,
    /// The host's OCFL validation, installed once at startup (the validation routes answer
    /// 503 until then).
    validation: Arc<OnceLock<Arc<dyn ocfl_validation::OcflValidation>>>,
    /// The host's history source, installed once at startup (the history routes answer 503
    /// until then).
    history: Arc<OnceLock<Arc<dyn history::HistorySource>>>,
    /// The host's library commands, installed once at startup (the library and repair
    /// routes answer 503 until then).
    libraries: Arc<OnceLock<Arc<dyn local_library::LibraryOperations>>>,
    /// The host's navigation service, installed once at startup (the navigation route
    /// answers 503 until then).
    navigation: Arc<OnceLock<Arc<navigation::NavigationService>>>,
    /// Where the host keeps each user's preferences, installed once at startup (the
    /// preference routes answer 503 until then).
    preferences: Arc<OnceLock<Arc<dyn preferences::PreferenceStore>>>,
    /// The rate bound of the client-events route, in memory only (API-S-030).
    client_events: Arc<client_events::RateLimiter>,
    /// The host's display-name lookup for journal pages, installed once at startup (without
    /// one, journal pages carry no names; API-S-031).
    journal_names: Arc<OnceLock<Arc<dyn admin::JournalNames>>>,
    /// The host's history backfill launcher, installed once at startup (the backfill route
    /// answers 503 until then; API-S-029).
    history_backfill: Arc<OnceLock<Arc<dyn history::launch::BackfillLauncher>>>,
    /// The host's folder dialog for the library routes, installed once at startup (the
    /// folder-choice route answers 503 until then; API-S-025).
    location_chooser: Arc<OnceLock<Arc<dyn local_library::choice::LocationChooser>>>,
    /// The host's durable job service for the job routes, installed once at startup (they
    /// answer 503 until then; API-S-037).
    jobs: Arc<OnceLock<Arc<jobs::Jobs>>>,
    /// The host's worker for each job kind, to which a resumed job is handed (API-S-037).
    job_runners: Arc<std::sync::Mutex<JobRunners>>,
    /// The host's import dialog and job runner, installed once at startup (the import route
    /// answers 503 until then).
    import_starter: Arc<OnceLock<Arc<dyn imports::ImportStarter>>>,
    /// The operation IDs of the import plan requests in flight, in memory only (API-S-036).
    import_flights: Arc<imports::InFlight>,
    /// The host's home collections, installed once at startup (uploads and copies answer 503
    /// until then; API-S-011, API-S-026).
    home_collections: Arc<OnceLock<Arc<dyn home::HomeCollections>>>,
}

/// The installed job runners, by job kind.
type JobRunners = std::collections::BTreeMap<String, Arc<dyn jobs::JobRunner>>;

impl std::fmt::Debug for ApiState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiState")
            .field("monitor_installed", &self.monitor.get().is_some())
            .field("authoriser_installed", &self.authoriser.get().is_some())
            .field("host", &self.host.get())
            .field("files_installed", &self.files.get().is_some())
            .field("content_handles", &self.content_handles)
            .field("validation_installed", &self.validation.get().is_some())
            .field("history_installed", &self.history.get().is_some())
            .field("libraries_installed", &self.libraries.get().is_some())
            .field("jobs_installed", &self.jobs.get().is_some())
            .field("navigation_installed", &self.navigation.get().is_some())
            .field("preferences_installed", &self.preferences.get().is_some())
            .field("client_events", &self.client_events)
            .field(
                "journal_names_installed",
                &self.journal_names.get().is_some(),
            )
            .field(
                "history_backfill_installed",
                &self.history_backfill.get().is_some(),
            )
            .field(
                "location_chooser_installed",
                &self.location_chooser.get().is_some(),
            )
            .field(
                "import_starter_installed",
                &self.import_starter.get().is_some(),
            )
            .field(
                "home_collections_installed",
                &self.home_collections.get().is_some(),
            )
            .finish()
    }
}

impl ApiState {
    /// Create state with a fresh event bus, no monitor and no authoriser (every guarded call is refused; the public liveness
    /// probe still answers).
    pub fn new() -> Self {
        Self::with_clocks(
            Arc::new(journal::now_ms),
            client_events::limiter::monotonic_clock(),
        )
    }

    /// Like [`ApiState::new`], with content handles and the client-events rate bound timed
    /// by `clock` (Unix milliseconds, UTC) instead of the system clock, so tests can move
    /// time.
    pub fn with_clock(clock: content::store::HandleClock) -> Self {
        Self::with_clocks(clock.clone(), clock)
    }

    /// State with content handles timed by the wall `clock` and the client-events rate
    /// window by `window_clock`.
    fn with_clocks(
        clock: content::store::HandleClock,
        window_clock: content::store::HandleClock,
    ) -> Self {
        Self {
            events: EventBus::new(),
            monitor: Arc::new(OnceLock::new()),
            authoriser: Arc::new(OnceLock::new()),
            host: Arc::new(OnceLock::new()),
            files: Arc::new(OnceLock::new()),
            content_handles: Arc::new(content::ContentHandles::new(clock)),
            file_receipts: Arc::new(files::CommandReceipts::new()),
            validation: Arc::new(OnceLock::new()),
            history: Arc::new(OnceLock::new()),
            libraries: Arc::new(OnceLock::new()),
            navigation: Arc::new(OnceLock::new()),
            preferences: Arc::new(OnceLock::new()),
            client_events: Arc::new(client_events::RateLimiter::new(window_clock)),
            journal_names: Arc::new(OnceLock::new()),
            history_backfill: Arc::new(OnceLock::new()),
            location_chooser: Arc::new(OnceLock::new()),
            jobs: Arc::new(OnceLock::new()),
            job_runners: Arc::new(std::sync::Mutex::new(JobRunners::new())),
            import_starter: Arc::new(OnceLock::new()),
            import_flights: Arc::new(imports::InFlight::new()),
            home_collections: Arc::new(OnceLock::new()),
        }
    }

    /// Install the host's job service for the job routes (API-S-037). Returns false, keeping
    /// the first, if one was already installed.
    pub fn install_jobs(&self, jobs: Arc<jobs::Jobs>) -> bool {
        self.jobs.set(jobs).is_ok()
    }

    /// The installed job service, if any.
    pub fn jobs(&self) -> Option<Arc<jobs::Jobs>> {
        self.jobs.get().cloned()
    }

    /// Install `runner` as the worker of job `kind`, to which `POST /api/v1/jobs/{job}/resume`
    /// hands a resumed job of that kind (API-S-037). Returns false, keeping the first, if the
    /// kind already has one.
    pub fn install_job_runner(&self, kind: &str, runner: Arc<dyn jobs::JobRunner>) -> bool {
        let mut runners = self
            .job_runners
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match runners.contains_key(kind) {
            true => false,
            false => runners.insert(kind.to_string(), runner).is_none(),
        }
    }

    /// The installed worker of job `kind`, if any.
    pub fn job_runner(&self, kind: &str) -> Option<Arc<dyn jobs::JobRunner>> {
        self.job_runners
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(kind)
            .cloned()
    }

    /// The rate bound of the client-events route.
    pub fn client_event_limiter(&self) -> &client_events::RateLimiter {
        &self.client_events
    }

    /// Install the host's display-name lookup for journal pages (API-S-031). Returns false,
    /// keeping the first, if one was already installed.
    pub fn install_journal_names(&self, names: Arc<dyn admin::JournalNames>) -> bool {
        self.journal_names.set(names).is_ok()
    }

    /// The installed display-name lookup for journal pages, if any.
    pub fn journal_names(&self) -> Option<Arc<dyn admin::JournalNames>> {
        self.journal_names.get().cloned()
    }

    /// Install the host's navigation service (API-S-021). Returns false, keeping the first,
    /// if one was already installed.
    pub fn install_navigation(&self, navigation: Arc<navigation::NavigationService>) -> bool {
        self.navigation.set(navigation).is_ok()
    }

    /// The installed navigation service, if any.
    pub fn navigation(&self) -> Option<Arc<navigation::NavigationService>> {
        self.navigation.get().cloned()
    }

    /// Install where the host keeps each user's preferences (API-S-011). Returns false,
    /// keeping the first, if a store was already installed.
    pub fn install_preferences(&self, store: Arc<dyn preferences::PreferenceStore>) -> bool {
        self.preferences.set(store).is_ok()
    }

    /// The installed preference store, if any.
    pub fn preferences(&self) -> Option<Arc<dyn preferences::PreferenceStore>> {
        self.preferences.get().cloned()
    }

    /// Install the host's file service. Returns false, keeping the first, if one was already
    /// installed.
    pub fn install_files(&self, files: Arc<dyn files::FileOperations>) -> bool {
        self.files.set(files).is_ok()
    }

    /// The installed file service, if any.
    pub fn files(&self) -> Option<Arc<dyn files::FileOperations>> {
        self.files.get().cloned()
    }

    /// The live content handles.
    pub fn content_handles(&self) -> &content::ContentHandles {
        &self.content_handles
    }

    /// The receipts of the file write commands.
    pub fn file_receipts(&self) -> &Arc<files::CommandReceipts> {
        &self.file_receipts
    }

    /// The receipts of every command route of this host: one store, so an operation ID is
    /// one idempotency key across the file write routes and the library routes (API-S-019).
    pub fn command_receipts(&self) -> &Arc<files::CommandReceipts> {
        &self.file_receipts
    }

    /// End every content handle minted with the Bearer `token` (logout, session end or
    /// rotation on `filesquid-server`). Answers how many ended. The token is used only to
    /// compute its digest and is not kept.
    pub fn end_credential(&self, token: &str) -> usize {
        self.content_handles
            .end_credential(&auth::token_digest(token))
    }

    /// Describe the host this API runs on (API-S-020): its `identity` (type and, for a server,
    /// operator name) and the `enrolment` port its hosted-service state comes from. Each host
    /// calls it once at startup; the API never derives these from a request. Returns false,
    /// keeping the first description, if the host was already described.
    pub fn describe_host(
        &self,
        identity: host::HostIdentity,
        enrolment: Arc<dyn host::EnrolmentPort>,
    ) -> bool {
        let info = host::HostInfo::new(identity, enrolment, self.events.clone());
        self.host.set(info).is_ok()
    }

    /// The described host, if the host has described itself.
    pub fn host(&self) -> Option<host::HostInfo> {
        self.host.get().cloned()
    }

    /// Install the host's authoriser. Returns false, keeping the first, if one was already
    /// installed, so a credential cannot be swapped while the host runs.
    pub fn install_authoriser(&self, authoriser: auth::Authoriser) -> bool {
        self.authoriser.set(authoriser).is_ok()
    }

    /// The installed authoriser, or one that refuses every guarded call when none is installed.
    pub fn authoriser(&self) -> auth::Authoriser {
        self.authoriser
            .get()
            .cloned()
            .unwrap_or_else(auth::Authoriser::deny_all)
    }

    /// Install the host's monitor. Returns false if one was already installed.
    pub fn install_monitor(&self, monitor: monitor::Monitor) -> bool {
        self.monitor.set(monitor).is_ok()
    }

    /// Install the host's history source (API-S-029). Returns false, keeping the first, if
    /// one was already installed.
    pub fn install_history(&self, source: Arc<dyn history::HistorySource>) -> bool {
        self.history.set(source).is_ok()
    }

    /// Install the host's history backfill launcher for the backfill route (API-S-029).
    /// Returns false, keeping the first, if one was already installed.
    pub fn install_history_backfill(
        &self,
        launcher: Arc<dyn history::launch::BackfillLauncher>,
    ) -> bool {
        self.history_backfill.set(launcher).is_ok()
    }

    /// Install the host's folder dialog for the folder-choice route (API-S-025). Returns
    /// false, keeping the first, if one was already installed.
    pub fn install_location_chooser(
        &self,
        chooser: Arc<dyn local_library::choice::LocationChooser>,
    ) -> bool {
        self.location_chooser.set(chooser).is_ok()
    }

    /// Install the host's import dialog and job runner for the import route. Returns false,
    /// keeping the first, if one was already installed.
    pub fn install_import_starter(&self, starter: Arc<dyn imports::ImportStarter>) -> bool {
        self.import_starter.set(starter).is_ok()
    }

    /// The installed import starter, if any.
    pub fn import_starter(&self) -> Option<Arc<dyn imports::ImportStarter>> {
        self.import_starter.get().cloned()
    }

    /// Install the host's home collections for the upload and copy routes. Returns false,
    /// keeping the first, if one was already installed.
    pub fn install_home_collections(&self, home: Arc<dyn home::HomeCollections>) -> bool {
        self.home_collections.set(home).is_ok()
    }

    /// The installed home collections, if any.
    pub fn home_collections(&self) -> Option<Arc<dyn home::HomeCollections>> {
        self.home_collections.get().cloned()
    }

    /// The operation IDs of the import plan requests in flight.
    pub(crate) fn import_flights(&self) -> Arc<imports::InFlight> {
        Arc::clone(&self.import_flights)
    }

    /// The installed folder dialog, if any.
    pub fn location_chooser(&self) -> Option<Arc<dyn local_library::choice::LocationChooser>> {
        self.location_chooser.get().cloned()
    }

    /// The installed history backfill launcher, if any.
    pub fn history_backfill(&self) -> Option<Arc<dyn history::launch::BackfillLauncher>> {
        self.history_backfill.get().cloned()
    }

    /// The installed history source, if any.
    pub fn history(&self) -> Option<Arc<dyn history::HistorySource>> {
        self.history.get().cloned()
    }

    /// Install the host's library commands for the library and repair routes (API-S-025).
    /// Returns false, keeping the first, if they were already installed.
    pub fn install_libraries(&self, libraries: Arc<dyn local_library::LibraryOperations>) -> bool {
        self.libraries.set(libraries).is_ok()
    }

    /// The installed library commands, if any.
    pub fn libraries(&self) -> Option<Arc<dyn local_library::LibraryOperations>> {
        self.libraries.get().cloned()
    }

    /// The installed monitor, if any.
    pub fn monitor(&self) -> Option<monitor::Monitor> {
        self.monitor.get().cloned()
    }

    /// Install the host's OCFL validation (API-S-022). Returns false if one was already
    /// installed.
    pub fn install_validation(&self, validation: Arc<dyn ocfl_validation::OcflValidation>) -> bool {
        self.validation.set(validation).is_ok()
    }

    /// The installed OCFL validation, if any.
    pub fn validation(&self) -> Option<Arc<dyn ocfl_validation::OcflValidation>> {
        self.validation.get().cloned()
    }
}

impl Default for ApiState {
    fn default() -> Self {
        Self::new()
    }
}
