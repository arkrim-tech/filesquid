//! The Recently deleted routes (Task API-F2.15; API-S-019, ADR 041 Decisions 8 to 11).
//!
//! - `POST /api/v1/recently-deleted/commands` (`files.write`): the library commands of one
//!   Trash, `{"envelope": CommandEnvelope, "input": LibraryCommand}` with the scope
//!   `library:<library_id>`: delete files one by one, delete a collection with its files,
//!   restore files (into an "untitled collection" when their home is gone, named in
//!   `restored_into`), delete files now (a confirmed purge) and set a library's own
//!   retention. Every other library command is `invalid_request`. The answer is a
//!   `CommandOutcome` with the [`CommandResult`] as `result`; a stale revision and
//!   `restore_name_exhausted` are HTTP 409 with a `conflict` outcome beside the error.
//!   A repeated operation ID of a command that changed something answers its first outcome
//!   from the catalog's receipt.
//! - `GET /api/v1/settings/recently-deleted` (`library.read`): the installation default
//!   retention, the expiry setting, whether the caller may change them, and each readable
//!   library's own period and revision.
//! - `PUT /api/v1/settings/recently-deleted` (`settings.recently_deleted.configure`): the
//!   admin (on the desktop, the owner) sets the installation default and the expiry setting.
//!
//! The library service journals what it commits and what it refuses; a handler journals only
//! what it refuses itself (a body, envelope or command it cannot take, a library the caller
//! may not read, a host without the service), once, with opaque IDs only (API-S-036).

use crate::auth::{journal_entry, Action, Principal, Resource};
use crate::envelope::{self, Command};
use crate::error::{ApiError, ErrorCode};
use crate::journal::{JournalEntry, Level, Outcome};
use crate::library::{
    CommandRequest, CommandResult, ExpiryMode, InstallationSettings, LibraryCommand,
    LibraryError, LibraryService, PageKey, SETTINGS_OPERATION,
};
use crate::navigation::NodeRef;
use crate::operation::{CommandEnvelope, OperationId};
use crate::routes::Route;
use crate::ApiState;
use axum::extract::rejection::JsonRejection;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json};
use filesquid_core::LibraryId;
use serde::Serialize;
use std::sync::Arc;

/// Journal operation of a refusal made by the command route itself.
pub const COMMAND_OPERATION: &str = "recently_deleted.command";

/// Journal operation of a refusal made by the settings read route itself.
pub const SETTINGS_READ_OPERATION: &str = "settings.recently_deleted.read";

/// Most libraries one settings answer lists (AGENTS.md §22).
pub const MAX_SETTINGS_LIBRARIES: usize = 1_000;

/// Libraries read per catalog page.
const LIBRARY_PAGE: usize = 200;

/// The route table of this module.
pub(crate) fn routes() -> Vec<Route> {
    vec![
        (
            "/recently-deleted/commands",
            post(command),
            Action::WriteFiles,
        ),
        (
            "/settings/recently-deleted",
            get(read_settings).put(write_settings),
            Action::ReadLibraries,
        ),
    ]
}

/// One library's own retention in the settings answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LibraryRetentionView {
    /// The library.
    pub library_id: String,
    /// Its name.
    pub name: String,
    /// Its own period in days, or `null` when the installation default applies.
    pub retention_days: Option<u32>,
    /// Its revision: the expected revision of its next retention command.
    pub revision: String,
    /// Whether the caller may change its period.
    pub may_change: bool,
}

/// The answer of the settings routes (mockups 49 and 50).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RecentlyDeletedSettingsView {
    /// Days a deleted file stays in Recently deleted in a library without its own period.
    pub default_retention_days: u32,
    /// What expiry does to a deleted file's object.
    pub expiry: ExpiryMode,
    /// Whether the caller may change the installation settings (only the admin).
    pub may_change: bool,
    /// The readable libraries with their own periods, in catalog order.
    pub libraries: Vec<LibraryRetentionView>,
    /// False when more libraries exist than one answer lists.
    pub complete: bool,
}

/// `POST /api/v1/recently-deleted/commands`.
async fn command(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<Command<LibraryCommand>>, JsonRejection>,
) -> Response {
    let actor = principal.actor();
    let checked = check_command(&state, &principal, request);
    let (service, request) = match checked {
        Ok(ready) => ready,
        Err((id, err)) => return refuse(&state, COMMAND_OPERATION, actor, id.as_ref(), err).await,
    };
    let operation_id = request.envelope.operation_id().clone();
    let run = move || service.execute(actor, request);
    match tokio::task::spawn_blocking(run).await {
        Ok(Ok(result)) => committed(&operation_id, &result),
        Ok(Err(err)) => envelope::refusal(Some(&operation_id), library_error(&err)),
        Err(_) => ApiError::internal(COMMAND_OPERATION, "the command task failed").into_response(),
    }
}

