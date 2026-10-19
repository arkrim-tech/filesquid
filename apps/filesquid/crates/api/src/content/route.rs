//! The content routes: minting a handle (`POST /api/v1/content-handles`, guarded like every
//! Bearer route with [`Action::ReadFiles`]) and reading through one
//! (`GET` and `HEAD /api/v1/content/{handle}`, guarded by the handle itself).
//!
//! The content route checks its handle before anything else, so no file is read for an
//! unusable handle, and every unusable request under `/api/v1/content/` (an unknown,
//! malformed, missing, expired, evicted or revoked handle, a failed policy check, an extra
//! path segment or another method) gets the one [`denial`], journaled as `api.auth.denied`
//! with reason `invalid_credentials` and target `content.read`. The handle never reaches a
//! log line, the journal or an error answer.

use super::{ContentTarget, Minted, CONTENT_PATH_TEMPLATE, CONTENT_PREFIX, ISSUED_OPERATION};
use crate::auth::{
    deny, journal_entry, presented_credential, Action, Credential, Principal, Resource,
};
use crate::error::{ApiError, DenialReason, ErrorCode};
use crate::files::names::Disposition;
use crate::files::{
    answer_content, ContentAnswerKind, ContentRequest, FileError, MintRequest, MintView,
    RangeRequest,
};
use crate::journal::{JournalEntry, Level, Outcome};
use crate::routes::Route;
use crate::{ApiState, API_PREFIX};
use axum::body::Body;
use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{Path, State};
use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE, RANGE};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json};
use filesquid_ocfl::inventory::VersionNum;

/// The headers every content-route answer carries, denials included (API-S-018).
pub const PRIVACY_HEADERS: [(&str, &str); 4] = [
    ("referrer-policy", "no-referrer"),
    ("cache-control", "private, no-store"),
    ("x-content-type-options", "nosniff"),
    ("content-security-policy", "default-src 'none'; sandbox"),
];

/// The mint route, relative to [`API_PREFIX`].
pub(crate) fn mint_route() -> Route {
    ("/content-handles", post(mint), Action::ReadFiles)
}

/// The content route, relative to [`API_PREFIX`]: `GET` and `HEAD` only; every other method
/// gets the content denial.
pub(crate) fn content_routes() -> Route {
    let handlers = get(read).head(read).fallback(refuse_method);
    (CONTENT_PATH_TEMPLATE, handlers, Action::ReadContent)
}

/// True when `path` (with or without [`API_PREFIX`]) lies under the content route, so an
/// unusable request there gets the content denial instead of a 404, 405 or 401 fallback.
pub(crate) fn is_content_path(path: &str) -> bool {
    let relative = path.strip_prefix(API_PREFIX).unwrap_or(path);
    relative.starts_with(CONTENT_PREFIX)
}

/// The one answer every unusable content request gets: `forbidden` (403), reason
/// `invalid_credentials`, not retryable, with the privacy headers; for `HEAD` the same status
/// and headers with an empty body. It never depends on the presented handle.
pub fn denial(head_only: bool) -> Response {
    let error = ApiError::denied(DenialReason::InvalidCredentials);
    let bytes = serde_json::to_vec(&error).unwrap_or_default();
    let len = bytes.len();
    let body = match head_only {
        true => Body::empty(),
        false => Body::from(bytes),
    };
    let mut response = (StatusCode::FORBIDDEN, body).into_response();
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(CONTENT_LENGTH, HeaderValue::from(len));
    add_privacy_headers(headers);
    response
}

/// Journal and answer the content denial (API-S-017: an authorisation refusal).
pub(crate) async fn refuse_content(state: &ApiState, head_only: bool) -> Response {
    deny(
        state,
        DenialReason::InvalidCredentials,
        None,
        Some(Action::ReadContent),
    )
    .await;
    denial(head_only)
}

/// Any method but `GET` and `HEAD` on the content route.
async fn refuse_method(State(state): State<ApiState>, method: Method) -> Response {
    refuse_content(&state, method == Method::HEAD).await
}

/// `GET` and `HEAD /api/v1/content/{handle}`: accept the handle first, then answer that one
/// file version with the ranged-read handling and caps, with the privacy headers.
async fn read(
    State(state): State<ApiState>,
    method: Method,
    handle: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
) -> Response {
    let head_only = method == Method::HEAD;
    let target = handle.ok().and_then(|Path(handle)| accept(&state, &handle));
    let Some(target) = target else {
        return refuse_content(&state, head_only).await;
    };
    let range = RangeRequest::parse(headers.get(RANGE).and_then(|v| v.to_str().ok()));
    let request = ContentRequest::of_target(target, range, head_only);
    let outcome = match state.files() {
        Some(files) => files.content(request).await,
        None => Err(FileError::unavailable("no file service is installed")),
    };
    let kind = ContentAnswerKind {
        head_only,
        disposition: Disposition::Inline,
    };
    let mut response = answer_content(&state, "content_handle", outcome, kind).await;
    add_privacy_headers(response.headers_mut());
    response
}

