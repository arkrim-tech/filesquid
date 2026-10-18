# Architecture Decision Record - FileSquid

002-tauri2-desktop-shell.md

## Title

Tauri 2 desktop shell with IPC to Rust and universal CSS

## Status

Accepted

Accepted: 2026-09-16T01:01:07Z. Revised: 2026-09-22T04:34:37Z.

Amended 2026-10-17 by [ADR 035](./035-macos-first-desktop-mvp.md): macOS is the first desktop target and the FileSquid MVP is desktop-only; Linux and Windows desktop follow the MVP. The Linux-first statements below are kept as history.

Amended 2026-10-15 by [ADR 030](./030-filesquid-api-and-transports.md): the UI reaches the one FileSquid API through a Tauri custom URI scheme served in-process, replacing per-feature IPC commands; there is still no TCP listener. Transport spike S1 is complete and accepted; S1 item (a), `Range` media playback through the custom scheme on each desktop webview, remains an open gate before the file-content endpoints are accepted ([ADR 030](./030-filesquid-api-and-transports.md) S1 results). Amended 2026-10-18 by [ADR 030](./030-filesquid-api-and-transports.md) Decision 5 (owner decision): media elements load through short-lived content handles served by the same custom scheme, so media playback still needs no TCP listener. Svelte is replaced by vanilla TypeScript ([ADR 031](./031-vanilla-typescript-ui-and-white-label-theming.md)). The host crate is `filesquid-desktop` at `apps/filesquid/crates/desktop/`.

Amended 2026-10-18 in Task DE-0.1 (capability JSON): the main window's only capability (`capabilities/default.json`) grants `core:event:allow-listen` and `core:event:allow-unlisten` and nothing else. Filesystem and SQLite access stay in the Rust host behind the API authoriser; the webview gets no fs, sql, shell, dialog, http, process or opener permission, no `core:path` (it returns absolute paths), no asset protocol and no remote context. Tauri checks app commands against the capabilities only when the app has an ACL manifest, and `tauri-build` creates one only when the app has at least one permission. So `build.rs` passes `tauri_build::AppManifest` the policy's command list (empty today; each listed command gets generated `allow-`/`deny-` permissions), and `permissions/app-acl.toml` declares one permission that allows nothing, so the manifest exists while there is no command. An unlisted custom command is then refused in the window: in the packaged-window test, `health_ping` went from "Command health_ping not found" to "health_ping not allowed". The policy lives in `src/capability_policy.rs` and its input reader in `src/capability_inputs.rs`. `build.rs` fails the build when any of these breaks the policy: a capability file; the effective configuration (`tauri.conf.json` merged with `tauri.<platform>.conf.json` and the CLI's `TAURI_CONFIG`, as Tauri merges them; windows other than `main` included); a JSON5 or TOML configuration file it cannot read; `Cargo.toml`; a linked non-core Tauri plugin (except `tauri-plugin-wdio-webdriver` in test-instrumented builds); the app ACL file; or a `#[tauri::command]` ([DE-S-002](../../apps/filesquid/crates/desktop/resources/srs.md)). Tauri offers no hook on its access-control refusals, so they are returned to the page but not journaled; the API's own refusals are journaled as before.

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
