//! The host's IPC gate: every Tauri IPC call from a webview passes the host before Tauri sees
//! it, and every refusal is journaled (Task DE-0.9; DE-S-002, DE-S-003;
//! [ADR 002](../../../../../resources/architecture/002-tauri2-desktop-shell.md) amendment of
//! 2026-10-18).
//!
//! Tauri's IPC script (`ipc-protocol.js`) sends every invoke as `POST ipc://localhost/<cmd>`
//! (`http://ipc.localhost/<cmd>` on Windows) with the invoke key, the callback ids and the
//! page origin in headers. Tauri registers its own handler for the `ipc` scheme only when the
//! app has not registered one (`tauri` 2.12 `manager/webview.rs`), so the host registers
//! [`IPC_SCHEME`] itself and owns that path without a fork:
//!
//! 1. [`decide`] (pure) checks the method, the page origin (the app's own origins only), the
//!    invoke key and the host IPC allowlist [`ALLOWED_COMMANDS`], which matches the
//!    capability policy's `GRANTED_PERMISSIONS`. Anything else is refused here, so a refused
//!    command never reaches Tauri.
//! 2. An allowed call goes to Tauri's own `on_message` ([`forward`]), which runs Tauri's
//!    access control. The host's responder sees the answer, so a call Tauri itself refuses is
//!    journaled too (`acl_denied`), and a call Tauri never answers is journaled and answered
//!    (`no_response`).
//!
//! Every refusal is journaled as [`REFUSED_OPERATION`] with the actor `webview:<label>`, the
//! refusal code and, as target, the command as an opaque id ([`command_id`]). The payload,
//! the page address and the invoke key are never journaled. Answers keep Tauri's wire format
//! (`Tauri-Response: ok|error`), so the page's `invoke` promise resolves or rejects as before.
//!
//! Tauri's `postMessage` fallback does not pass this gate; `ipc_observer` journals its refusals.

use crate::api_protocol::app_origins;
use filesquid_api::journal::is_opaque;
use http::header::{
    ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_EXPOSE_HEADERS,
    CONTENT_TYPE, ORIGIN,
};
use http::{HeaderMap, HeaderValue, Method, Request, Response, StatusCode};
use tauri::ipc::{CallbackFn, InvokeBody, InvokeError, InvokeResponse, InvokeResponseBody};
use tauri::webview::InvokeRequest;
use tauri::{AppHandle, Manager, Runtime, Url, WebviewWindow};

/// The URI scheme of Tauri's IPC, which the host registers so it handles every IPC call.
pub const IPC_SCHEME: &str = "ipc";

/// Every IPC command the host hands to Tauri: listening for and no longer listening for the
/// API's events. It matches the capability policy's `GRANTED_PERMISSIONS`
/// (`core:event:allow-listen`, `core:event:allow-unlisten`); a test keeps the two in step.
pub const ALLOWED_COMMANDS: &[&str] = &["plugin:event|listen", "plugin:event|unlisten"];

/// Journal operation of an IPC call the host or Tauri refused.
pub const REFUSED_OPERATION: &str = "desktop.ipc.refused";

/// Journal operation of an allowed IPC call that Tauri answered with an error that is not a
/// refusal.
pub const FAILED_OPERATION: &str = "desktop.ipc.failed";

/// Request header with the invoke key Tauri gives its own IPC script.
pub const INVOKE_KEY_HEADER: &str = "Tauri-Invoke-Key";

/// Request header with the id of the page's success callback.
pub const CALLBACK_HEADER: &str = "Tauri-Callback";

/// Request header with the id of the page's error callback.
pub const ERROR_HEADER: &str = "Tauri-Error";

/// Response header that tells Tauri's IPC script whether the call succeeded (`ok`) or not
/// (`error`).
pub const RESPONSE_HEADER: &str = "Tauri-Response";

/// Journal id of a command name that is not an opaque token.
pub const UNRECOGNISED_COMMAND: &str = "unrecognised";

