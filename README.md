![FileSquid Logo](./resources/assets/FileSquid-logo.png)

# About this project

**FileSquid** is a free, open-source file platform that keeps your files in an archival standard you own, with descriptions and provenance attached, on your own computer, on your own server, or in an encrypted cloud you can leave at any time ([product vision](./resources/product-vision.md)).

It is designed to look and work like a familiar cloud drive: folders, list and grid views. Underneath, every file is stored the way archives store them:

- **OCFL 1.1** objects on disk or S3-compatible storage: versioned, checksummed, readable without FileSquid, and enough to rebuild the whole application if its database is lost (`ocfl` module).
- **Metadata** in a human-readable `metadata.md` record using ISO 15836-2 Dublin Core `/terms/` (`dcterms:`), with Schema.org for anything Dublin Core cannot say and, once the planned namespace registry ships, any additional schema you register (for example RiC-O, or your own key:value namespace), next to the embedded EXIF, ID3, IPTC and XMP (`metadata` module).
- **C2PA Content Credentials** that start at import and record what was done to a file and whether AI was involved (`c2pa` module).

FileSquid is one Rust backend with **one API**. Every user interface is plain HTML, CSS and TypeScript that calls that API. A **distribution** puts views, a theme, a vocabulary and a module set on top of the same backend:

| Distribution | Audience | Release |
|---|---|---|
| **FileSquid** (default) | Anyone who wants a private, durable cloud drive | First consumer release, preceded by an early-access macOS desktop MVP ([ADR 035](./resources/architecture/035-macos-first-desktop-mvp.md)) |
| **ImageSquid** | Consumers and families managing photos and videos | Second consumer release |
| **Arkrim** | Digital archivists and records managers | Later |

FileSquid runs as a **desktop app** (Tauri 2; the API is served in-process with no network port), as a **multi-user `filesquid-server`** that you or your organisation operate, and later as an **encrypted browser host**. The paid offer is encrypted **backup and/or sync** at `*.filesquid.tech`: the service stores ciphertext only and never holds your keys. Next come version-compliant batch curation jobs (for example transcoding, Parquet datasets and FAIR reporting), with Arkrim as their UI ([ADR 029](./resources/architecture/029-filesquid-unified-product-and-distributions.md), [ADR 032](./resources/architecture/032-hosting-and-privacy-model.md)).


# Setup Development Environment

## Native prerequisites (platform-agnostic)

