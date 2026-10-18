//! Desktop transport for the FileSquid API.
//!
//! The webview reaches the API through the `filesquid://` custom URI scheme
//! (`http://filesquid.localhost` on Windows and Android). Requests are answered in-process
//! by [`filesquid_api::router`]; no TCP port is opened (ADR 030, FileSquid T-049).
//!
//! Every request MUST carry the per-launch token as `Authorization: Bearer <token>`; the
//! router refuses anything else (deny-by-default, API-S-004). The host hands the token only
//! to pages of its own origins ([`app_origins`]) through [`launch_token_script`], and answers
//! the CORS preflight those pages send before a request with an `Authorization` header.
//! Release builds trust only the app's own scheme origins ([`RELEASE_ORIGINS`]); the Vite
//! dev-server origins ([`DEV_ORIGINS`]) are trusted in debug builds only.
//!
//! **Ranged reads.** A `Range` header reaches the router unchanged, and a `206 Partial
//! Content` answer comes back with its `Content-Range` and `Accept-Ranges` headers, which app
//! pages may send and read across origins. Tauri takes a complete body for each response, so
//! the handler buffers it, and never more than [`MAX_SCHEME_RESPONSE_BYTES`]: a file-content
//! route answers a capped range, and a larger body becomes a typed `internal` error instead
//! of unbounded memory (ADR 030, S1 results).
//!
//! **Host failures.** A request the handler cannot convert or a response it cannot build is
//! answered with the API's typed [`ApiError`] body, like every router failure (ADR 030
//! Decision 6), and journaled with its operation and error code only.

use axum::body::Body;
#[cfg(test)]
use filesquid_api::admin::NEXT_CURSOR_HEADER;
use filesquid_api::auth::LaunchToken;
use filesquid_api::{ApiError, ApiState, ErrorCode};
use http::header::{
    ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN,
    ACCESS_CONTROL_EXPOSE_HEADERS, ACCESS_CONTROL_MAX_AGE, ACCESS_CONTROL_REQUEST_METHOD,
    CONTENT_TYPE, ORIGIN, VARY,
};
use http::{HeaderValue, Method, Request, Response, StatusCode, Uri};
use http_body_util::{BodyExt, Limited};
use tower::ServiceExt;

/// Custom URI scheme name registered with Tauri.
pub const SCHEME: &str = "filesquid";

/// The app's own page origins (the bundled UI served by Tauri), trusted in every build.
pub const RELEASE_ORIGINS: &[&str] = &[
    "tauri://localhost",
    "http://tauri.localhost",
    "https://tauri.localhost",
];

/// The Vite dev-server origins of `@filesquid/ui` (5173) and ImageSquid (5174). Trusted in
/// debug builds only, so a release webview never hands the token to whatever local server
/// listens on these ports.
pub const DEV_ORIGINS: &[&str] = &["http://127.0.0.1:5173", "http://127.0.0.1:5174"];

/// Largest response body the scheme handler buffers, in bytes (16 MiB). Tauri's responder
/// takes a complete body, so this bounds the memory one request can take. File-content routes
/// MUST cap each range at or below it; a larger body is answered with a typed `internal`
/// error, never buffered.
pub const MAX_SCHEME_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

/// Page origins of the app's own webviews that may call the API cross-origin and receive the
/// per-launch token: [`RELEASE_ORIGINS`], plus [`DEV_ORIGINS`] in debug builds. The scheme is
/// only reachable from this app's webviews; the allowlist keeps other origins (for example a
/// page navigated to by mistake) from reading responses or learning the token.
pub fn app_origins() -> Vec<&'static str> {
    let mut origins = RELEASE_ORIGINS.to_vec();
    if cfg!(debug_assertions) {
        origins.extend_from_slice(DEV_ORIGINS);
    }
    origins
}