/// Longest command name read from a request path, in bytes.
const MAX_COMMAND_LEN: usize = 256;

/// JSON media type.
const JSON: &str = "application/json";

/// Raw bytes media type, Tauri's default for a request without a content type.
const BYTES: &str = "application/octet-stream";

/// Why the host refused an IPC call, or recorded Tauri's refusal of one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Neither `POST` nor a CORS preflight.
    MethodNotAllowed,
    /// A callback header, the content type, the command or the JSON body is missing or invalid.
    MalformedRequest,
    /// The page origin is missing or not one of the app's own origins.
    ForeignOrigin,
    /// The invoke key is missing or wrong: the call does not come from Tauri's IPC script.
    InvalidInvokeKey,
    /// The command is not in [`ALLOWED_COMMANDS`].
    CommandNotAllowed,
    /// No app window has the webview's label.
    UnknownWebview,
    /// Tauri's access control refused a call the host forwarded.
    AclDenied,
    /// Tauri dropped a forwarded call without answering it.
    NoResponse,
}

impl Refusal {
    /// The journal error code.
    pub fn code(self) -> &'static str {
        match self {
            Self::MethodNotAllowed => "method_not_allowed",
            Self::MalformedRequest => "malformed_request",
            Self::ForeignOrigin => "foreign_origin",
            Self::InvalidInvokeKey => "invalid_invoke_key",
            Self::CommandNotAllowed => "command_not_allowed",
            Self::UnknownWebview => "unknown_webview",
            Self::AclDenied => "acl_denied",
            Self::NoResponse => "no_response",
        }
    }

    /// The HTTP status of the refusal answer.
    fn status(self) -> StatusCode {
        match self {
            Self::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            Self::MalformedRequest => StatusCode::BAD_REQUEST,
            Self::NoResponse => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::FORBIDDEN,
        }
    }
}

/// An IPC call that passed [`decide`]: what Tauri needs besides the headers and body.
#[derive(Debug)]
pub struct Invocation {
    /// The command, for example `plugin:event|listen`.
    pub cmd: String,
    /// The page origin, one of the app's own origins.
    pub origin: Url,
    /// The page's success callback id.
    pub callback: CallbackFn,
    /// The page's error callback id.
    pub error: CallbackFn,
    /// Whether the body is JSON (otherwise raw bytes).
    pub json: bool,
}

/// What the gate does with one IPC request.
#[derive(Debug)]
pub enum Gate {
    /// Answer the CORS preflight; it carries no call.
    Preflight,
    /// Hand the call to Tauri.
    Forward(Invocation),
    /// Refuse the call; the command, when the request names one.
    Refuse(Refusal, Option<String>),
}

/// Delivers the answer to the webview (the scheme's responder in the app, a recorder in tests).
pub type Reply = Box<dyn FnOnce(Response<Vec<u8>>) + Send>;

/// Decide what to do with one IPC request, given the app's `invoke_key`. Pure.
///
/// Checks, in order: the method (`OPTIONS` is a preflight, anything but `POST` is refused),
/// the page origin, the invoke key, the callback headers and content type, then the command
/// against [`ALLOWED_COMMANDS`].
pub fn decide(request: &Request<Vec<u8>>, invoke_key: &str) -> Gate {
    let cmd = command_of(request);
    match *request.method() {
        Method::OPTIONS => Gate::Preflight,
        Method::POST => match check(request.headers(), invoke_key, cmd.as_deref()) {
            Ok(invocation) => Gate::Forward(invocation),
            Err(refusal) => Gate::Refuse(refusal, cmd),
        },
        _ => Gate::Refuse(Refusal::MethodNotAllowed, cmd),
    }
}

