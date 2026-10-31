//! [`LibraryService`]: runs library commands and queries against the injected catalog.
//!
//! A command is checked (schema version, names, file batch), replayed when its operation ID
//! was already committed, planned against committed state, and handed to the catalog as one
//! [`CatalogCommit`] together with its journal entry, so the host stores both in the same
//! transaction. A command that changes nothing is handed to the catalog with no change, so
//! the catalog rechecks its expected revision atomically (a revision that moved on after
//! planning is `conflict`); it answers `committed` at the current revision and stores no
//! receipt, no journal entry and no OCFL version (owner decision, 2026-10-19), so a retry of
//! its operation ID is planned again against the state it then meets. A refused command is journaled once as
//! `failed` through the bounded journal queue every service is built with, and logged to
//! stdout. Queries page through the catalog with an opaque cursor.

use super::catalog::{
    CatalogCommit, CommitReceipt, FileListing, FileScope, Keyed, LibraryCatalog, PageKey,
};
use super::command::{CommandRequest, CommandResult};
use super::plan::{Planned, Planner};
use super::policy::{DeletionPolicy, VisibilityFilter};
use super::query::{Cursor, LibraryQuery, Page, QueryAnswer, QueryRequest};
use super::settings::{InstallationSettings, RetentionSettings, SETTINGS_OPERATION};
use super::{LibraryError, LOG_TARGET};
use crate::journal::{now_ms, JournalEntry, Level, Outcome};
use crate::journal_writer::JournalSender;
use crate::navigation::NavigationPublisher;
use crate::operation::{CommandOutcome, CommandOutcomeKind, OperationId, PROTOCOL_SCHEMA_VERSION};
use crate::registry::Clock;
use filesquid_core::{LibraryId, RepositoryId};
use std::sync::Arc;

/// Runs library commands and queries for every host.
#[derive(Clone)]
pub struct LibraryService {
    catalog: Arc<dyn LibraryCatalog>,
    clock: Clock,
    policy: DeletionPolicy,
    failures: JournalSender,
    navigation: Option<NavigationPublisher>,
    /// The installation's Recently deleted settings, when the host keeps them; the
    /// built-in defaults apply otherwise.
    settings: Option<Arc<dyn RetentionSettings>>,
}

impl std::fmt::Debug for LibraryService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryService")
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

impl LibraryService {
    /// A service over `catalog` with the system clock and the default deletion policy. Every
    /// refused command, and every catalog failure behind a query, is journaled as `failed`
    /// through `failures`, the host's bounded journal queue (producers wait; nothing is
    /// dropped).
    pub fn new(catalog: Arc<dyn LibraryCatalog>, failures: JournalSender) -> Self {
        Self {
            catalog,
            clock: Arc::new(now_ms),
            policy: DeletionPolicy::default(),
            failures,
            navigation: None,
            settings: None,
        }
    }

    /// The same service reading the installation's Recently deleted settings (default
    /// retention and expiry setting) from `settings` (ADR 041 Decisions 10 and 11).
    pub fn with_settings(mut self, settings: Arc<dyn RetentionSettings>) -> Self {
        self.settings = Some(settings);
        self
    }

    /// The installation's Recently deleted settings: the stored ones, else the defaults.
    ///
    /// # Errors
    /// `unavailable` or `internal` when the settings store fails.
    pub fn installation_settings(&self) -> Result<InstallationSettings, LibraryError> {
        match &self.settings {
            Some(store) => Ok(store.read()?),
            None => Ok(InstallationSettings {
                default_retention_days: self.policy.recoverable_days,
                ..InstallationSettings::default()
            }),
        }
    }

    /// Store new installation settings for `actor`, journaled with opaque IDs only (no value
    /// is a name or path). Only the admin may call this; the route checks it. A change that
    /// changes nothing stores nothing.
    ///
    /// # Errors
    /// `invalid_retention` for a period outside 1 to 3,650 days; `unavailable` when no store
    /// is installed or it fails. A refusal is journaled once as `failed`.
    pub fn set_installation_settings(
        &self,
        actor: &str,
        settings: InstallationSettings,
    ) -> Result<InstallationSettings, LibraryError> {
        self.store_settings(actor, settings)
            .inspect_err(|err| self.journal_failure(actor, SETTINGS_OPERATION, None, err))
    }

