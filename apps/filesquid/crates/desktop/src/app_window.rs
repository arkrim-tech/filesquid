//! The app window, built by the host with deny-by-default guards whose every refusal is
//! journaled (Task DE-0.9; DE-S-002, DE-S-003, DE-S-005;
//! [ADR 002](../../../../../resources/architecture/002-tauri2-desktop-shell.md) amendment of
//! 2026-10-18).
//!
//! A window Tauri creates from the configuration gets the platform's defaults for new-window
//! requests (`window.open`), permission requests (camera, microphone, location, …) and
//! downloads, with nothing journaled. The host therefore marks every configured window as not
//! created by Tauri ([`host_built_windows`], applied to the bundled configuration with any
//! distribution overlay already merged) and builds the main window itself
//! ([`create_main_window`]):
//!
//! - every new-window request is refused ([`NEW_WINDOW_REFUSED_OPERATION`], `new_window_denied`);
//! - every permission request is refused ([`PERMISSION_REFUSED_OPERATION`], `permission_denied`,
//!   target the permission kind);
//! - a download is allowed only for a `blob:` URL of an app page (the Monitor's journal export)
//!   and journaled ([`DOWNLOAD_OPERATION`] started, then succeeded or failed); any other is
//!   refused ([`DOWNLOAD_REFUSED_OPERATION`], `download_denied`);
//! - its opening and closing are journaled ([`OPENED_OPERATION`], [`CLOSED_OPERATION`]), and a
//!   window that cannot be built is journaled ([`CREATE_FAILED_OPERATION`]).
//!
//! Navigation off the app's origins is refused and journaled by the app-pages plugin for every
//! webview (`event_channel::navigation_allowed`). No entry holds an address, a file name, a
//! path or page content.

use crate::event_channel::is_app_url;
use filesquid_api::journal::is_opaque;
use filesquid_api::trace_layer::ACTIVITY_TARGET;
use tauri::webview::{DownloadEvent, NewWindowResponse, PermissionKind, PermissionResponse};
use tauri::{AppHandle, Runtime, Url, WebviewWindow, WebviewWindowBuilder, WindowEvent};

/// Label of the app's only window.
pub const MAIN_WINDOW: &str = "main";

/// Journal operation when the host has built an app window.
pub const OPENED_OPERATION: &str = "desktop.window.opened";

/// Journal operation when an app window has closed.
pub const CLOSED_OPERATION: &str = "desktop.window.closed";

/// Journal operation when an app window cannot be built.
pub const CREATE_FAILED_OPERATION: &str = "desktop.window.create";

/// Journal operation when a page's request for a new window is refused.
pub const NEW_WINDOW_REFUSED_OPERATION: &str = "desktop.window.refused";

/// Journal operation when a page's permission request is refused.
pub const PERMISSION_REFUSED_OPERATION: &str = "desktop.permission.refused";

/// Journal operation of an allowed download (started, then succeeded or failed).
pub const DOWNLOAD_OPERATION: &str = "desktop.download";

/// Journal operation when a download is refused.
pub const DOWNLOAD_REFUSED_OPERATION: &str = "desktop.download.refused";

/// Journal id of a permission kind whose name is not an opaque token.
const UNRECOGNISED_PERMISSION: &str = "unrecognised";

/// Mark every window of `config` as built by the host, not by Tauri, so each one gets the
/// host's guards. Pure.
pub fn host_built_windows(config: &mut tauri::Config) {
    for window in &mut config.app.windows {
        window.create = false;
    }
}

/// Build the main window from its configuration with the host's guards, and journal that it
/// opened, or that it could not be built. Performs I/O (creates the native window).
///
/// # Errors
/// The configuration has no `main` window, or Tauri cannot create it.
pub fn create_main_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<WebviewWindow<R>> {
    let built = build_main_window(app);
    match &built {
        Ok(window) => journal_window_opened(window.label()),
        Err(_) => journal_create_failed(),
    }
    built
}

/// Build the main window with the new-window, permission and download guards, and journal
/// its closing.
fn build_main_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<WebviewWindow<R>> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == MAIN_WINDOW)
        .cloned()
        .ok_or(tauri::Error::WindowNotFound)?;
    let (opener, asker, saver) = (
        config.label.clone(),
        config.label.clone(),
        config.label.clone(),
    );
    let window = WebviewWindowBuilder::from_config(app, &config)?
        .on_new_window(move |_url, _features| {
            refuse_new_window(&opener);
            NewWindowResponse::Deny
        })
        .on_permission_request(move |_webview, kind| {
            refuse_permission(&asker, &kind);
            PermissionResponse::Deny
        })
        .on_download(move |_webview, event| screen_download(&saver, event))
        .build()?;
    let label = config.label;
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            journal_window_closed(&label);
        }
    });
    Ok(window)
}

