//! The folder choice of the library routes (API-S-025; owner decision 2026-10-20):
//! `POST /api/v1/libraries/location-choices` (`library.manage`) asks the host to let the owner
//! pick a folder in the host's own native dialog and answers the opaque location ID the host
//! registered it under, or that the owner cancelled. The UI then names that ID in the create,
//! open and snapshot-copy routes. The folder never crosses the API as a path (API-S-005).
//!
//! The dialog is the host's, behind [`LocationChooser`], installed with
//! [`crate::ApiState::install_location_chooser`]; a host without one (`filesquid-server`)
//! answers `503 unavailable`. One dialog is open at a time: a request while one is open is
//! refused `409 conflict`. Each request is journaled once as [`CHOOSE_OPERATION`]:
//! `succeeded` with the location ID as its target, `cancelled`, or `failed` with its error
//! code; never a path or folder name.
//!
//! **Folder and volume names (Task API-F2.31; API-S-034, owner decision 2026-11-02):** a chosen
//! folder is answered with its own name and its volume's name as the host read them
//! ([`ChosenFolder`]), each passed through API-S-005's safe transformation here, whatever the
//! host handed over, and `null` when the host cannot read it. They are answered to the caller
//! only: never journaled, logged or sent as an event.

use crate::auth::{journal_entry, Action, Principal};
use crate::error::{ApiError, ErrorCode};
use crate::journal::{JournalEntry, Level, Outcome};
use crate::local_library::FolderNames;
use crate::routes::Route;
use crate::ApiState;
use axum::extract::rejection::JsonRejection;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Journal operation of a folder choice.
pub const CHOOSE_OPERATION: &str = "library.location.choose";

/// What the folder is for, so the host's dialog can say so.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChoicePurpose {
    /// An empty folder for a new library.
    Create,
    /// A folder that holds a library.
    Open,
    /// An empty folder for a snapshot copy.
    CopyDestination,
}

/// Body of `POST /api/v1/libraries/location-choices`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocationChoiceRequest {
    /// What the folder is for.
    pub purpose: ChoicePurpose,
}

/// How a folder choice ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChoiceOutcome {
    /// The owner picked a folder; `location_id` names it.
    Chosen,
    /// The owner closed the dialog without picking one.
    Cancelled,
}

/// The answer of a folder choice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocationChoice {
    /// Chosen or cancelled.
    pub outcome: ChoiceOutcome,
    /// The opaque ID the host registered the folder under; absent when cancelled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location_id: Option<String>,
    /// The chosen folder's own name (API-S-034), `null` when cancelled, when the host cannot
    /// read it, and on every host but the desktop.
    pub folder_name: Option<String>,
    /// The name of the volume that holds the chosen folder, under the same rules.
    pub volume_name: Option<String>,
}

/// A folder the owner picked, as the host registered it: its opaque location ID and the names
/// the host read of it and its volume.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChosenFolder {
    /// The opaque ID the host registered the folder under.
    pub location_id: String,
    /// The folder's own name and its volume's name, each `None` when unreadable.
    pub names: FolderNames,
}

impl ChosenFolder {
    /// The folder registered as `location_id` with `names`. Pure.
    pub fn new(location_id: impl Into<String>, names: FolderNames) -> Self {
        Self {
            location_id: location_id.into(),
            names,
        }
    }
}

/// Why a host could not offer a choice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChooseError {
    /// A dialog is already open.
    Busy,
    /// The picked folder could not be registered (not a plain folder, or the host's
    /// registry is full).
    Refused,
    /// The host cannot show a dialog now.
    Unavailable,
}

impl ChooseError {
    /// The journal error code.
    pub fn error_code(&self) -> &'static str {
        match self {
            ChooseError::Busy => "dialog_open",
            ChooseError::Refused => "location_refused",
            ChooseError::Unavailable => "unavailable",
        }
    }
}

