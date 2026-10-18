# Architecture Decision Record - FileSquid

002-tauri2-desktop-shell.md

## Title

Tauri 2 desktop shell with IPC to Rust and universal CSS

## Status

Accepted

Accepted: 2026-09-16T01:01:07Z. Revised: 2026-09-22T04:34:37Z.

Amended 2026-10-17 by [ADR 035](./035-macos-first-desktop-mvp.md): macOS is the first desktop target and the FileSquid MVP is desktop-only; Linux and Windows desktop follow the MVP. The Linux-first statements below are kept as history.

Amended 2026-10-15 by [ADR 030](./030-filesquid-api-and-transports.md): the UI reaches the one FileSquid API through a Tauri custom URI scheme served in-process, replacing per-feature IPC commands; there is still no TCP listener. Transport spike S1 is complete and accepted; S1 item (a), `Range` media playback through the custom scheme on each desktop webview, remains an open gate before the file-content endpoints are accepted ([ADR 030](./030-filesquid-api-and-transports.md) S1 results). Amended 2026-10-18 by [ADR 030](./030-filesquid-api-and-transports.md) Decision 5 (owner decision): media elements load through short-lived content handles served by the same custom scheme, so media playback still needs no TCP listener. Svelte is replaced by vanilla TypeScript ([ADR 031](./031-vanilla-typescript-ui-and-white-label-theming.md)). The host crate is `filesquid-desktop` at `apps/filesquid/crates/desktop/`.

Amended 2026-10-18 in Task DE-0.1 (capability JSON): the main window's only capability (`capabilities/default.json`) grants `core:event:allow-listen` and `core:event:allow-unlisten` and nothing else. Filesystem and SQLite access stay in the Rust host behind the API authoriser; the webview gets no fs, sql, shell, dialog, http, process or opener permission, no `core:path` (it returns absolute paths), no asset protocol and no remote context. Tauri checks app commands against the capabilities only when the app has an ACL manifest, and `tauri-build` creates one only when the app has at least one permission. So `build.rs` passes `tauri_build::AppManifest` the policy's command list (empty today; each listed command gets generated `allow-`/`deny-` permissions), and `permissions/app-acl.toml` declares one permission that allows nothing, so the manifest exists while there is no command. An unlisted custom command is then refused in the window: in the packaged-window test, `health_ping` went from "Command health_ping not found" to "health_ping not allowed". The policy lives in `src/capability_policy.rs` and its input reader in `src/capability_inputs.rs`. `build.rs` fails the build when any of these breaks the policy: a capability file; the effective configuration (`tauri.conf.json` merged with `tauri.<platform>.conf.json` and the CLI's `TAURI_CONFIG`, as Tauri merges them; windows other than `main` included); a JSON5 or TOML configuration file it cannot read; `Cargo.toml`; a linked non-core Tauri plugin (except `tauri-plugin-wdio-webdriver` in test-instrumented builds); the app ACL file; or a `#[tauri::command]` ([DE-S-002](../../apps/filesquid/crates/desktop/resources/srs.md)). The note recorded here on 2026-10-18 that Tauri's own access-control refusals reached the page but not the journal is superseded by the Task DE-0.9 amendment below.

Amended 2026-10-18 in Task DE-0.9 (every refusal journaled; owner decision of 2026-10-18 that everything that happens in the FileSquid stack is logged): Tauri 2.12 has no hook on its access-control refusals, so the host takes the IPC path itself, without a fork of Tauri. Tauri registers its own handler for the `ipc` URI scheme only when the app has not registered one, so the host registers it (`src/ipc_gate.rs`). Every IPC call Tauri's IPC script sends passes the host first: it checks the method, the page origin (the app's own origins only), the invoke key (`AppHandle::invoke_key`) and an IPC allowlist that matches the granted permissions (`plugin:event|listen`, `plugin:event|unlisten`), and refuses anything else before Tauri sees it. An allowed call goes to Tauri's own `Webview::on_message`, so Tauri's access control still runs, with a host responder that journals Tauri's refusal (`acl_denied`) and a guard that journals and answers a call Tauri drops (`no_response`). Answers keep Tauri's wire format. Wry's `postMessage` handler, which Tauri installs on every webview and a page script can call, and Tauri's silent invoke-key check stay inside Tauri; with Tauri's `tracing` feature they report through `tracing`, and a host `tracing` layer (`src/ipc_observer.rs`) journals their refusals. That layer alone handles Tauri's IPC trace and webview errors, so neither a request body Tauri records in its IPC spans nor the invoke key Tauri names in its message reaches stdout or the journal, whatever `RUST_LOG` says; an unreadable `postMessage` call is journaled as `malformed_request`. The host now builds the main window itself (every configured window, distribution overlays included, is marked `create: false` at launch; `src/app_window.rs`) so it can refuse and journal every new-window request, every permission request (camera, microphone, location and the rest) and every download that is not a `blob:` URL of an app page; the Monitor's journal export download is allowed and journaled. Window opening and closing, a window that cannot be built and a panic on any host thread (`src/panic_journal.rs`) are journaled too, and navigation refusals stay journaled as before. Every entry is `desktop.ipc.refused`, `desktop.ipc.failed`, `desktop.window.refused`, `desktop.permission.refused`, `desktop.download`, `desktop.download.refused`, `desktop.window.opened`, `desktop.window.closed`, `desktop.window.create`, `desktop.navigation.refused` or `desktop.panic`, with the actor `webview:<label>` where a page acted, an error code and at most an opaque target (a command such as `plugin:fs.read_text_file`, a window label or a permission kind); no payload, address, file name, path, panic message or key. The packaged-window test reads each refusal back from the journal through the API ([DE-S-002, DE-S-003](../../apps/filesquid/crates/desktop/resources/srs.md)). The `tracing` layer relies on Tauri's span and event names (`ipc::request::handle`, `ipc::request::response`, the `__TAURI_INVOKE_KEY__` message on `tauri::webview`) and the gate on Tauri leaving the `ipc` scheme to the app; both are pinned by the integration and packaged-window tests, which a Tauri upgrade must pass.