Install a **stable** Rust toolchain with [rustup](https://rustup.rs/) (the workspace's `rust-toolchain.toml` adds `rustfmt` and `clippy`). Install **Node.js** 22.12.0 or newer (Node.js 24 LTS is simplest, since it bundles npm 11) with a version manager such as [mise](https://mise.jdx.dev/) or [asdf](https://asdf-vm.com/), or the installer from [nodejs.org](https://nodejs.org/). Use **npm 11 or newer** (bundled with Node.js 24, or `npm install -g npm@11` on Node.js 22; do not use `npm@latest`, because npm 12 requires Node.js 22.22.2 or newer): npm 10 crashes with `Cannot read properties of null (reading 'edgesOut')` when it resolves the relative-path link from ImageSquid to `@filesquid/ui`. Both packages declare the requirement in `engines` and enforce it with `engine-strict`, so an old npm stops with a clear message instead. Desktop builds need the OS webview toolchain described in [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/): MSVC + WebView2 on Windows, Xcode Command Line Tools on macOS, WebKitGTK 4.1 on Linux. Docker (optional) runs the Compose services.

Language manifests live **inside** `apps/`, never at the repository root:

| Path | What it is |
|---|---|
| `apps/filesquid/` | Cargo workspace: `crates/{core,ocfl,metadata,cli,api,server,desktop}`; `crates/c2pa` is planned (docs and tests only, not yet a workspace member) |
| `apps/filesquid/ui/` | `@filesquid/ui`: API client, custom elements, design tokens and the default FileSquid UI (vanilla TypeScript, Vite; ports 5173 dev / 4173 preview) |
| `apps/imagesquid/` | The ImageSquid distribution: views, tokens, labels, brand; no backend code (ports 5174 / 4174) |
| `scripts/docs/` | Documentation validators (requirement coverage, PRD allocation, links, ADR structure) |

Install the UI packages once (ImageSquid links `@filesquid/ui` by relative path, so install it first):

```sh
cd apps/filesquid/ui && npm ci
cd ../../imagesquid && npm ci
```

## Project naming

Use **FileSquid** for displayed branding of the product and **filesquid** for technical identifiers (Cargo packages `filesquid-core`, `filesquid-ocfl`, `filesquid-metadata`, `filesquid-c2pa`, `filesquid-cli`, `filesquid-api`, `filesquid-server`, `filesquid-desktop`; the one command-line tool is `fsquid`; npm scope `@filesquid`; desktop identifier `tech.filesquid.desktop`; hosted service domain `filesquid.tech`). **ImageSquid** is a distribution: displayed brand ImageSquid, npm package `@imagesquid/ui`, desktop identifier `app.imagesquid.desktop`, domain `imagesquid.app`. Internal code, schemas and APIs use FileSquid's internal names (repository, library, collection, virtual collection, virtual exhibit, file); a distribution changes only its UI labels ([terminology](./resources/product-vision.md#terminology)). The `ocfl`, `metadata` and `c2pa` crates stay independently installable and publishable for programmes that never run the FileSquid app.

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

Open `http://127.0.0.1:8083`. `GET /api/v1/health` returns `{"schema_version":1,"status":"ok","product":"filesquid"}`. Without `FILESQUID_UI_DIR` the server serves the API only. `FILESQUID_BIND` defaults to `127.0.0.1:8083`.

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

`FILESQUID_SKIP_TESTS=1 git commit ...` skips the lint and test step for a work-in-progress commit; run `bash scripts/verify_tests.sh` before pushing. `FILESQUID_E2E=1` adds the Playwright end-to-end suites. The individual commands the hook runs are listed below.

Rust workspace (lint first; the cucumber BDD features in each crate's `tests/features/` run as part of `cargo test` and write `tests/allure-results/cucumber-junit.xml` per crate):

```sh
cd apps/filesquid
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

One module on its own, for example `cargo test -p filesquid-ocfl --locked`.

FileSquid UI and the ImageSquid distribution (unit and DOM tests in jsdom; end-to-end tests run Playwright against the **built** UI served by the real `filesquid-server`). Install the Playwright browser once with `npx playwright install chromium`:

```sh
cd apps/filesquid/ui
npx prettier --check . && npm run lint && npm run check && npm test && npm run test:e2e
cd ../../imagesquid
npx prettier --check . && npm run lint && npm run check && npm test && npm run test:e2e
```

`npm test` clears and rewrites `tests/allure-results/vitest-junit.xml`; `npm run test:e2e` writes `tests/allure-results/playwright-junit.xml`. Open the JUnit files in any JUnit or Allure viewer.

The API contract (`apps/filesquid/crates/api/openapi.json`) and the generated TypeScript types must agree. After changing the contract, regenerate the types with `npm run contract:types` in `apps/filesquid/ui`; the Rust `unit_contract` test and the Vitest `contract.test.ts` fail on drift.

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
- **Monitor dashboard:** open the app and choose **Monitor** in the navigation (route `#/monitor`). On the desktop the owner needs no sign-in. On `filesquid-server`, enter the admin token: the value of `FILESQUID_ADMIN_TOKEN`, or the token generated at first start into `<data dir>/admin-token` (owner-only file permissions on Unix; the server logs the path, never the token).

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
curl http://127.0.0.1:8083/metrics
```

On Windows PowerShell, read the token with `$TOKEN = Get-Content filesquid-data/admin-token` and call `curl.exe` with the same arguments.

---

# License

FileSquid and the ImageSquid distribution are licensed under the [MIT License](./LICENSE). Copyright (c) 2026 Pieter Van Garderen. See [COPYRIGHT](./COPYRIGHT). 

# Open Agent Dev

This project is developed with Claude Code and Grok Build using the [Open Agent Dev](https://github.com/peterVG/open-agent-dev/) tooling developed by [@peterVG](https://vangarderen.net)