/// The installed service and the checked command of `request`, or the refusal with the
/// operation ID when the envelope was read.
fn check_command(
    state: &ApiState,
    principal: &Principal,
    request: Result<Json<Command<LibraryCommand>>, JsonRejection>,
) -> Result<(Arc<LibraryService>, CommandRequest), (Option<OperationId>, ApiError)> {
    let Json(body) = request.map_err(|_| (None, invalid("The body is not a Recently deleted command.")))?;
    let operation_id = Some(body.envelope.operation_id().clone());
    let library = trash_library(&body.input)
        .ok_or_else(|| (operation_id.clone(), invalid("This route takes only delete, restore, delete now and retention commands.")))?
        .clone();
    check_envelope(&body.envelope, &library).map_err(|err| (operation_id.clone(), err))?;
    if !may_read(state, principal, &library) {
        let err = ApiError::new(ErrorCode::NotFound, "That library is not known here.");
        return Err((operation_id, err));
    }
    let service = state
        .library_service()
        .ok_or_else(|| (operation_id.clone(), unavailable()))?;
    let request = CommandRequest {
        envelope: body.envelope,
        command: body.input,
    };
    Ok((service, request))
}

/// The library of a command this route takes, or `None` for any other command. Pure.
fn trash_library(command: &LibraryCommand) -> Option<&LibraryId> {
    match command {
        LibraryCommand::DeleteFiles { library_id, .. }
        | LibraryCommand::DeleteCollection { library_id, .. }
        | LibraryCommand::RestoreFiles { library_id, .. }
        | LibraryCommand::PurgeFiles { library_id, .. }
        | LibraryCommand::SetLibraryRetention { library_id, .. } => Some(library_id),
        _ => None,
    }
}

/// An envelope of schema version 1 whose scope is `library:<library_id>`.
fn check_envelope(envelope: &CommandEnvelope, library: &LibraryId) -> Result<(), ApiError> {
    let scope = format!("library:{}", library.as_str());
    envelope::check(envelope.clone(), (), &scope)
        .map(drop)
        .map_err(invalid)
}

/// `GET /api/v1/settings/recently-deleted`.
async fn read_settings(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
) -> Response {
    let actor = principal.actor();
    let Some(service) = state.library_service() else {
        return refuse(&state, SETTINGS_READ_OPERATION, actor, None, unavailable()).await;
    };
    let view = settings_view(&state, &principal, service).await;
    match view {
        Ok(view) => Json(view).into_response(),
        Err(err) => refuse(&state, SETTINGS_READ_OPERATION, actor, None, err).await,
    }
}

/// `PUT /api/v1/settings/recently-deleted`: only the admin; the guard checked `library.read`,
/// so this handler checks the admin action itself before anything else.
async fn write_settings(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<InstallationSettings>, JsonRejection>,
) -> Response {
    let actor = principal.actor();
    if !may_configure(&state, &principal) {
        let err = ApiError::denied(crate::error::DenialReason::NotPermitted);
        return refuse(&state, SETTINGS_OPERATION, actor, None, err).await;
    }
    let (Ok(Json(settings)), Some(service)) = (request, state.library_service()) else {
        let err = match state.library_service() {
            Some(_) => invalid("The body is not Recently deleted settings."),
            None => unavailable(),
        };
        return refuse(&state, SETTINGS_OPERATION, actor, None, err).await;
    };
    let store = service.clone();
    let stored = tokio::task::spawn_blocking(move || store.set_installation_settings(actor, settings)).await;
    match stored {
        Ok(Ok(_)) => match settings_view(&state, &principal, service).await {
            Ok(view) => Json(view).into_response(),
            Err(err) => err.into_response(),
        },
        Ok(Err(err)) => library_error(&err).into_response(),
        Err(_) => ApiError::internal(SETTINGS_OPERATION, "the settings task failed").into_response(),
    }
}