/// Check a `POST` request's headers and command. Pure.
fn check(headers: &HeaderMap, invoke_key: &str, cmd: Option<&str>) -> Result<Invocation, Refusal> {
    let origin = app_origin(headers).ok_or(Refusal::ForeignOrigin)?;
    if !keys_match(header_text(headers, INVOKE_KEY_HEADER), invoke_key) {
        return Err(Refusal::InvalidInvokeKey);
    }
    let callback = callback_id(headers, CALLBACK_HEADER).ok_or(Refusal::MalformedRequest)?;
    let error = callback_id(headers, ERROR_HEADER).ok_or(Refusal::MalformedRequest)?;
    let json = body_is_json(headers).ok_or(Refusal::MalformedRequest)?;
    let cmd = cmd.ok_or(Refusal::MalformedRequest)?;
    if !ALLOWED_COMMANDS.contains(&cmd) {
        return Err(Refusal::CommandNotAllowed);
    }
    Ok(Invocation {
        cmd: cmd.to_string(),
        origin,
        callback,
        error,
        json,
    })
}

/// The command a request names: its path without the leading `/`, percent-decoded, or `None`
/// when it is empty, longer than 256 bytes or not UTF-8. Pure.
fn command_of(request: &Request<Vec<u8>>) -> Option<String> {
    let path = request.uri().path();
    let raw = path.strip_prefix('/').unwrap_or(path);
    let decoded = percent_decode(raw.as_bytes())?;
    let cmd = String::from_utf8(decoded).ok()?;
    (!cmd.is_empty() && cmd.len() <= MAX_COMMAND_LEN).then_some(cmd)
}

/// Decode `%XX` escapes; `None` for a truncated or non-hex escape. Bounded by the input length.
fn percent_decode(raw: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(raw.len());
    let mut bytes = raw.iter();
    while let Some(&byte) = bytes.next() {
        if byte != b'%' {
            out.push(byte);
            continue;
        }
        let high = (*bytes.next()? as char).to_digit(16)?;
        let low = (*bytes.next()? as char).to_digit(16)?;
        out.push((high * 16 + low) as u8);
    }
    Some(out)
}

/// The `Origin` header as a URL when it is one of the app's own origins. Pure.
fn app_origin(headers: &HeaderMap) -> Option<Url> {
    let origin = header_text(headers, ORIGIN.as_str())?;
    if !app_origins().contains(&origin) {
        return None;
    }
    Url::parse(origin).ok()
}

/// The text of header `name`, when present and visible ASCII.
fn header_text<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

/// Compare a presented key with the expected one in time independent of where they differ.
fn keys_match(presented: Option<&str>, expected: &str) -> bool {
    let Some(presented) = presented else {
        return false;
    };
    let same_len = presented.len() == expected.len();
    let diff = presented
        .bytes()
        .zip(expected.bytes())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b));
    same_len && diff == 0
}

/// A numeric callback id header.
fn callback_id(headers: &HeaderMap, name: &str) -> Option<CallbackFn> {
    header_text(headers, name)?.parse().ok().map(CallbackFn)
}

/// Whether the body is JSON (`Some(true)`) or raw bytes (`Some(false)`, also when no content
/// type is given, as Tauri reads it); `None` for any other content type.
fn body_is_json(headers: &HeaderMap) -> Option<bool> {
    let Some(value) = header_text(headers, CONTENT_TYPE.as_str()) else {
        return Some(false);
    };
    let essence = value.split(';').next().unwrap_or_default().trim();
    match essence.to_ascii_lowercase().as_str() {
        JSON => Some(true),
        BYTES => Some(false),
        _ => None,
    }
}

/// The journal id of a command name: `|` becomes `.` (`plugin:fs|read_text_file` →
/// `plugin:fs.read_text_file`), and a name that is still not an opaque token becomes
/// [`UNRECOGNISED_COMMAND`]. Pure.
pub fn command_id(cmd: &str) -> String {
    let id = cmd.replace('|', ".");
    if is_opaque(&id) {
        id
    } else {
        UNRECOGNISED_COMMAND.to_string()
    }
}

/// The journal actor of webview `label`.
fn actor(label: &str) -> String {
    format!("webview:{label}")
}

