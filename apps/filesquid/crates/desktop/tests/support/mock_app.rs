//! Test support: a Tauri app on the mock runtime with the main window, so the host's IPC gate
//! and window guards run against real Tauri code (`Webview::on_message`, the runtime
//! authority) without a display. The mock context has no capability, so Tauri's access
//! control refuses every command, the granted event commands included.

use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::{App, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// The page origin the Linux webview sends with every IPC request from the bundled UI.
pub const APP_ORIGIN: &str = "tauri://localhost";

/// A mock-runtime app and its `main` window.
pub fn mock_app() -> (App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let window = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
        .build()
        .expect("mock main window");
    (app, window)
}

/// An IPC request as Tauri's `ipc-protocol.js` sends it for `cmd` with a JSON `body`:
/// `POST ipc://localhost/<cmd>` with the invoke key, callback ids, origin and content type.
pub fn ipc_request(cmd: &str, invoke_key: &str, body: &str) -> http::Request<Vec<u8>> {
    let encoded: String = cmd
        .bytes()
        .map(|b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-' | b'.' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect();
    http::Request::builder()
        .method("POST")
        .uri(format!("ipc://localhost/{encoded}"))
        .header("Content-Type", "application/json")
        .header("Tauri-Callback", "11")
        .header("Tauri-Error", "12")
        .header("Tauri-Invoke-Key", invoke_key)
        .header("Origin", APP_ORIGIN)
        .body(body.as_bytes().to_vec())
        .expect("IPC request")
}