/// The file version `handle` names, if the handle is live and its minting credential is
/// still accepted and may still read files (checked again at every request, API-S-018).
fn accept(state: &ApiState, handle: &str) -> Option<ContentTarget> {
    let authoriser = state.authoriser();
    state.content_handles().accept(handle, |credential| {
        authoriser
            .authenticate_digest(credential)
            .and_then(|principal| {
                authoriser.authorise(&principal, Action::ReadFiles, &Resource::Host)
            })
            .is_ok()
    })
}

/// Add the privacy headers to a content-route answer.
fn add_privacy_headers(headers: &mut HeaderMap) {
    for (name, value) in PRIVACY_HEADERS {
        headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
}

/// `POST /api/v1/content-handles`: mint a handle for one file version for the caller, whose
/// Bearer credential the guard has already authorised for [`Action::ReadFiles`].
async fn mint(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    headers: HeaderMap,
    request: Result<Json<MintRequest>, JsonRejection>,
) -> Response {
    let actor = principal.actor();
    let minted = async {
        let target = target(request)?;
        let files = state
            .files()
            .ok_or_else(|| FileError::unavailable("no file service is installed"))?;
        files.check_target(target.clone()).await?;
        let credential = credential_digest(&headers)?;
        let minted = state
            .content_handles()
            .mint(credential, target.clone())
            .map_err(FileError::internal)?;
        Ok::<_, FileError>((target, minted))
    }
    .await;
    match minted {
        Ok((target, minted)) => issued(&state, actor, &target, minted).await,
        Err(err) => crate::files::refuse_file(&state, ISSUED_OPERATION, actor, err).await,
    }
}

/// The checked target of a mint request: one explicit version (`v1`, `v2`, ...).
fn target(request: Result<Json<MintRequest>, JsonRejection>) -> Result<ContentTarget, FileError> {
    let Json(request) = request.map_err(|_| {
        FileError::invalid("The JSON body has unknown, missing or malformed fields.")
    })?;
    VersionNum::parse(&request.version)
        .map_err(|_| FileError::invalid("A content handle names one version, such as v1."))?;
    Ok(ContentTarget {
        library_id: request.library_id,
        object_id: request.object_id,
        version: request.version,
        path: request.path,
    })
}

/// The SHA-256 digest of the caller's Bearer credential, which the handle is tied to.
fn credential_digest(headers: &HeaderMap) -> Result<[u8; 32], FileError> {
    match presented_credential(headers) {
        Credential::Bearer(token) => Ok(crate::auth::token_digest(token)),
        _ => Err(FileError::new(
            ErrorCode::Forbidden,
            "A content handle is minted with a Bearer credential.",
        )),
    }
}

/// Journal the mint (actor, library, object, version and absolute expiry; never the handle)
/// and answer the handle.
async fn issued(state: &ApiState, actor: &str, target: &ContentTarget, minted: Minted) -> Response {
    let expires_at = rfc3339_seconds(minted.expires_ms);
    let entry = JournalEntry::new(ISSUED_OPERATION, Outcome::Succeeded)
        .level(Level::Info)
        .actor(actor)
        .target(target.library_id.as_str())
        .target(target.object_id.as_str())
        .target(target.version.as_str())
        .target(format!("expires:{expires_at}"));
    journal_entry(state, entry).await;
    let url = format!("{API_PREFIX}/content/{}", minted.handle);
    let mut response = Json(MintView {
        handle: minted.handle,
        url,
        expires_at,
    })
    .into_response();
    // The answer carries the handle: keep it out of caches and referrers like the content.
    add_privacy_headers(response.headers_mut());
    response
}

/// `ms` as RFC 3339 UTC to the second (`2026-10-18T01:00:00Z`). Pure.
fn rfc3339_seconds(ms: i64) -> String {
    use time::format_description::well_known::Rfc3339;
    time::OffsetDateTime::from_unix_timestamp(ms.div_euclid(1_000))
        .ok()
        .and_then(|t| t.format(&Rfc3339).ok())
        .unwrap_or_default()
}