## Context / Requirement Reference

The product MUST look and work as one application on Linux, macOS and Windows, with Linux desktop and independent web first and macOS/Windows at maturity ([FE-017](../product-vision.md#cross-platform-access), [FE-064](../product-vision.md#desktop-host-isolation), ImageSquid C-036). It MUST reuse the web interface for the independent browser app ([T-002](../product-vision.md#client-processing-and-hosted-mailbox)). Native file access, workers and OS adapters MUST stay out of the untrusted webview ([U-18](../product-vision.md#one-api-hosts-and-distributions)).

The owner accepted Tauri's webview differences: function parity is mandatory; pixel-identical screenshots are not.

## Decision

The installed desktop application is **Tauri 2**. The Svelte UI runs in the operating-system webview (WebView2 on Windows, WKWebView on macOS, WebKitGTK on Linux). The UI talks to the Rust host through Tauri commands and events.

The desktop **MUST NOT** run a local Phoenix, Axum, or other HTTP server for the private UI. IPC is the desktop connection; Axum is the hosted connection ([ADR 001](./001-shared-rust-core-with-tauri-and-axum-hosts.md), [ADR 004](./004-axum-hosted-ciphertext-mailbox.md)).

Look-and-feel and parity policy:

- One design system and **universal CSS** first.
- Pixel-by-pixel visual match is not a success criterion.
- Per-webview CSS MAY be added only to fix a measured engine bug, and MUST be covered by fixtures.
- **Mandatory common functionality** is the intersection of product requirements that ImageSquid C-036 does not mark as a named platform adapter or capture-dependent exception. Those common functions MUST pass on every supported desktop OS. A required feature MUST NOT be labeled unsupported on every platform to satisfy a gate ([FE-017](../product-vision.md#cross-platform-access)).
- **Permitted exceptions** (PhotoKit, OS share sheets, specific codecs) remain explicit, tested, and visible as capability states. Visible unsupported states apply only to those declared exceptions ([C-001](../product-vision.md#platforms-originals-and-service-limits)).
- Linux WebKitGTK, macOS WKWebView and Windows WebView2 each receive the same functional collection-browse, keyboard, and declared-format tests.

The webview receives opaque handles and allowed commands, not arbitrary filesystem paths or SQL.

**Minimum IPC security policy** (capability files are later; this policy is not):

- Commands are **deny-by-default**. Registered custom commands MUST NOT be available to every window unless an explicit capability grants them ([Tauri capabilities](https://v2.tauri.app/security/capabilities/)).
- Each window/webview has an explicit allowlist of commands.
- Rust MUST authorize the caller and validate opaque handles before touching files, SQL or keys.
- Remote or untrusted pages MUST NOT inherit privileged IPC.

## Rationale

Tauri hosts the same Svelte UI the browser uses, without bundling Chromium (Electron) and without a second Flutter UI. Rust in-process owns files, SQLite, OCFL and workers, which is the FileSquid U-11 path. Skipping a local web server avoids a second network hop and a second process on the owner's machine.

Universal CSS plus honest capability states is the practical reading of "works the same" given three webviews.

## Assumptions

- WebView2, WKWebView and WebKitGTK on supported OS versions can run the Svelte SPA and the **mandatory common** media APIs. Declared exceptions use visible unsupported states; they cannot empty the required set.
- Linux-first validation will catch the weakest webview before macOS/Windows CSS assumptions accumulate.
- Custom window chrome MAY be used later so title bars match; native file dialogs MAY still look native.
- Tauri 2.x is used; Tauri 3 remains alpha at writing and is not adopted.

## Alternatives Considered

- **Electron:** Ships Chromium, which usually improves CSS/codec consistency and usually raises installer size and idle RAM. Pixel identity is not guaranteed, and FileSquid memory cost is unmeasured. Rejected for expected resource cost, Node privilege surface, and a bundled browser the product does not need.
- **Flutter:** Can ship desktop and Flutter web. Rejected because it does not match the selected Svelte/DOM interface; a second UI would break the one-client rule ([ADR 003](./003-svelte-client-rendered-ui.md)). [Flutter web](https://docs.flutter.dev/platform-integration/web)
- **Qt or per-OS native UI:** Strong local I/O; rejected because they duplicate the web UI.
- **Wails:** Same webview model with Go. Rejected; Rust is the systems language.
- **Local Axum inside the desktop:** Redundant with Tauri IPC and would look like a server the user did not ask to run.

## Consequences / Implications

Easier: one UI; small host; native Rust workers. Harder: codec/canvas/color/input must be tested on three engines; Linux WebKitGTK is the riskiest. Exact capability JSON, signing and updater trust remain implementation ADRs, but they MUST implement the deny-by-default policy above. Platform adapters such as PhotoKit stay macOS-bound.

## Related Decisions / Notes

- [ADR 001](./001-shared-rust-core-with-tauri-and-axum-hosts.md), [ADR 003](./003-svelte-client-rendered-ui.md), [ADR 006](./006-libvips-native-stills-not-imagemagick.md), [ADR 007](./007-durable-jobs-and-interactive-performance.md)
- [Tauri 2 process model](https://v2.tauri.app/concept/process-model/), [Tauri capabilities](https://v2.tauri.app/security/capabilities/)