    /// Check and store `settings`; see [`Self::set_installation_settings`].
    fn store_settings(
        &self,
        actor: &str,
        settings: InstallationSettings,
    ) -> Result<InstallationSettings, LibraryError> {
        if !settings.is_valid() {
            return Err(LibraryError::invalid(
                "invalid_retention",
                "Choose a period between 1 and 3,650 days.",
            ));
        }
        let store = self.settings.as_ref().ok_or(LibraryError::Unavailable)?;
        if store.read()? == settings {
            return Ok(settings);
        }
        let journal = JournalEntry::new(SETTINGS_OPERATION, Outcome::Succeeded)
            .actor(actor)
            .at_ms((self.clock)());
        store.store(&settings, journal)?;
        Ok(settings)
    }

    /// Publish a `navigation.change` event after each commit that adds, removes or renames
    /// a library, collection or virtual collection, or changes which files are in view
    /// (API-S-021).
    pub fn with_navigation(mut self, publisher: NavigationPublisher) -> Self {
        self.navigation = Some(publisher);
        self
    }

    /// Use `clock` (Unix milliseconds, UTC) for deletion times and journal times.
    pub fn with_clock(mut self, clock: Clock) -> Self {
        self.clock = clock;
        self
    }

    /// The recoverable-deletion policy in force: the installation default retention, from
    /// the settings store when the host installed one.
    ///
    /// # Errors
    /// `unavailable` or `internal` when the settings store fails.
    pub fn deletion_policy(&self) -> Result<DeletionPolicy, LibraryError> {
        Ok(DeletionPolicy {
            recoverable_days: self.installation_settings()?.default_retention_days,
        })
    }

    /// Run one command for `actor` (an opaque reference such as `user:<id>`), committing
    /// its change and journal entry through the catalog.
    ///
    /// # Errors
    ///
    /// Returns a [`LibraryError`] when the command is invalid, names something outside its
    /// library, meets a stale revision (`conflict`) or the catalog fails; nothing changes.
    pub fn execute(
        &self,
        actor: &str,
        request: CommandRequest,
    ) -> Result<CommandResult, LibraryError> {
        let operation = request.command.operation();
        let library = request.command.library_id().cloned();
        self.run_command(actor, request)
            .inspect_err(|err| self.journal_failure(actor, operation, library.as_ref(), err))
    }

    /// Check, replay or plan, then commit one command.
    fn run_command(
        &self,
        actor: &str,
        request: CommandRequest,
    ) -> Result<CommandResult, LibraryError> {
        check_schema(request.envelope.schema_version())?;
        request.command.validate()?;
        let operation_id = request.envelope.operation_id().clone();
        if let Some(receipt) = self.catalog.receipt(&operation_id)? {
            return Ok(committed(&operation_id, receipt));
        }
        let policy = DeletionPolicy {
            recoverable_days: self.installation_settings()?.default_retention_days,
        };
        let planner = Planner {
            catalog: self.catalog.as_ref(),
            now_ms: (self.clock)(),
            policy,
        };
        let planned = planner.plan(&request.envelope, &request.command)?;
        self.commit_planned(actor, request, planned)
    }

    /// Commit `planned` with its journal entry and answer the receipt. A plan with no change
    /// still goes through the catalog, which rechecks the expected revision atomically and
    /// stores nothing for it, neither the journal entry nor a receipt.
    fn commit_planned(
        &self,
        actor: &str,
        request: CommandRequest,
        planned: Planned,
    ) -> Result<CommandResult, LibraryError> {
        let library_id = planned.library_id.clone().ok_or(LibraryError::Internal)?;
        let journal = planned
            .targets
            .iter()
            .fold(
                JournalEntry::new(request.command.operation(), Outcome::Succeeded).actor(actor),
                |entry, target| entry.target(target.as_str()),
            )
            .at_ms((self.clock)());
        let operation_id = request.envelope.operation_id().clone();
        let parents = super::navigation_parents(&library_id, &planned.changes);
        let receipt = self.catalog.commit(CatalogCommit {
            operation_id: operation_id.clone(),
            library_id,
            expected_revision: request.envelope.expected_revision().clone(),
            changes: planned.changes,
            journal,
            created_id: planned.created_id,
            deletion: planned.deletion,
            restored_into: planned.restored_into,
        })?;
        self.announce(parents);
        Ok(committed(&operation_id, receipt))
    }

