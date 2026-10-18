![FileSquid Logo](./resources/assets/FileSquid-logo.png)

# About this project

**FileSquid** is a free, open-source file platform that keeps your files in an archival standard you own, with descriptions and provenance attached, on your own computer, on your own server, or in an encrypted cloud you can leave at any time ([product vision](./resources/product-vision.md)).

It is designed to look and work like a familiar cloud drive: folders, list and grid views. Underneath, every file is stored the way archives store them:

- **OCFL 1.1** objects on disk or S3-compatible storage: versioned, checksummed, readable without FileSquid, and enough to rebuild the whole application if its database is lost (`ocfl` module).
- **Metadata** in a human-readable `metadata.md` record using ISO 15836-2 Dublin Core `/terms/` (`dcterms:`), with Schema.org for anything Dublin Core cannot say and, once the planned namespace registry ships, any additional schema you register (for example RiC-O, or your own key:value namespace), next to the embedded EXIF, ID3, IPTC and XMP (`metadata` module).
- **C2PA Content Credentials** that start at import and record what was done to a file and whether AI was involved (`c2pa` module; ships with the second consumer release, not the MVP or the first release, [ADR 037](./resources/architecture/037-c2pa-in-release-2.md)).

FileSquid is one Rust backend with **one API**. Every user interface is plain HTML, CSS and TypeScript that calls that API. A **distribution** puts views, a theme, a vocabulary and a module set on top of the same backend:

| Distribution | Audience | Release |
|---|---|---|
| **FileSquid** (default) | Anyone who wants a private, durable cloud drive | First consumer release, preceded by an early-access macOS desktop MVP ([ADR 035](./resources/architecture/035-macos-first-desktop-mvp.md)) |
| **ImageSquid** | Consumers and families managing photos and videos | Second consumer release, together with FileSquid's Content Credentials (C2PA) |
| **Arkrim** | Digital archivists and records managers | Later |

FileSquid runs as a **desktop app** (Tauri 2; the API is served in-process with no network port), as a **multi-user `filesquid-server`** that you or your organisation operate, and later as an **encrypted browser host**. The paid offer is encrypted **backup and/or sync** at `*.filesquid.tech`: the service stores ciphertext only and never holds your keys. Next come version-compliant batch curation jobs (for example transcoding, Parquet datasets and FAIR reporting), with Arkrim as their UI ([ADR 029](./resources/architecture/029-filesquid-unified-product-and-distributions.md), [ADR 032](./resources/architecture/032-hosting-and-privacy-model.md)).


# Setup Development Environment

## Native prerequisites (platform-agnostic)