/// The journal actor of webview `label`.
fn actor(label: &str) -> String {
    format!("webview:{label}")
}

/// Journal that webview `label` was refused a new window. The address is not journaled.
pub fn refuse_new_window(label: &str) {
    tracing::warn!(
        operation = NEW_WINDOW_REFUSED_OPERATION,
        outcome = "failed",
        actor = actor(label).as_str(),
        error_code = "new_window_denied",
        "new window refused"
    );
}

/// Journal that webview `label` was refused the permission `kind`, named by kind only.
pub fn refuse_permission(label: &str, kind: &PermissionKind) {
    tracing::warn!(
        operation = PERMISSION_REFUSED_OPERATION,
        outcome = "failed",
        actor = actor(label).as_str(),
        error_code = "permission_denied",
        target_id = permission_id(kind).as_str(),
        "permission request refused"
    );
}

/// The journal id of a permission kind: its name in snake case (`Camera` → `camera`,
/// `ClipboardRead` → `clipboard_read`), or `unrecognised`. Pure.
pub fn permission_id(kind: &PermissionKind) -> String {
    let name = format!("{kind:?}");
    let mut id = String::with_capacity(name.len() + 4);
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            id.push('_');
        }
        id.push(c.to_ascii_lowercase());
    }
    if is_opaque(&id) {
        id
    } else {
        UNRECOGNISED_PERMISSION.to_string()
    }
}

/// True when a download of `url` is allowed: a `blob:` URL created by an app page. Pure.
pub fn download_allowed(url: &Url) -> bool {
    url.scheme() == "blob"
        && Url::parse(url.path())
            .map(|inner| is_app_url(&inner))
            .unwrap_or(false)
}

/// Decide one download event of webview `label` and journal it: a request is screened
/// ([`screen_download_request`]), a finished download is journaled. Any other event is
/// refused.
fn screen_download(label: &str, event: DownloadEvent<'_>) -> bool {
    match event {
        DownloadEvent::Requested { url, .. } => screen_download_request(label, &url),
        DownloadEvent::Finished { success, .. } => {
            journal_download_finished(label, success);
            true
        }
        _ => false,
    }
}

/// Allow and journal a download request of webview `label` when [`download_allowed`];
/// otherwise refuse and journal it. The address and file name are never journaled.
pub fn screen_download_request(label: &str, url: &Url) -> bool {
    let allowed = download_allowed(url);
    if allowed {
        tracing::info!(
            target: ACTIVITY_TARGET,
            operation = DOWNLOAD_OPERATION,
            outcome = "started",
            actor = actor(label).as_str(),
            "download started"
        );
    } else {
        tracing::warn!(
            operation = DOWNLOAD_REFUSED_OPERATION,
            outcome = "failed",
            actor = actor(label).as_str(),
            error_code = "download_denied",
            "download refused"
        );
    }
    allowed
}

/// Journal that a download of webview `label` finished, with whether it succeeded.
pub fn journal_download_finished(label: &str, success: bool) {
    let outcome = if success { "succeeded" } else { "failed" };
    tracing::info!(
        target: ACTIVITY_TARGET,
        operation = DOWNLOAD_OPERATION,
        outcome,
        actor = actor(label).as_str(),
        "download finished"
    );
}

/// Journal that the host built window `label`.
pub fn journal_window_opened(label: &str) {
    tracing::info!(
        target: ACTIVITY_TARGET,
        operation = OPENED_OPERATION,
        outcome = "succeeded",
        target_id = label,
        "app window opened"
    );
}

/// Journal that window `label` closed.
pub fn journal_window_closed(label: &str) {
    tracing::info!(
        target: ACTIVITY_TARGET,
        operation = CLOSED_OPERATION,
        outcome = "succeeded",
        target_id = label,
        "app window closed"
    );
}

/// Journal that the main window could not be built.
fn journal_create_failed() {
    tracing::error!(
        operation = CREATE_FAILED_OPERATION,
        outcome = "failed",
        error_code = "window_create_failed",
        "the app window could not be built"
    );
}