/// Answer one IPC request from webview `label` of `app`: preflights are answered, refused
/// calls are journaled and answered, and allowed calls go to Tauri ([`forward`]). Performs
/// I/O: journal writes, and Tauri's handling of an allowed call.
pub fn handle<R: Runtime>(
    app: &AppHandle<R>,
    label: &str,
    request: Request<Vec<u8>>,
    reply: Reply,
) {
    match decide(&request, app.invoke_key()) {
        Gate::Preflight => reply(preflight_response()),
        Gate::Refuse(refusal, cmd) => refuse(refusal, label, cmd.as_deref(), reply),
        Gate::Forward(invocation) => dispatch(app, label, invocation, request, reply),
    }
}

/// Journal a refusal and answer it.
fn refuse(refusal: Refusal, label: &str, cmd: Option<&str>, reply: Reply) {
    journal_refusal(refusal, label, cmd);
    reply(refusal_response(refusal, cmd));
}

/// Hand an allowed call to the window `label`, or refuse it when there is no such window or
/// its body is not valid JSON.
fn dispatch<R: Runtime>(
    app: &AppHandle<R>,
    label: &str,
    invocation: Invocation,
    request: Request<Vec<u8>>,
    reply: Reply,
) {
    let Some(window) = app.get_webview_window(label) else {
        return refuse(Refusal::UnknownWebview, label, Some(&invocation.cmd), reply);
    };
    let cmd = invocation.cmd.clone();
    match invoke_request(invocation, request) {
        Ok(request) => forward(&window, request, reply),
        Err(refusal) => refuse(refusal, label, Some(&cmd), reply),
    }
}

/// Build Tauri's invoke request from a checked call: a JSON body is parsed (an empty one is
/// `{}`, as Tauri reads it), anything else is raw bytes. Pure.
fn invoke_request(
    invocation: Invocation,
    request: Request<Vec<u8>>,
) -> Result<InvokeRequest, Refusal> {
    let (parts, body) = request.into_parts();
    let body = match (invocation.json, body.is_empty()) {
        (true, true) => InvokeBody::default(),
        (true, false) => serde_json::from_slice::<serde_json::Value>(&body)
            .map(InvokeBody::Json)
            .map_err(|_| Refusal::MalformedRequest)?,
        (false, _) => InvokeBody::Raw(body),
    };
    let invoke_key = header_text(&parts.headers, INVOKE_KEY_HEADER)
        .unwrap_or_default()
        .to_string();
    Ok(InvokeRequest {
        cmd: invocation.cmd,
        callback: invocation.callback,
        error: invocation.error,
        url: invocation.origin,
        body,
        headers: parts.headers,
        invoke_key,
    })
}

/// Hand `request` to Tauri's own IPC handling on `window`, which runs Tauri's access control
/// and the command. The answer is delivered through `reply` in Tauri's wire format; an error
/// answer is journaled ([`REFUSED_OPERATION`] with `acl_denied` for an access-control refusal,
/// otherwise [`FAILED_OPERATION`]), and a call Tauri drops unanswered is journaled and answered
/// as `no_response`. Performs I/O.
pub fn forward<R: Runtime>(window: &WebviewWindow<R>, request: InvokeRequest, reply: Reply) {
    let guard = ReplyGuard {
        reply: Some(reply),
        label: window.label().to_string(),
        cmd: request.cmd.clone(),
    };
    window.clone().on_message(
        request,
        Box::new(move |_, _, response, _, _| guard.answer(response)),
    );
}

/// Holds the reply of a forwarded call until Tauri answers it, and answers it as
/// `no_response` if Tauri drops the responder without answering.
struct ReplyGuard {
    reply: Option<Reply>,
    label: String,
    cmd: String,
}

impl ReplyGuard {
    /// Journal an error answer, then deliver Tauri's answer.
    fn answer(mut self, response: InvokeResponse) {
        if let InvokeResponse::Err(error) = &response {
            journal_tauri_error(&self.label, &self.cmd, error);
        }
        if let Some(reply) = self.reply.take() {
            reply(invoke_response(response));
        }
    }
}