    /// Publish the navigation change of a committed command, if a publisher is set and the
    /// command changed what navigation shows.
    /// The injected catalog.
    pub(super) fn catalog(&self) -> &dyn LibraryCatalog {
        self.catalog.as_ref()
    }

    /// Now, from the service's clock, in Unix milliseconds.
    pub(super) fn now_ms(&self) -> i64 {
        (self.clock)()
    }

    /// Hand `entry` to the bounded failure journal queue.
    pub(super) fn send_failure(&self, entry: JournalEntry) {
        self.failures.send(entry);
    }

    /// Announce that files may have left any view, so clients reread their trees.
    pub(super) fn announce_everything(&self) {
        self.announce(Some(super::NavigationParents::Everything));
    }

    fn announce(&self, change: Option<super::NavigationParents>) {
        let (Some(publisher), Some(change)) = (&self.navigation, change) else {
            return;
        };
        match change {
            super::NavigationParents::Nodes(nodes) => publisher.publish(&nodes),
            super::NavigationParents::Everything => publisher.publish_all(),
        }
    }

    /// Journal a refused command once through the bounded queue (waits while it is full),
    /// and log it to stdout. The stdout line is INFO under [`LOG_TARGET`], which the journal
    /// layer does not record, so the journal gets exactly one record.
    fn journal_failure(
        &self,
        actor: &str,
        operation: &'static str,
        library: Option<&LibraryId>,
        err: &LibraryError,
    ) {
        tracing::info!(
            target: LOG_TARGET,
            operation,
            error_code = err.error_code(),
            "library command refused"
        );
        let level = match err {
            LibraryError::Internal | LibraryError::Unavailable => Level::Error,
            _ => Level::Warn,
        };
        let entry = JournalEntry::new(operation, Outcome::Failed)
            .actor(actor)
            .error_code(err.error_code())
            .level(level)
            .at_ms((self.clock)());
        self.failures.send(match library {
            Some(lib) => entry.target(lib.as_str()),
            None => entry,
        });
    }

    /// Answer one query.
    ///
    /// # Errors
    ///
    /// Returns a [`LibraryError`] for a wrong schema version, a malformed cursor, an unknown
    /// library or scope (`not_found`) or a catalog failure. A catalog failure (`unavailable`
    /// or `internal`) is a host fault, so it is journaled as `library.query` (actor
    /// `system`); a refused request is the caller's and is only answered.
    pub fn query(&self, request: QueryRequest) -> Result<QueryAnswer, LibraryError> {
        self.run_query(request).inspect_err(|err| {
            if matches!(err, LibraryError::Unavailable | LibraryError::Internal) {
                self.journal_failure("system", "library.query", None, err);
            }
        })
    }

    /// Check the request and answer the query from the catalog.
    fn run_query(&self, request: QueryRequest) -> Result<QueryAnswer, LibraryError> {
        check_schema(request.envelope.schema_version())?;
        let after = request.cursor.as_ref().map(Cursor::decode).transpose()?;
        let limit = request.envelope.page_size().get() as usize;
        let after = after.as_ref();
        match request.query {
            LibraryQuery::ListLibraries { repository_id } => {
                self.list_libraries(&repository_id, after, limit)
            }
            LibraryQuery::OpenLibrary { library_id } => self.open_library(&library_id),
            LibraryQuery::ListCollections { library_id } => {
                self.open_library(&library_id)?;
                let rows = self.catalog.collections(&library_id, after, limit + 1)?;
                to_page(rows, limit).map(QueryAnswer::Collections)
            }
            LibraryQuery::ListVirtualCollections { library_id } => {
                self.open_library(&library_id)?;
                let rows = self
                    .catalog
                    .virtual_collections(&library_id, after, limit + 1)?;
                to_page(rows, limit).map(QueryAnswer::VirtualCollections)
            }
            LibraryQuery::ListFiles {
                library_id,
                scope,
                visibility,
            } => self.list_files(library_id, scope, visibility, after, limit),
            LibraryQuery::DeletionPolicy => Ok(QueryAnswer::DeletionPolicy(DeletionPolicy {
                recoverable_days: self.installation_settings()?.default_retention_days,
            })),
        }
    }

