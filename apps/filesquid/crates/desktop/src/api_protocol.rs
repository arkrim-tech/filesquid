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

use axum::body::Body;
use filesquid_api::admin::NEXT_CURSOR_HEADER;
use filesquid_api::auth::LaunchToken;
use filesquid_api::ApiState;
use http::header::{
    ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN,
    ACCESS_CONTROL_EXPOSE_HEADERS, ACCESS_CONTROL_MAX_AGE, ACCESS_CONTROL_REQUEST_METHOD, ORIGIN,
    VARY,
};
use http::{HeaderValue, Method, Request, Response, StatusCode};
use http_body_util::BodyExt;
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

/// Request headers an app page may send cross-origin (answered in the CORS preflight).
const ALLOWED_HEADERS: &str = "authorization, content-type";

/// Methods an app page may use cross-origin (answered in the CORS preflight).
const ALLOWED_METHODS: &str = "GET, HEAD, POST, PUT, PATCH, DELETE";

/// Seconds a webview may cache a preflight answer.
const PREFLIGHT_MAX_AGE: &str = "600";

/// Global the init script defines in app pages to hold the per-launch token.
pub const TOKEN_GLOBAL: &str = "__FILESQUID_LAUNCH_TOKEN__";

/// Answer one webview request with the FileSquid API router.
///
/// A CORS preflight from an app origin is answered here without a token (preflights never
/// carry one); every other request goes to the router, which authorises it. Returns the
/// response with its body collected, plus CORS headers when the request comes from an
/// allowed app origin. Never panics: internal failures become `500`.
pub async fn handle(state: ApiState, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let origin = request.headers().get(ORIGIN).cloned();
    let app_origin = origin.filter(is_app_origin);
    let mut response = match (&app_origin, is_preflight(&request)) {
        (Some(_), true) => preflight_response(),
        _ => route(state, request).await,
    };
    allow_app_origin(&mut response, app_origin.as_ref());
    response
}

/// True for a CORS preflight: `OPTIONS` with `Access-Control-Request-Method`.
fn is_preflight(request: &Request<Vec<u8>>) -> bool {
    request.method() == Method::OPTIONS
        && request
            .headers()
            .contains_key(ACCESS_CONTROL_REQUEST_METHOD)
}

/// The empty `204` preflight answer that lets app pages send `Authorization`.
fn preflight_response() -> Response<Vec<u8>> {
    let mut response = error_response(StatusCode::NO_CONTENT);
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

/// Rebase the scheme URL onto a path-only URI and run it through the router.
async fn route(state: ApiState, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let (mut parts, body) = request.into_parts();
    let path = parts
        .uri
        .path_and_query()
        .map(|pq| pq.as_str().to_string())
        .unwrap_or_else(|| "/".to_string());
    parts.uri = match path.parse() {
        Ok(uri) => uri,
        Err(_) => return error_response(StatusCode::BAD_REQUEST),
    };
    let routed = filesquid_api::router(state)
        .oneshot(Request::from_parts(parts, Body::from(body)))
        .await;
    let Ok(response) = routed;
    let (parts, body) = response.into_parts();
    match body.collect().await {
        Ok(collected) => Response::from_parts(parts, collected.to_bytes().to_vec()),
        Err(err) => {
            tracing::error!(%err, "API response body failed");
            error_response(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// True when `origin` is one of [`app_origins`].
fn is_app_origin(origin: &HeaderValue) -> bool {
    origin
        .to_str()
        .map(|text| app_origins().contains(&text))
        .unwrap_or(false)
}

/// Echo an app origin in `Access-Control-Allow-Origin` and expose the journal export's
/// continuation header (`X-Journal-Next-Cursor`) to it; `origin` is already filtered.
fn allow_app_origin(response: &mut Response<Vec<u8>>, origin: Option<&HeaderValue>) {
    if let Some(value) = origin {
        let headers = response.headers_mut();
        headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, value.clone());
        headers.insert(VARY, HeaderValue::from_static("Origin"));
        headers.insert(
            ACCESS_CONTROL_EXPOSE_HEADERS,
            HeaderValue::from_static(NEXT_CURSOR_HEADER),
        );
    }
}

/// Build an empty response with the given status.
fn error_response(status: StatusCode) -> Response<Vec<u8>> {
    let mut response = Response::new(Vec::new());
    *response.status_mut() = status;
    response
}
