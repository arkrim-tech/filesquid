# Task API-F1.3 Conflict Artifact

Recorded during [Task API-F1.3](../plan.md#phase-f1-the-api-global-tasks-fs-11fs-14-after-the-mvp-api-f18-in-fs-15-api-f19-in-fs-310-and-api-f15-in-fs-c1) (file operation routes and content handles) for owner review. Two points need an owner decision; a third is a follow-up for the modules engineer.

## 1. ADR 030 S1 item (a): a `<video>` does not play from the custom scheme on WebKitGTK

### Conflict Description

Acceptance gate (2) of API-F1.3 requires `Range` reads of a 100 MB file into `<img>` and `<video>` through content handles on WebKitGTK, WKWebView and WebView2, with a seek and a playback that outlasts the 300 s idle window. Measured in a packaged WebKitGTK window (Linux, Xvfb, WebKitGTK 2.52; `tests/e2e/specs/content-handles.e2e.mjs`):

- pass: minting with the per-launch token; `HEAD`, a suffix range (`206`) and a range past the end (`416`, `bytes */<length>`) of a 105 MB VP8 WebM through the handle with no token; the `403` denial of an unknown handle; an `<img>` through a handle;
- fail: a `<video>` with `src="filesquid://localhost/api/v1/content/<handle>"`. WebKitGTK sends one probe request (`Range: bytes=0-1445`), receives a correct `206` (`video/webm`, `Content-Range: bytes 0-1445/<length>`), and then reports `MEDIA_ERR_SRC_NOT_SUPPORTED` (`networkState` `NETWORK_NO_SOURCE`) without another request. The same bytes play from a `blob:` URL, and an answer without the privacy headers fails the same way. This is consistent with WebKitGTK's GStreamer source taking only `http(s)` and `blob` URIs, not custom schemes.

The seek and the playback past the idle window could therefore not be measured on WebKitGTK. WKWebView and WebView2 need macOS and Windows machines, which this environment does not have. The suite pins the WebKitGTK measurement in a test of its own, so it fails once WebKitGTK plays; the full playback test runs on macOS and Windows.

### PRD/SRS Reference

- [plan.md](../plan.md), Task API-F1.3, acceptance gate (2): "... on WebKitGTK, WKWebView and WebView2, including a seek and a playback that outlasts the 300 s idle window, MUST pass and be recorded in ADR 030."
- [API-S-018](../srs.md): media elements MUST read file bytes through content handles; [DE-S-005](../../../desktop/resources/srs.md): the desktop opens no TCP port for the API.

### ADR Reference

- [ADR 030](../../../../../../resources/architecture/030-filesquid-api-and-transports.md), S1 results, row (a): "This gate MUST pass before the F1 file-content endpoints are accepted. If it fails on a webview, the loopback-port alternative with a per-launch token is re-evaluated for media only, and this ADR is amended." The row now records the measurement.
- [ADR 002](../../../../../../resources/architecture/002-tauri2-desktop-shell.md): the desktop opens no TCP listener.

### Recommended Resolution

Option A (recommended): on Linux, a loopback listener on `127.0.0.1` with a random port, serving only `GET` and `HEAD /api/v1/content/{handle}` through the same router, with the content handle as its only credential (handles are 256-bit, short-lived and tied to the minting credential, which is what makes a listener acceptable for media only). It amends ADR 002 and T-049 for media on Linux. The UI asks the API for the media base URL, so pages never build it themselves.

Option B: a MediaSource Extensions player in the UI on WebKitGTK: the page fetches ranges through the content handle and appends them to a `SourceBuffer`. No port is opened, but each container format needs its own demuxing rules, and the UI owns seeking.

Option C: accept API-F1.3 for the API, keep the gate open as its own task, and decide between A and B there.

In every option, the same suite must also pass on macOS (WKWebView) and Windows (WebView2) before the gate closes.

Artifacts to update: ADR 030 (S1 (a) row and the decision), ADR 002 and T-049 (option A), DE-S-005, [plan.md](../plan.md) (API-F1.3 box and Status line, or a new task), [resources/implementation-plan-checklist.md](../../../../../../resources/implementation-plan-checklist.md), the UI preview components (UI-), and the e2e spec.

### Impact Assessment

Until it is resolved, video previews cannot play in the Linux desktop app; images, downloads and every ranged read work. On macOS and Windows playback is unproven.

## 2. API-S-018 names "the file's stored media type", but nothing stores one yet

### Conflict Description

API-S-018 requires every successful content answer to carry "the file's stored media type". OCFL stores no media type, and the file record that will hold the media type the client gives at import arrives with MD-0.8 and API-F1.4 (the upload route "gains its file record" there). API-F1.3 therefore derives the type from the logical path's extension through a fixed table (`crates/api/src/files/media.rs`), with `application/octet-stream` for anything else. It never sniffs the bytes, and every content answer carries `X-Content-Type-Options: nosniff` and `Content-Security-Policy: default-src 'none'; sandbox`.

### PRD/SRS Reference

- [API-S-018](../srs.md): "a success MUST also carry `Accept-Ranges: bytes` and the file's stored media type."
- [OCFL-S-012](../../../ocfl/resources/srs.md): "an extension-based type label is a display hint only."

### ADR Reference

- [ADR 030](../../../../../../resources/architecture/030-filesquid-api-and-transports.md) Decision 5 does not say where the media type comes from.
- [ADR 038](../../../../../../resources/architecture/038-import-file-record.md): the import file record holds the media type the client provides.

### Recommended Resolution

Accept the extension table until API-F1.4 stores the file record's media type, then switch the content answers to it. Amend API-S-018 to read "the file's media type (from its file record, or until that exists from its extension; never sniffed)".

Artifacts to update: [API-S-018](../srs.md), API-F1.4 in [plan.md](../plan.md) (switch the content answers to the record's type).

### Impact Assessment

A file with a missing or misleading extension is answered as `application/octet-stream` or with the extension's type; with `nosniff` and the sandbox CSP, a wrong type can stop a preview from playing but cannot run content.

## 3. Follow-up for the modules engineer: the filesystem backend's `head` reads the whole file

`FsBackend::head` hashes the whole file for its entity tag, and the view API's `file_details` reads a file's size through `head`. A ranged read needs the length, so each range request of a 100 MB video would hash 100 MB. The API now caches each content's length by its digest (stored content never changes) and calls `head` once per content; `a_file_length_is_read_once_then_cached` tests it. A size-only storage call (for example a `size` method, or a `head` that does not hash) in `filesquid-ocfl` would remove the first full read as well. This is an engine change (Task IO-), so it is reported here rather than made in this branch.