impl Drop for ReplyGuard {
    fn drop(&mut self) {
        if let Some(reply) = self.reply.take() {
            refuse(Refusal::NoResponse, &self.label, Some(&self.cmd), reply);
        }
    }
}

/// True when Tauri's error answer is an access-control refusal: Tauri says the command is
/// "not allowed" (`Command <cmd> not allowed by ACL` in release builds, a longer explanation
/// in debug builds).
fn is_acl_refusal(error: &InvokeError) -> bool {
    error
        .0
        .as_str()
        .is_some_and(|text| text.contains("not allowed"))
}

/// Journal Tauri's error answer to a forwarded call, without its text.
fn journal_tauri_error(label: &str, cmd: &str, error: &InvokeError) {
    if is_acl_refusal(error) {
        return journal_refusal(Refusal::AclDenied, label, Some(cmd));
    }
    tracing::error!(
        operation = FAILED_OPERATION,
        outcome = "failed",
        actor = actor(label).as_str(),
        error_code = "command_failed",
        target_id = command_id(cmd).as_str(),
        "an allowed IPC call failed"
    );
}

/// Journal one refusal: actor `webview:<label>`, the refusal code and the command's opaque id
/// when known. Writes to the journal through the host's tracing layer.
pub fn journal_refusal(refusal: Refusal, label: &str, cmd: Option<&str>) {
    let target = cmd.map(command_id);
    tracing::warn!(
        operation = REFUSED_OPERATION,
        outcome = "failed",
        actor = actor(label).as_str(),
        error_code = refusal.code(),
        target_id = target.as_deref(),
        "IPC call refused"
    );
}

/// Headers every IPC answer carries, as Tauri's own handler sets them: any origin may read
/// the answer and its `Tauri-Response` header (the scheme is reachable from the app's
/// webviews only).
fn ipc_headers(response: &mut Response<Vec<u8>>) {
    let headers = response.headers_mut();
    headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    headers.insert(
        ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static(RESPONSE_HEADER),
    );
}

/// The answer to a CORS preflight.
fn preflight_response() -> Response<Vec<u8>> {
    let mut response = Response::new(Vec::new());
    response
        .headers_mut()
        .insert(ACCESS_CONTROL_ALLOW_HEADERS, HeaderValue::from_static("*"));
    ipc_headers(&mut response);
    response
}

/// The answer to a refused call, in Tauri's wire format: `Tauri-Response: error` and a JSON
/// string saying the command is not allowed, which rejects the page's `invoke` promise. Pure.
pub fn refusal_response(refusal: Refusal, cmd: Option<&str>) -> Response<Vec<u8>> {
    let code = refusal.code();
    let message = match cmd {
        Some(cmd) => format!("Command {cmd} not allowed by the FileSquid host ({code})"),
        None => format!("IPC request not allowed by the FileSquid host ({code})"),
    };
    let body = serde_json::Value::String(message).to_string().into_bytes();
    let mut response = typed_response(body, JSON, false);
    *response.status_mut() = refusal.status();
    response
}

/// Tauri's answer in its wire format: JSON or raw bytes, with `Tauri-Response`. Pure.
fn invoke_response(response: InvokeResponse) -> Response<Vec<u8>> {
    match response {
        InvokeResponse::Ok(InvokeResponseBody::Json(json)) => {
            typed_response(json.into_bytes(), JSON, true)
        }
        InvokeResponse::Ok(InvokeResponseBody::Raw(bytes)) => typed_response(bytes, BYTES, true),
        InvokeResponse::Err(error) => typed_response(error.0.to_string().into_bytes(), JSON, false),
    }
}

/// An IPC answer with `body`, its content type and `Tauri-Response: ok|error`.
fn typed_response(body: Vec<u8>, content_type: &'static str, ok: bool) -> Response<Vec<u8>> {
    let mut response = Response::new(body);
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    let outcome = if ok { "ok" } else { "error" };
    headers.insert(RESPONSE_HEADER, HeaderValue::from_static(outcome));
    ipc_headers(&mut response);
    response
}