/// The host's folder dialog and registry.
pub trait LocationChooser: Send + Sync {
    /// Let the owner pick a folder for `purpose` and register it, answering its opaque
    /// location ID with the names the host read of the folder and its volume, or `None` when
    /// the owner cancelled. Blocks until the dialog closes.
    ///
    /// # Errors
    ///
    /// A [`ChooseError`]; nothing is registered.
    fn choose(&self, purpose: ChoicePurpose) -> Result<Option<ChosenFolder>, ChooseError>;
}

/// The folder-choice route, relative to [`crate::API_PREFIX`].
pub(crate) fn routes() -> Vec<Route> {
    vec![(
        "/libraries/location-choices",
        post(choose),
        Action::ManageLibraries,
    )]
}

/// `POST /api/v1/libraries/location-choices`.
async fn choose(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<LocationChoiceRequest>, JsonRejection>,
) -> Response {
    let actor = principal.actor().to_string();
    let Ok(Json(request)) = request else {
        return refuse_invalid(&state, &actor).await;
    };
    let chosen = match state.location_chooser() {
        Some(chooser) => run(chooser, request.purpose).await,
        None => Err(ChooseError::Unavailable),
    };
    match chosen {
        Ok(location) => answer(&state, &actor, location).await,
        Err(err) => refuse(&state, &actor, err).await,
    }
}

/// Run the host's dialog off the async executor; a panicked chooser is unavailable.
async fn run(
    chooser: Arc<dyn LocationChooser>,
    purpose: ChoicePurpose,
) -> Result<Option<ChosenFolder>, ChooseError> {
    let joined = tokio::task::spawn_blocking(move || chooser.choose(purpose)).await;
    joined.unwrap_or(Err(ChooseError::Unavailable))
}

/// Journal the choice with its opaque location ID only, and answer it with the folder's safe
/// names.
async fn answer(state: &ApiState, actor: &str, chosen: Option<ChosenFolder>) -> Response {
    let entry = match &chosen {
        Some(folder) => JournalEntry::new(CHOOSE_OPERATION, Outcome::Succeeded)
            .target(folder.location_id.as_str()),
        None => JournalEntry::new(CHOOSE_OPERATION, Outcome::Cancelled),
    };
    journal_entry(state, entry.actor(actor)).await;
    Json(choice_answer(chosen)).into_response()
}

/// The answer of `chosen`: its location ID and safe names, or cancelled. Pure.
fn choice_answer(chosen: Option<ChosenFolder>) -> LocationChoice {
    let Some(folder) = chosen else {
        return LocationChoice {
            outcome: ChoiceOutcome::Cancelled,
            location_id: None,
            folder_name: None,
            volume_name: None,
        };
    };
    let names = folder.names.made_safe();
    LocationChoice {
        outcome: ChoiceOutcome::Chosen,
        location_id: Some(folder.location_id),
        folder_name: names.folder_name,
        volume_name: names.volume_name,
    }
}

/// Journal an unreadable body once and answer `400 invalid_request`.
async fn refuse_invalid(state: &ApiState, actor: &str) -> Response {
    let entry = JournalEntry::new(CHOOSE_OPERATION, Outcome::Failed)
        .level(Level::Warn)
        .actor(actor)
        .error_code("invalid_request");
    journal_entry(state, entry).await;
    let message = "The request names unknown, missing or malformed fields.";
    ApiError::new(ErrorCode::InvalidRequest, message).into_response()
}

/// Journal the refusal once and answer its typed error.
async fn refuse(state: &ApiState, actor: &str, err: ChooseError) -> Response {
    let (level, code, message) = match err {
        ChooseError::Busy => (
            Level::Warn,
            ErrorCode::Conflict,
            "A folder dialog is already open.",
        ),
        ChooseError::Refused => (
            Level::Warn,
            ErrorCode::Conflict,
            "That folder cannot hold a library. Choose a plain folder.",
        ),
        ChooseError::Unavailable => (
            Level::Error,
            ErrorCode::Unavailable,
            "This host cannot show a folder dialog now.",
        ),
    };
    let entry = JournalEntry::new(CHOOSE_OPERATION, Outcome::Failed)
        .level(level)
        .actor(actor)
        .error_code(err.error_code());
    journal_entry(state, entry).await;
    ApiError::new(code, message).into_response()
}