/// The settings answer for `principal`. Reads the settings store and the catalog on a
/// blocking thread.
async fn settings_view(
    state: &ApiState,
    principal: &Principal,
    service: Arc<LibraryService>,
) -> Result<RecentlyDeletedSettingsView, ApiError> {
    let may_change = may_configure(state, principal);
    let navigation = state.navigation();
    let principal = principal.clone();
    let read = move || -> Result<RecentlyDeletedSettingsView, LibraryError> {
        let settings = service.installation_settings()?;
        let (libraries, complete) = match navigation {
            Some(navigation) => readable_libraries(&navigation, &principal)?,
            None => (Vec::new(), true),
        };
        Ok(RecentlyDeletedSettingsView {
            default_retention_days: settings.default_retention_days,
            expiry: settings.expiry,
            may_change,
            libraries,
            complete,
        })
    };
    match tokio::task::spawn_blocking(read).await {
        Ok(view) => view.map_err(|err| library_error(&err)),
        Err(_) => Err(ApiError::internal(SETTINGS_READ_OPERATION, "the settings task failed")),
    }
}

/// The libraries of the host's repository that `principal` may read, with their own periods,
/// at most [`MAX_SETTINGS_LIBRARIES`], and whether that was all of them.
fn readable_libraries(
    navigation: &crate::navigation::NavigationService,
    principal: &Principal,
) -> Result<(Vec<LibraryRetentionView>, bool), LibraryError> {
    let catalog = navigation.catalog();
    let repository = navigation.repository_id();
    let mut views = Vec::new();
    let mut after: Option<PageKey> = None;
    for _ in 0..=MAX_SETTINGS_LIBRARIES / LIBRARY_PAGE {
        let page = catalog.libraries(repository, after.as_ref(), LIBRARY_PAGE)?;
        let full = page.len() == LIBRARY_PAGE;
        for row in page {
            after = Some(row.key);
            let id = row.item.library.id().clone();
            if !navigation.may_read(principal, &NodeRef::Library(id.clone())) {
                continue;
            }
            let Some(state) = catalog.library(&id)? else { continue };
            views.push(LibraryRetentionView {
                library_id: id.as_str().to_string(),
                name: state.record.name,
                retention_days: state.retention_days,
                revision: state.revision,
                may_change: true,
            });
            if views.len() >= MAX_SETTINGS_LIBRARIES {
                return Ok((views, false));
            }
        }
        if !full {
            return Ok((views, true));
        }
    }
    Ok((views, false))
}

/// Whether `principal` may read `library` through the host's navigation access.
fn may_read(state: &ApiState, principal: &Principal, library: &LibraryId) -> bool {
    state
        .navigation()
        .is_none_or(|nav| nav.may_read(principal, &NodeRef::Library(library.clone())))
}

/// Whether `principal` may change the installation settings: the admin action.
fn may_configure(state: &ApiState, principal: &Principal) -> bool {
    state
        .authoriser()
        .authorise(principal, Action::ConfigureRecentlyDeleted, &Resource::Host)
        .is_ok()
}

/// `200` with the `committed` outcome and `result`.
fn committed(operation_id: &OperationId, result: &CommandResult) -> Response {
    match envelope::committed(operation_id, result) {
        Some(answer) => Json(answer).into_response(),
        None => ApiError::internal(COMMAND_OPERATION, "the answer could not be encoded")
            .into_response(),
    }
}

/// The API error of a library refusal, keeping its status and message.
fn library_error(err: &LibraryError) -> ApiError {
    ApiError::from(err.clone())
}

/// `invalid_request` with `message`.
fn invalid(message: &str) -> ApiError {
    ApiError::new(ErrorCode::InvalidRequest, message)
}

/// `unavailable`: the host installed no library service.
fn unavailable() -> ApiError {
    ApiError::new(
        ErrorCode::Unavailable,
        "Recently deleted is not available on this host yet. Try again shortly.",
    )
}

/// Journal a refusal of `operation` made by a handler, once, and answer it: a conflict of a
/// command whose envelope was read as HTTP 409 with a `conflict` outcome.
async fn refuse(
    state: &ApiState,
    operation: &'static str,
    actor: &str,
    operation_id: Option<&OperationId>,
    err: ApiError,
) -> Response {
    let level = match err.code() {
        ErrorCode::Internal | ErrorCode::Unavailable => Level::Error,
        _ => Level::Warn,
    };
    let mut entry = JournalEntry::new(operation, Outcome::Failed)
        .level(level)
        .actor(actor)
        .error_code(err.code().as_str());
    if let Some(id) = operation_id {
        entry = entry.operation_id(id.as_str());
    }
    journal_entry(state, entry).await;
    envelope::refusal(operation_id, err)
}