    /// One page of the libraries of `repository_id`.
    fn list_libraries(
        &self,
        repository_id: &RepositoryId,
        after: Option<&PageKey>,
        limit: usize,
    ) -> Result<QueryAnswer, LibraryError> {
        let rows = self.catalog.libraries(repository_id, after, limit + 1)?;
        to_page(rows, limit).map(QueryAnswer::Libraries)
    }

    /// The record and revision of `library_id`.
    fn open_library(&self, library_id: &LibraryId) -> Result<QueryAnswer, LibraryError> {
        let state = self
            .catalog
            .library(library_id)?
            .ok_or(LibraryError::NotFound("library"))?;
        Ok(QueryAnswer::Library(state))
    }

    /// One page of the files of `scope` in `library_id`, in `visibility`.
    fn list_files(
        &self,
        library_id: LibraryId,
        scope: FileScope,
        visibility: VisibilityFilter,
        after: Option<&PageKey>,
        limit: usize,
    ) -> Result<QueryAnswer, LibraryError> {
        self.open_library(&library_id)?;
        self.check_scope(&library_id, &scope)?;
        let listing = FileListing {
            library_id,
            scope,
            visibility,
        };
        let rows = self.catalog.files(&listing, after, limit + 1)?;
        to_page(rows, limit).map(QueryAnswer::Files)
    }

    /// A collection or virtual-collection scope must belong to `library_id`.
    fn check_scope(&self, library_id: &LibraryId, scope: &FileScope) -> Result<(), LibraryError> {
        let (belongs, entity) = match scope {
            FileScope::Library => (true, "library"),
            FileScope::Collection { collection_id } => (
                self.catalog
                    .collection(collection_id)?
                    .is_some_and(|c| c.library_id() == library_id),
                "collection",
            ),
            FileScope::VirtualCollection {
                virtual_collection_id,
            } => (
                self.catalog
                    .virtual_collection(virtual_collection_id)?
                    .is_some_and(|v| v.library_id() == library_id),
                "virtual_collection",
            ),
        };
        belongs.then_some(()).ok_or(LibraryError::NotFound(entity))
    }
}

/// Only [`PROTOCOL_SCHEMA_VERSION`] is understood.
fn check_schema(version: u32) -> Result<(), LibraryError> {
    if version != PROTOCOL_SCHEMA_VERSION {
        return Err(LibraryError::invalid(
            "unsupported_schema_version",
            "This request uses a protocol version FileSquid does not support.",
        ));
    }
    Ok(())
}

/// The answer to a committed (or replayed) operation: everything comes from its receipt, so
/// a replay answers exactly what the first attempt answered.
fn committed(operation_id: &OperationId, receipt: CommitReceipt) -> CommandResult {
    CommandResult {
        outcome: CommandOutcome::new(operation_id.clone(), CommandOutcomeKind::Committed),
        library_id: Some(receipt.library_id),
        revision: Some(receipt.revision),
        created_id: receipt.created_id,
        deletion: receipt.deletion,
        restored_into: receipt.restored_into,
    }
}

/// Keep at most `limit` of `rows` (fetched as `limit + 1`) and point the cursor past the
/// last kept row when more remain.
///
/// # Errors
///
/// Returns [`LibraryError::Internal`] when the last kept row's key cannot be a cursor.
fn to_page<T>(mut rows: Vec<Keyed<T>>, limit: usize) -> Result<Page<T>, LibraryError> {
    let more = rows.len() > limit;
    rows.truncate(limit);
    let next_cursor = match (more, rows.last()) {
        (true, Some(row)) => Some(Cursor::encode(&row.key)?),
        _ => None,
    };
    Ok(Page {
        items: rows.into_iter().map(|row| row.item).collect(),
        next_cursor,
    })
}