/// Request headers an app page may send cross-origin (answered in the CORS preflight):
/// the bearer token, a JSON body type and a byte range.
const ALLOWED_HEADERS: &str = "authorization, content-type, range";

/// Methods an app page may use cross-origin (answered in the CORS preflight).
const ALLOWED_METHODS: &str = "GET, HEAD, POST, PUT, PATCH, DELETE";

/// Response headers an app page may read cross-origin besides the CORS-safelisted ones: the
/// range of a partial answer, the range unit, and the journal export's continuation cursor
/// ([`filesquid_api::admin::NEXT_CURSOR_HEADER`]; a unit test keeps the two in step).
const EXPOSED_HEADERS: &str = "content-range, accept-ranges, x-journal-next-cursor";

/// Seconds a webview may cache a preflight answer.
const PREFLIGHT_MAX_AGE: &str = "600";

/// Global the init script defines in app pages to hold the per-launch token.
pub const TOKEN_GLOBAL: &str = "__FILESQUID_LAUNCH_TOKEN__";

/// Journal operation for a scheme request the handler could not convert.
const REQUEST_OPERATION: &str = "desktop.scheme.request";

/// Journal operation for a response the handler could not build or buffer.
const RESPONSE_OPERATION: &str = "desktop.scheme.response";

/// Answer one webview request with the FileSquid API router.
///
/// A CORS preflight from an app origin is answered here without a token (preflights never
/// carry one); every other request goes to the router, which authorises it. Returns the
/// response with its body collected, plus CORS headers when the request comes from an
/// allowed app origin. Never panics: host failures become typed [`ApiError`] answers.
pub async fn handle(state: ApiState, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let origin = request.headers().get(ORIGIN).cloned();
    let app_origin = origin.filter(is_app_origin);
    let mut response = match (&app_origin, is_preflight(&request)) {
        (Some(_), true) => preflight_response(),
        _ => answer(state, request).await,
    };
    allow_app_origin(&mut response, app_origin.as_ref());
    response
}

/// Send a non-preflight request to the API router (shipping builds).
#[cfg(not(feature = "test-webdriver"))]
async fn answer(state: ApiState, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    route(state, request).await
}

/// Send a non-preflight request to the 100 MB range fixture when it asks for it, otherwise
/// to the API router (test-instrumented builds only).
#[cfg(feature = "test-webdriver")]
async fn answer(state: ApiState, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    if crate::test_fixture::is_fixture_request(&request) {
        return crate::test_fixture::serve(&state, &request).await;
    }
    route(state, request).await
}

/// True for a CORS preflight: `OPTIONS` with `Access-Control-Request-Method`.
fn is_preflight(request: &Request<Vec<u8>>) -> bool {
    request.method() == Method::OPTIONS
        && request
            .headers()
            .contains_key(ACCESS_CONTROL_REQUEST_METHOD)
}

/// The empty `204` preflight answer that lets app pages send `Authorization` and `Range`.
fn preflight_response() -> Response<Vec<u8>> {
    let mut response = Response::new(Vec::new());
    *response.status_mut() = StatusCode::NO_CONTENT;
    let headers = response.headers_mut();
    headers.insert(
        ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static(ALLOWED_HEADERS),
    );
    headers.insert(
        ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static(ALLOWED_METHODS),
    );
    headers.insert(
        ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static(PREFLIGHT_MAX_AGE),
    );
    response
}

/// JavaScript that defines `window.__FILESQUID_LAUNCH_TOKEN__` (read-only, not enumerable)
/// in the main frame of pages whose origin is one of [`app_origins`], and does nothing
/// anywhere else. The host registers it as a webview initialization script, so the token
/// reaches only the app's own pages. Pure: builds text only. The origins are fixed ASCII
/// literals and the token is hex, so neither needs escaping inside the JavaScript strings.
pub fn launch_token_script(token: &LaunchToken) -> String {
    let quoted: Vec<String> = app_origins().iter().map(|o| format!("\"{o}\"")).collect();
    let origins = quoted.join(", ");
    let secret = token.secret();
    format!(
        r#"(function () {{
  var origins = [{origins}];
  if (origins.indexOf(window.location.origin) === -1) return;
  Object.defineProperty(window, "{TOKEN_GLOBAL}", {{ value: "{secret}", writable: false, enumerable: false, configurable: false }});
}})();
"#
    )
}