Install a **stable** Rust toolchain with [rustup](https://rustup.rs/) (the workspace's `rust-toolchain.toml` adds `rustfmt` and `clippy`), then add the browser target once with `rustup target add wasm32-unknown-unknown` (the `filesquid-ocfl` tests build that crate for it). Install **Node.js** 22.12.0 or newer (Node.js 24 LTS is simplest, since it bundles npm 11) with a version manager such as [mise](https://mise.jdx.dev/) or [asdf](https://asdf-vm.com/), or the installer from [nodejs.org](https://nodejs.org/). Use **npm 11 or newer** (bundled with Node.js 24, or `npm install -g npm@11` on Node.js 22; do not use `npm@latest`, because npm 12 requires Node.js 22.22.2 or newer): npm 10 crashes with `Cannot read properties of null (reading 'edgesOut')` when it resolves the relative-path link from ImageSquid to `@filesquid/ui`. Both packages declare the requirement in `engines` and enforce it with `engine-strict`, so an old npm stops with a clear message instead. Desktop builds need the OS webview toolchain described in [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/): MSVC + WebView2 on Windows, Xcode Command Line Tools on macOS, WebKitGTK 4.1 on Linux. Docker (optional) runs the Compose services.

Language manifests live **inside** `apps/`, never at the repository root:

| Path | What it is |
|---|---|
| `apps/filesquid/` | Cargo workspace: `crates/{core,ocfl,metadata,c2pa,cli,api,native,server,desktop}` |
| `apps/filesquid/ui/` | `@filesquid/ui`: API client, custom elements, design tokens and the default FileSquid UI (vanilla TypeScript, Vite; ports 5173 dev / 4173 preview) |
| `apps/imagesquid/` | The ImageSquid distribution: views, tokens, labels, brand; no backend code (ports 5174 / 4174) |
| `scripts/docs/` | Documentation validators (requirement coverage, PRD allocation, links, ADR structure) |

Install the UI packages once (ImageSquid links `@filesquid/ui` by relative path, so install it first):

```sh
cd apps/filesquid/ui && npm ci
cd ../../imagesquid && npm ci
```

## Project naming

Use **FileSquid** for displayed branding of the product and **filesquid** for technical identifiers (Cargo packages `filesquid-core`, `filesquid-ocfl`, `filesquid-metadata`, `filesquid-c2pa`, `filesquid-cli`, `filesquid-api`, `filesquid-native`, `filesquid-server`, `filesquid-desktop`; the one command-line tool is `fsquid`; npm scope `@filesquid`; desktop identifier `tech.filesquid.desktop`; hosted service domain `filesquid.tech`). **ImageSquid** is a distribution: displayed brand ImageSquid, npm package `@imagesquid/ui`, desktop identifier `app.imagesquid.desktop`, domain `imagesquid.app`. Internal code, schemas and APIs use FileSquid's internal names (repository, library, collection, virtual collection, virtual exhibit, file); a distribution changes only its UI labels ([terminology](./resources/product-vision.md#terminology)). The `ocfl`, `metadata` and `c2pa` crates stay independently installable and publishable for programmes that never run the FileSquid app.

## Run the application

**Command line.** Every FileSquid CLI command runs through one tool, `fsquid`, built by `filesquid-cli` ([ADR 036](./resources/architecture/036-single-fsquid-cli.md)). From `apps/filesquid`:

```sh
cargo run -p filesquid-cli -- --help
cargo run -p filesquid-cli -- metadata validate dcterms:title
```

A command that only one module defines runs as `fsquid <command>`; any command also runs as `fsquid <module> <command>`; `validate` always needs the module name.

**Server host** (API plus the built default UI), from `apps/filesquid`:

```sh
cd apps/filesquid/ui && npm run build && cd ..
FILESQUID_BIND=127.0.0.1:8083 FILESQUID_UI_DIR=ui/dist cargo run -p filesquid-server
```

Open `http://127.0.0.1:8083`. Without `FILESQUID_UI_DIR` the server serves the API only. `FILESQUID_BIND` defaults to `127.0.0.1:8083`.

`FILESQUID_OPERATOR_NAME` names who runs the server, for example `FILESQUID_OPERATOR_NAME="Garderen Family"`. A server's operator can read the files it stores, so the name belongs in the UI's privacy disclosure; the host descriptor reports it now, and the default UI does not display it yet (its host privacy indicator is a later task). Surrounding whitespace is trimmed, and the name must be 1 to 256 characters with no control characters; a longer name, one with a control character, or a value that is not valid Unicode stops the server with an error that names the variable but not the value. When it is unset or blank, the server still starts, reports no operator name, and writes one `server.operator_unnamed` warning to stdout (whatever `RUST_LOG` says) and to the activity journal at every start. Any authenticated caller (any accepted credential, the administrator token included) can read the host descriptor (host type, operator name and hosted backup and sync state) at `GET /api/v1/host`.

Every API route except the liveness probe, plus the WebSocket event channel and `/metrics`, is **deny-by-default**: a call without the token answers `401 unauthorized`, and one with a wrong token `403 forbidden`, with a typed error body. HTTP requests, `/metrics` included, send the token as `Authorization: Bearer <token>`. The WebSocket event channel accepts that header too, but a browser cannot set headers on a WebSocket, so it offers the subprotocols `filesquid.events.v1` and `filesquid.bearer.<token>` instead. The liveness probe `GET /api/v1/health/live` is public for process supervisors and load balancers and answers only `{"status":"up"}`; the static UI is public too.

Until server accounts arrive, the token is the administrator token: the value of `FILESQUID_ADMIN_TOKEN` (surrounding whitespace is trimmed and a blank value counts as unset; any other value must be 64 hexadecimal characters, 256 bits, like a generated token, and the server refuses to start otherwise, never printing the value), or, on Linux and macOS only, the token generated at first start into `admin-token` in the data directory (`FILESQUID_DATA_DIR`, default `filesquid-data`). On Windows the server stores no token file and refuses to start until `FILESQUID_ADMIN_TOKEN` is set. On Linux and macOS the server refuses to start when the token file is unsafe, and names the case:

- **Readable by group or others:** `chmod 600 "$FILESQUID_DATA_DIR/admin-token"`, then restart.
- **Owned by another user:** `chown <server user> "$FILESQUID_DATA_DIR/admin-token"` (as an administrator), or delete the file and restart to generate a new token.
- **A symbolic link:** delete the link and restart to generate a new token, or replace it with a regular file that holds the token and `chmod 600` it.
- **Not a regular file** (for example a directory or a FIFO): remove it and restart to generate a new token.

In a second terminal in `apps/filesquid` (with the default data directory):

```sh
curl http://127.0.0.1:8083/api/v1/health/live
TOKEN=$(cat filesquid-data/admin-token)
curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8083/api/v1/health
curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8083/api/v1/host
```

The full health route returns `{"schema_version":1,"status":"ok","product":"filesquid"}`. The host route returns `{"schema_version":1,"host_type":"server","operator_name":null,"hosted_service":{"enrolled":false,"backup_enabled":false,"sync_enabled":false,"service_host":null}}` until a non-blank, valid `FILESQUID_OPERATOR_NAME` is set; encrypted backup and sync are reported as not enrolled until they exist.

On Windows, set `FILESQUID_ADMIN_TOKEN` yourself (the server stores no token file there) and reuse the same value for requests. In PowerShell 7, from `apps/filesquid`:

```powershell
Push-Location ui; npm ci; npm run build; Pop-Location
$TOKEN = -join (1..32 | ForEach-Object { '{0:x2}' -f [System.Security.Cryptography.RandomNumberGenerator]::GetInt32(256) })
$TOKEN                                   # note it; the server never prints it
$env:FILESQUID_ADMIN_TOKEN = $TOKEN
$env:FILESQUID_UI_DIR = "ui/dist"
cargo run -p filesquid-server
```

In a second PowerShell window, set `$TOKEN` to the same value, then:

```powershell
curl.exe http://127.0.0.1:8083/api/v1/health/live
curl.exe -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8083/api/v1/health
```

The same `FILESQUID_ADMIN_TOKEN` flow works on Linux and macOS, where it takes precedence over the token file. Generate a well-formed token there with:

```sh
export FILESQUID_ADMIN_TOKEN=$(openssl rand -hex 32)
TOKEN=$FILESQUID_ADMIN_TOKEN    # for the curl examples
```

The desktop host needs no token from you: it generates a new per-launch token at every start and gives it only to its own pages. In the browser UI served by `filesquid-server`, enter the token on the **Monitor** page (`#/monitor`); how `@filesquid/ui` sends it is in its [README](./apps/filesquid/ui/README.md#authenticating-to-the-filesquid-api).

**UI development** with hot reload: start the server as above, then in another terminal:

```sh
cd apps/filesquid/ui && npm run dev      # FileSquid at http://127.0.0.1:5173
cd apps/imagesquid && npm run dev        # ImageSquid at http://127.0.0.1:5174
```

Both dev servers proxy `/api` (including the WebSocket event channel) to `FILESQUID_API_URL`, default `http://127.0.0.1:8083`.

**Desktop host** (native webview; not a Compose service). A debug build without installers:

```sh
cd apps/filesquid/ui && npm run desktop:build -- --debug --no-bundle    # FileSquid
cd apps/imagesquid && npm run desktop:build -- --debug --no-bundle      # ImageSquid (Tauri config overlay)
```

Both produce `apps/filesquid/target/debug/filesquid-desktop`, branded by the overlay. Installers stay disabled (`bundle.active` is false) until the [ADR 011](./resources/architecture/011-native-media-adapters-and-ffmpeg-license.md) licence review.

## Run tests

This repository has no hosted CI. The gate before every push is the local pre-commit hook, which runs every verifier under `scripts/` (branch protection and the `<type>/<slug>` branch-name prefix, ports, document sync, traceability, links, workspace boundaries, logging, formatting, documentation checks, then Clippy, Cargo and UI tests). Run it by hand from the repository root or install it once as the git hook:

```sh
bash scripts/pre-commit.sh                     # run all gates now
ln -s ../../scripts/pre-commit.sh .git/hooks/pre-commit   # or install as the git pre-commit hook
```

`FILESQUID_SKIP_TESTS=1 git commit ...` skips the lint and test step for a work-in-progress commit; run `bash scripts/verify_tests.sh` before pushing. `FILESQUID_E2E=1` adds the Playwright end-to-end suites, and `FILESQUID_DESKTOP_E2E=1` the desktop packaged-window suite. The individual commands the hook runs are listed below.

Rust workspace (lint first; the cucumber BDD features in each crate's `tests/features/` run as part of `cargo test` and write `tests/allure-results/cucumber-junit.xml` per crate):

```sh
cd apps/filesquid
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

One module on its own, for example `cargo test -p filesquid-ocfl --locked`. That crate's BDD features also build it for the browser (`cargo build -p filesquid-ocfl --locked --no-default-features --target wasm32-unknown-unknown`, in a separate target directory under Cargo's test temp dir), so `cargo test` fails if the `wasm32-unknown-unknown` target is missing or the crate stops building for it. Its OCFL 1.1 community fixture tests (`integration_fixture_conformance`, which validates every good, warning and bad fixture object, `integration_fixture_inventories`, `integration_fixture_reads` and the `@fixtures` BDD feature) read the fixtures committed under the crate's `tests/fixtures/ocfl/1.1` (`OCFL/fixtures` commit `d323fc207126becec066998b43408d998bf0fb1b`, CC-BY-4.0), so they run offline in every `cargo test` and in the hook, and fail on any conformance failure. `FILESQUID_OCFL_FIXTURES` may name another `1.1` tree, for example a checkout at a new commit before the pin moves; see the [module README](./apps/filesquid/crates/ocfl/README.md#pinned-ocfl-community-fixtures). Its write conformance test (`integration_content_fixtures`, and the write scenario of the `@fixtures` feature) writes every `content` fixture through the engine and also runs the independent community validators `ocfl-validate.py` (from `ocfl-py`, `python3 -m pip install ocfl-py`) and `rocfl` (`cargo install rocfl --locked`) when they are on `PATH` and executable, or the one program named in `FILESQUID_OCFL_COMMUNITY_VALIDATOR` (on Windows the search also tries `PATHEXT` suffixes such as `rocfl.exe`, but does not run a bare `ocfl-validate.py` script, so use `rocfl` or name the `.exe` in that variable). They are optional test oracles: without one the community check prints `SKIPPED community OCFL validation` and the engine checks still run, unless `FILESQUID_OCFL_REQUIRE_COMMUNITY_VALIDATOR=1` makes that a failure. The validators add about 3 seconds; see [Community validators](./apps/filesquid/crates/ocfl/README.md#community-validators-test-oracles). `integration_opaque_bytes` writes the same fixtures sealed by a test-only caller transform and runs the same validators over them ([Opaque bytes](./apps/filesquid/crates/ocfl/README.md#opaque-bytes)).

`cargo test -p filesquid-c2pa --locked` (also part of `cargo test --workspace`) builds the C2PA module for `wasm32-unknown-unknown`, with default features on and off, into `filesquid-c2pa-wasm32-check/` under the Cargo target directory. Install the target once with `rustup target add wasm32-unknown-unknown`; without it the check fails, unless `FILESQUID_SKIP_WASM32=1` is set to skip it explicitly (the skip is printed). The same run executes the `c2patool` oracle harness on the C2PA conformance files when `FILESQUID_C2PA_CONFORMANCE_DIR` holds the absolute path of a checkout of `c2pa-org/public-testfiles` (`FILESQUID_C2PATOOL` names the tool if it is not on `PATH`); otherwise that test is skipped and prints why. Until the engine reads manifests (CP-0.2 to CP-0.5) the harness only records `c2patool`'s verdicts and counts every file as pending; it does not yet compare the engine with the oracle. The oracle run takes about 10 seconds; see the [module README](./apps/filesquid/crates/c2pa/README.md#interoperability-oracle).

FileSquid UI and the ImageSquid distribution (unit and DOM tests in jsdom; end-to-end tests run Playwright against the **built** UI served by the real `filesquid-server`). Install the Playwright browser once with `npx playwright install chromium`:

```sh
cd apps/filesquid/ui
npx prettier --check . && npm run lint && npm run check && npm test && npm run test:e2e
cd ../../imagesquid
npx prettier --check . && npm run lint && npm run check && npm test && npm run test:e2e
```

`npm test` clears and rewrites `tests/allure-results/vitest-junit.xml`; `npm run test:e2e` writes `tests/allure-results/playwright-junit.xml`. Open the JUnit files in any JUnit or Allure viewer.

The API contract (`apps/filesquid/crates/api/openapi.json`, OpenAPI 3.1, versioned by `info.version` = `filesquid_api::API_VERSION`), the Rust routes and payload types, and the generated TypeScript types (`apps/filesquid/ui/src/api/schema.d.ts`, `openapi-typescript` 7.13) must agree. The Rust `unit_contract` test fails when a routed path is undocumented (or the reverse), when a serialized Rust type no longer matches its component schema, or when an event topic or the payload a publisher emits drifts from the `EventTopic` enum and `x-filesquid-event-payloads` map. The Vitest `contract.test.ts` fails when `schema.d.ts` differs from a fresh generation. Both run in the pre-commit hook. After changing the contract, regenerate the types and run both drift tests:

```sh
cd apps/filesquid/ui
npm run contract:types
npx vitest run tests/unit/contract.test.ts
cd ..
cargo test -p filesquid-api --locked --test unit_contract --test unit_errors
```

The event channel's limits (256 buffered events per subscriber, 65,536-byte events, 64 subscriptions) are tested at their boundaries, together with the routes answering the same through every host's address form and the BDD scenarios in `crates/api/tests/features/`:

```sh
cd apps/filesquid
cargo test -p filesquid-api --locked --test unit_events --test unit_routes --test features
```

Deny-by-default authorisation (API-S-004) is tested on every route, the 404/405 fallbacks and the host routes: no token, a wrong token, no installed authoriser, a principal the policy does not permit, the WebSocket subprotocol carrier, the journaled refusals and the desktop per-launch token through the custom scheme:

```sh
cd apps/filesquid
cargo test -p filesquid-api --locked --test unit_auth --test features
cargo test -p filesquid-server --locked --test integration_auth
cargo test -p filesquid-desktop --locked --test integration_api_protocol
```

The host descriptor (API-S-020, `GET /api/v1/host` and the `host.change` event) is tested through the router on every host type, with spoofed request headers, the operator-name bounds, the not-enrolled default and redaction of the answer, the event and the `server.operator_unnamed` entry. The `@server-start` scenarios of `crates/api/tests/features/host_descriptor_API-S-020.feature` run in the server's own cucumber harness, which starts the server in-process and runs the real `filesquid-server` binary for the refusal to start (JUnit in `crates/server/tests/allure-results/`):

```sh
cd apps/filesquid
cargo test -p filesquid-api --locked --test unit_host --test features
cargo test -p filesquid-server --locked --test integration_host --test features
cargo test -p filesquid-desktop --locked --test integration_api_protocol
```

The application-layer boundary (API-S-006) is guarded twice, and both guards must find nothing. `unit_isolation` runs `cargo tree` for `filesquid-api` with the default, no-default and all-features sets and fails on Tauri, libvips, FFmpeg, a SQLite driver, `fs4`, `filesquid-native`, a network client or an async runtime other than Tokio. It also scans every file in `crates/api/src/`, with comments and string literals removed, for `std::fs`, `tokio::fs`, `fs4`, the OS file-system extensions, file handles and network clients, including grouped `use std::{...}` imports that span lines:

```sh
cd apps/filesquid
cargo test -p filesquid-api --locked --test unit_isolation
```

The library domain commands and queries (API-S-019: create and open a library, collections, virtual collections, membership, hide and recoverable delete, permanent deletion with a scope confirmation, paged listing) run against the in-memory reference catalog, so they need no database:

```sh
cd apps/filesquid
cargo test -p filesquid-api --locked --test unit_library --test unit_library_queries --test features
```

The host-side I/O the API must not do lives in the native-processing library `filesquid-native` (`crates/native`): the SQLite journal store, the data-directory checks (write probe, free space) and `HostMonitor`, which both hosts link ([ADR 033](./resources/architecture/033-activity-journal-and-live-health-monitoring.md) amendment 2026-10-18). Its unit tests and BDD features run alone:

```sh
cd apps/filesquid
cargo test -p filesquid-native --locked
```

The desktop transport (DE-S-005): the event channel's forwarder (one counted subscription, resync hints, the `event-forwarder` process, navigation kept on the app's origins), ranged reads through the `filesquid://localhost` scheme (`http://filesquid.localhost` on Windows and Android) with its 16 MiB response cap, and typed host errors. The 100 MB range fixture and the BDD features in `crates/desktop/tests/features/` exist only in the test-instrumented build (`--features test-webdriver`, never shipped), so the hook runs the desktop crate a second time with that feature:

```sh
cd apps/filesquid
cargo test -p filesquid-desktop --locked --test integration_event_channel
cargo clippy -p filesquid-desktop --all-targets --features test-webdriver --locked -- -D warnings
cargo test -p filesquid-desktop --features test-webdriver --locked
```

Packaged-window tests (ADR 017): WebdriverIO with `@wdio/tauri-service` drives the test-instrumented desktop binary through its embedded WebDriver server (on `127.0.0.1`, port `TAURI_WEBDRIVER_PORT`, default 4445): the per-launch token, a 100 MB `Range` read through the scheme, an event subscription, and a refused navigation to a foreign site. Build the binary, then run the suite from `apps/filesquid/crates/desktop/tests/e2e`. On Linux the webview is WebKitGTK and a headless machine needs Xvfb (`xvfb-run`, from your distribution's `xvfb` package); on macOS and Windows run `npm run test:e2e` without it:

```sh
cd apps/filesquid
cargo build -p filesquid-desktop --features test-webdriver --locked
cd crates/desktop/tests/e2e
npm ci
npm run test:e2e                  # macOS, Windows, or Linux with a display
# headless Linux instead: xvfb-run -a npm run test:e2e
```

The suite drives `<target>/debug/filesquid-desktop`, where `<target>` is `CARGO_TARGET_DIR` (a relative value is resolved against `apps/filesquid`, where the build ran) or `apps/filesquid/target`; set `FILESQUID_DESKTOP_BIN` to drive another binary. Before launching, it copies the binary into a fresh temporary directory, so a concurrent Cargo build cannot replace it mid-run, and removes that directory afterwards. On Linux each run uses a fresh temporary `XDG_DATA_HOME`, so the tests never write into your own FileSquid data. `npm run test:e2e` clears and rewrites `apps/filesquid/crates/desktop/tests/allure-results/wdio/` (JUnit). `FILESQUID_DESKTOP_E2E=1 bash scripts/verify_tests.sh` builds the binary and runs the suite as part of the hook (with `xvfb-run` on Linux when no display is set).

Documentation checks, from the repository root:

```sh
python3 scripts/docs/check_prd_allocation.py        # every vision ID allocated in the PRD
python3 scripts/docs/check_links.py                 # relative links and anchors resolve
python3 scripts/docs/check_distribution_links.py    # no link crosses the FileSquid/ImageSquid boundary
node --test scripts/docs/validate-adrs.test.mjs    # ADR structure (ISO 42010)
```

## View logs

Every FileSquid process writes JSON events to **stdout** and never writes log files ([ADR 014](./resources/architecture/014-stdout-observability.md)). `filesquid-server` and `filesquid-desktop` print JSON lines with UTC timestamps to the terminal that started them; set `RUST_LOG` (for example `RUST_LOG=debug`) to change the level.

FileSquid's own tooling records **100% of activity** in its activity journal and shows **live health** of the system and its processes in the **Monitor** dashboard, on desktop and server alike ([ADR 033](./resources/architecture/033-activity-journal-and-live-health-monitoring.md)). Everything runs locally; no external service is needed.

- **Where the journal lives:** `filesquid.sqlite` in the host's data directory. The desktop app uses its application data directory. `filesquid-server` uses `FILESQUID_DATA_DIR` (default `./filesquid-data`, relative to where it starts) and creates it on first start.
- **Monitor dashboard:** open the app and choose **Monitor** in the navigation (route `#/monitor`). On the desktop the owner needs no sign-in: the host hands its own pages the per-launch token. On `filesquid-server`, enter the admin token: the value of `FILESQUID_ADMIN_TOKEN`, or the token generated at first start into `<data dir>/admin-token` (owner-only file permissions on Unix; the server logs the path, never the token). Until then the shell's API status reads **Missing credentials**; a wrong token reads **Invalid credentials**. The UI keeps the token in memory only, so enter it again after a reload ([`@filesquid/ui` README](./apps/filesquid/ui/README.md#authenticating-to-the-filesquid-api)).

To run `filesquid-server` with the default UI and read the journal from a terminal, change to `apps/filesquid` and run:

```sh
(cd ui && npm run build)
FILESQUID_UI_DIR=ui/dist cargo run -p filesquid-server
```

Then open `http://127.0.0.1:8083/#/monitor`. In a second terminal in `apps/filesquid`:

```sh
TOKEN=$(cat filesquid-data/admin-token)
curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8083/api/v1/admin/health
curl -H "Authorization: Bearer $TOKEN" "http://127.0.0.1:8083/api/v1/admin/journal?order=desc&limit=20"
curl -H "Authorization: Bearer $TOKEN" "http://127.0.0.1:8083/api/v1/admin/journal/export?operation=host.started" > journal.jsonl
curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8083/metrics
```

On Windows there is no token file: start the server with `FILESQUID_ADMIN_TOKEN` set as shown under **Server host**, set `$TOKEN` to the same value in PowerShell, and call `curl.exe` with the same arguments.

---

# License

FileSquid and the ImageSquid distribution are licensed under the [MIT License](./LICENSE). Copyright (c) 2026 Pieter Van Garderen. See [COPYRIGHT](./COPYRIGHT). 

# Open Agent Dev

This project is developed with Claude Code and Grok Build using the [Open Agent Dev](https://github.com/peterVG/open-agent-dev/) tooling developed by [@peterVG](https://vangarderen.net)