/// Rebase the scheme URL onto a path-only URI and run it through the router, buffering at
/// most [`MAX_SCHEME_RESPONSE_BYTES`] of its answer.
async fn route(state: ApiState, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let (mut parts, body) = request.into_parts();
    parts.uri = match path_only(&parts.uri) {
        Ok(uri) => uri,
        Err(error) => return api_error_response(&error),
    };
    let routed = filesquid_api::router(state)
        .oneshot(Request::from_parts(parts, Body::from(body)))
        .await;
    let Ok(response) = routed;
    into_scheme_response(response, MAX_SCHEME_RESPONSE_BYTES).await
}

/// The path and query of a scheme URL as a URI the router matches (`/api/v1/...`), or a typed
/// `invalid_request` error, journaled as `desktop.scheme.request`, when it has none that parses.
fn path_only(uri: &Uri) -> Result<Uri, ApiError> {
    let path = uri.path_and_query().map_or("/", |pq| pq.as_str());
    path.parse::<Uri>().map_err(|_| {
        tracing::warn!(
            operation = REQUEST_OPERATION,
            error_code = "invalid_request",
            "custom-scheme request address could not be routed"
        );
        ApiError::new(
            ErrorCode::InvalidRequest,
            "The request address is not valid.",
        )
    })
}

/// Buffer a router response for Tauri's responder, keeping its status and headers.
///
/// Reads at most `cap` bytes of the body. A longer body, or one that fails while it is read,
/// is answered with a typed `internal` [`ApiError`] instead; the detail goes to stdout and
/// the journal records only the operation `desktop.scheme.response` and the error code.
pub async fn into_scheme_response(response: Response<Body>, cap: usize) -> Response<Vec<u8>> {
    let (parts, body) = response.into_parts();
    match Limited::new(body, cap).collect().await {
        Ok(collected) => Response::from_parts(parts, collected.to_bytes().to_vec()),
        Err(err) => api_error_response(&ApiError::internal(RESPONSE_OPERATION, &err.to_string())),
    }
}

/// The scheme answer for a typed API error: its status, `application/json` and the
/// `ApiError` body (`code`, `message`, `retryable`, and `reason` for denials).
pub fn api_error_response(error: &ApiError) -> Response<Vec<u8>> {
    let body = match serde_json::to_vec(error) {
        Ok(body) => body,
        Err(err) => {
            tracing::error!(%err, operation = RESPONSE_OPERATION, error_code = "internal", "API error body failed");
            Vec::new()
        }
    };
    let mut response = Response::new(body);
    *response.status_mut() = error.code().status();
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
}

/// True when `origin` is one of [`app_origins`].
fn is_app_origin(origin: &HeaderValue) -> bool {
    origin
        .to_str()
        .map(|text| app_origins().contains(&text))
        .unwrap_or(false)
}

/// Echo an app origin in `Access-Control-Allow-Origin` and expose the range headers and the
/// journal export's continuation header to it; `origin` is already filtered.
fn allow_app_origin(response: &mut Response<Vec<u8>>, origin: Option<&HeaderValue>) {
    let Some(value) = origin else {
        return;
    };
    let headers = response.headers_mut();
    headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, value.clone());
    headers.insert(VARY, HeaderValue::from_static("Origin"));
    headers.insert(
        ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static(EXPOSED_HEADERS),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_exposed_headers_name_the_journal_cursor_header() {
        assert!(EXPOSED_HEADERS.split(", ").any(|h| h == NEXT_CURSOR_HEADER));
    }
}
