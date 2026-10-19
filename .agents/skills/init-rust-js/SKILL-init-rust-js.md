---
name: Rust/JavaScript Project Initialization
description: Initialize a Rust and TypeScript monorepo as one product-local Cargo workspace of module and host crates (Axum server, Tauri 2 desktop) plus Vite UI packages under apps/, with Cargo, Vitest, Playwright, Allure-compatible JUnit output, and the root Compose file.
allowed-tools:
  - code_interpreter
references:
  - ../../kb/tauri2.md
copyright: Pieter Van Garderen (2026)
license: CC-BY
---

# Rust / JavaScript Project Initialization

This skill guides the AI agent to initialize a **Rust + TypeScript** monorepo. It is the Rust equivalent of the Python/JS and BEAM/Elixir init skills. One Rust product exposes one API; two hosts run it, an **Axum server** and a **Tauri 2 desktop** process; one or more **Vite + TypeScript UI packages** call that API over HTTP (server) or a custom URI scheme (desktop).

**When to use:** The user asks to initialize a Rust project, scaffold Axum and/or Tauri 2, add a crate or a UI distribution, or run this skill. **Do not** run [init-python-js](../init-python-js/SKILL-init-python-js.md) or [init-beam-elixir](../init-beam-elixir/SKILL-init-beam-elixir.md) for this stack.

**Hard constraints (already accepted unless a later ADR supersedes them):**
- Language manifests (`Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `package.json`) MUST live **inside `apps/`**, never at the repository root ([workspace-boundaries.md](../../rules/workspace-boundaries.md)).
- The product is **one product-local Cargo workspace** (`apps/<product>/Cargo.toml`) with member crates under `apps/<product>/crates/` ([ADR 029](../../../resources/architecture/029-filesquid-unified-product-and-distributions.md), amending [ADR 013](../../../resources/architecture/013-cargo-svelte-workspaces.md)). Do not add a root Cargo workspace and do not scatter independent crates directly under `apps/`.
- Stay on **stable Tauri major 2**. Do not adopt Tauri 3 prereleases or unfiltered "Latest" badges. Recheck crates.io/npm and [tauri2.md](../../kb/tauri2.md) before pinning.
- Domain logic lives in Rust module crates behind the API crate. The TypeScript UI talks to the API only. Do not put privileged filesystem, process, or secret operations in the frontend ([src-isolation.md](../../rules/src-isolation.md)).
- Logging is unbuffered structured JSON on **stdout** (`tracing`), plus the product's own activity journal. MUST NOT open log files on disk ([logging-enforce.md](../../rules/logging-enforce.md), [ADR 033](../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md)).
- Docker Compose at the repo root orchestrates backing services, the optional server container and the opt-in telemetry sinks. The Tauri desktop host runs **natively** on the developer OS (system webviews). Do not containerize the desktop crate ([centralized-infrastructure.md](../../rules/centralized-infrastructure.md)).

Keep every file and folder already in the repository root. Do not delete product-vision, PRD, ADRs, or `.agents/`.

**Progressive disclosure:** Before writing Tauri commands, capabilities, plugins, packaging, or updater code, load [`.agents/kb/tauri2.md`](../../kb/tauri2.md) (Agent contract, Release snapshot, and Security model first). Do not invent permission identifiers, Cargo features, configuration fields, or JS import paths.

---

## Step 1: Scaffold the Root Directory Structure

Follow the Rust/JavaScript section in [workspace-boundaries.md](../../rules/workspace-boundaries.md). Treat this project as a **polyglot monorepo** whose deployable apps live inside `apps/`. Rust crates use `src/` (Cargo). The Tauri crate root is `apps/<product>/crates/desktop/` (no `src-tauri/` indirection).

```text
project-root/
├── docker-compose.yml             # Postgres, optional server container, opt-in telemetry sinks
├── .env.example                   # Copied to .env locally; .env is gitignored
├── config/
│   └── victoriametrics/           # Scrape config for the opt-in observability profile
├── data/                          # Sample media (tracked) and gitignored Compose volumes (data/*_data/)
├── scripts/                       # Pre-commit verifiers (scripts/pre-commit.sh) and docs/ validators; no hosted CI
├── apps/
│   ├── README.md
│   ├── <product>/                 # e.g. filesquid/: extractable to its own repository
│   │   ├── Cargo.toml             # [workspace] members = crates/*
│   │   ├── Cargo.lock
│   │   ├── rust-toolchain.toml
│   │   ├── distribution.json      # Default distribution manifest
│   │   ├── resources/
│   │   │   └── ui-mockups/README.md
│   │   ├── crates/
│   │   │   ├── core/              # Portable contracts (no HTTP, no Tauri, no codecs)
│   │   │   ├── <module>/          # One crate per domain module (e.g. ocfl, metadata, c2pa)
│   │   │   ├── api/               # The single API: routes, contract, auth, events; openapi.json
│   │   │   ├── server/            # Axum host; Dockerfile lives here
│   │   │   └── desktop/           # Tauri 2 host: tauri.conf.json, capabilities/, icons/, build.rs
│   │   └── ui/                    # Default UI package (Vite + TypeScript, no framework)
│   └── <distribution>/            # e.g. imagesquid/: UI-only white-label distribution, no Rust
└── README.md
```

Every crate and UI package has the same inner shape:

```text
<crate or package>/
├── Cargo.toml | package.json
├── src/                           # Execution directory (src-isolation.md)
├── resources/                     # Global docs; documentation-only task records in resources/tasks/
│   ├── srs.md                     # Bounded SRS with PRD parent link
│   ├── plan.md                    # Module plan
│   ├── ui-mockups/README.md       # UI packages and distributions only
│   └── tasks/                     # Task Plan / ToDos / Walkthrough files (task-workflow.md)
└── tests/
    ├── unit/
    ├── integration/
    ├── ui/                        # UI packages only (jsdom)
    ├── e2e/                       # Playwright (UI) or WebdriverIO (desktop)
    ├── features/                  # Gherkin
    ├── performance/               # Criterion benches or k6 scripts
    └── allure-results/            # Gitignored JUnit output
```

Use Cargo and the frontend scaffolder **inside** `apps/` (`cargo new --lib apps/<product>/crates/<module>`, then add the member to `apps/<product>/Cargo.toml`; Vite in `apps/<product>/ui/` or `apps/<distribution>/`). **DO NOT** generate workspace manifests or applications inside the repository root. **DO NOT** generate `apps/frontend` + `apps/backend` as FastAPI/SPA Python, Phoenix OTP apps, or WordPress plugins.

**Tauri scaffolder:** `npm create tauri-app` writes `package.json` next to `src-tauri/`. If you use it, run it in a scratch directory and **relocate** the frontend into the UI package and the Rust host into `apps/<product>/crates/desktop/` so no `package.json` / `Cargo.toml` lands at the repository root. Prefer composing Tauri by hand on top of the Vite app.

**Ports:** When adding Compose services or Vite/Axum/Tauri `devUrl` ports, scan existing published ports and increment by +1 so they never clash ([port-conflicts.md](../../rules/port-conflicts.md)). Keep every Vite dev and preview port **fixed** (`strictPort: true`) and fail on conflicts. Document chosen ports in `docker-compose.yml`, `tauri.conf.json`, `apps/README.md` and the root README. `scripts/verify_isolated_ports.sh` checks them.

**Module crates:** `core` and every domain module MUST contain domain types and use-cases with **no** Axum, Tokio-net, or `tauri` dependency, and no default dependency on another module crate. Each one MUST build and test alone (`cargo test -p <crate>`). Host crates depend on the API crate via `path = "../api"`. Do not duplicate domain logic in Tauri commands or Axum handlers.

**Distributions:** A white-label distribution is a UI-only package under `apps/<distribution>/` that depends on the default UI package (`file:../<product>/ui` in development, a published version for release), ships its own `src/theme/tokens.css`, `src/theme/labels.<lang>.json`, brand assets, `distribution.json` and an optional Tauri config overlay. It contains no Rust and no server code ([ADR 031](../../../resources/architecture/031-vanilla-typescript-ui-and-white-label-theming.md)).

---

## Step 2: Enforce Documentation Traceability & Sync

PRD lives at `resources/prd.md` (this repo does not use a root `prd.md`). See [docs-resources-split.md](../../rules/docs-resources-split.md): `docs/` is only for GitHub Pages manuals.

1. **Global source of truth:** [`resources/prd.md`](../../../resources/prd.md), [`resources/srs.md`](../../../resources/srs.md) and [`resources/product-vision.md`](../../../resources/product-vision.md).
2. **Localized specs:** Each crate and UI package MUST have `resources/srs.md` bounded to that crate or package. A distribution MAY have its own `resources/prd.md` and `resources/product-vision.md`; its SRS then points at that local PRD.
3. **Traceability metadata** at the top of each `srs.md` (depth of `../` depends on the file's location):
   ```yaml
   ---
   title: "filesquid-server Software Requirements Specification"
   parent: "../../../../../resources/prd.md"
   ---
   ```
4. **Line-by-line traceability:** Every SRS requirement MUST link to its parent in the PRD or global SRS with a relative markdown link (e.g. `[SRS-F-057](../../../../../resources/srs.md#srs-f-057-desktop-and-server-hosts)`). `scripts/verify_srs_traceability.sh` checks both directions.
5. **Sync:** Follow [sync-logic.md](../../rules/sync-logic.md). Server/desktop SRS changes that share domain behaviour MUST update the API crate SRS and the root PRD/SRS.

---

## Step 3: Enforce Technical Stack

You MUST configure the following. Do not add FastAPI, Phoenix, WordPress, Actix, Rocket, Dioxus, Leptos, or a UI framework (React, Svelte, Vue) unless a later ADR explicitly replaces this stack.

**Rust workspace:**
- Stable Rust toolchain pinned once, to an exact release, in `apps/<product>/rust-toolchain.toml` (for example `channel = "1.99.1"`, never the moving `channel = "stable"`, so a new release cannot break every branch overnight; components `rustfmt` and `clippy`). Verify that the pinned stable compiler builds the actual dependency graph; do not copy an old MSRV from a plugin README. Bump the version only in its own `build/` pull request that fixes the new lints.
- `resolver = "2"`, shared `[workspace.package]` for `edition` and `license`. Commit the single `Cargo.lock`.

**Server host (production):**
- **Axum** on **Tokio**, with `tower-http` as needed (trace, CORS, compression). JSON via Serde.
- Domain-driven module crates under `crates/<module>/`. HTTP adapters in `crates/api/` (one API, one OpenAPI document). The server binary adds the bind address, static UI serving and health endpoints.
- Persistence via **SQLx** (or an ADR-justified alternative): SQLite on desktop, Compose-backed Postgres on servers ([ADR 005](../../../resources/architecture/005-sqlite-local-postgres-hosted.md)). Bind parameters; never interpolate SQL.
- `tracing` + `tracing-subscriber` JSON **unbuffered to stdout**. MUST NOT use `tracing-appender` file guards or write `*.log` files ([logging-enforce.md](../../rules/logging-enforce.md)). Every domain operation, job state change and auth event goes to the activity journal ([ADR 033](../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md)).

**Desktop host (Tauri 2):**
- Stable Tauri **major 2** only. Known-good floor from the knowledge base snapshot (re-verify at init time; patches may move, the origin-confusion fix MUST remain):
  - Rust `tauri` **>= 2.11.1** (snapshot example: `2.11.5`)
  - `tauri-build` matching the installed `tauri` generation (snapshot example: `2.6.3`)
  - npm `@tauri-apps/api` minor-aligned with Rust `tauri`
  - npm `@tauri-apps/cli` / Rust `tauri-cli`
- Conventional `crates/desktop/` layout: `lib.rs` builder + `main.rs` desktop entry, `build.rs`, `tauri.conf.json`, `capabilities/`, `icons/`. Add `permissions/` only when custom `#[tauri::command]`s exist; if the UI reaches the API through a custom URI scheme instead, say so in the capability description.
- Frontend imports from `@tauri-apps/api/<module>` (e.g. `core`, `event`, `window`). **Forbidden v1 paths:** `@tauri-apps/api/tauri`, `appWindow`, `tauri.allowlist`, `build.devPath`, `build.distDir`, `tauri.windows`.
- `tauri.conf.json` uses the version-2 schema: windows under `app.windows`, `build.devUrl` + `build.frontendDist` (relative to the config file, e.g. `../../ui/dist`). `app.withGlobalTauri` MUST be `false`.
- Capabilities, permissions, and scopes are the ACL. Custom commands are reachable from local windows unless restricted through `AppManifest`; omitting a command from a capability is **not** a complete ACL. See Step 4.
- A distribution supplies a Tauri **config overlay** (`apps/<distribution>/tauri.<distribution>.json`) applied with `tauri dev --config`, so one desktop crate ships every distribution.
- Official plugins (`fs`, `shell`, `http`, `sql`, `store`, `updater`, …) are **opt-in**. Do not install the catalog. Each plugin needs crate + JS package (when applicable) + registration + config + capability/scope + tests for allowed **and** denied calls.
- `gen/schemas/` is regenerated by `tauri-build` on every build. Gitignore it.

**TypeScript UI (shared):**
- Vite + vanilla TypeScript custom elements, no UI framework ([ADR 031](../../../resources/architecture/031-vanilla-typescript-ui-and-white-label-theming.md)). Tauri requires a static `frontendDist`; an SSR server is **not** embedded.
- `src/api/` holds the API client and a host adapter that selects HTTP (browser) or the custom scheme (desktop). Do not sprinkle transport code through components. `src/theme/` holds tokens and labels. `src/components/` holds custom elements. Generated contract types come from `crates/api/openapi.json` (`npm run contract:types`).
- `@tauri-apps/cli` is a **UI package** devDependency so there is a single `package.json` / lockfile per package. The desktop crate does **not** get a Node project.
- Do not expose arbitrary `VITE_` / `TAURI_` environment prefixes to browser code. No native paths or secrets in frontend build variables.

**Local development:**
- Root `docker-compose.yml`: Postgres, the optional server container, and under the opt-in `observability` profile the optional VictoriaLogs and VictoriaMetrics sinks ([ADR 033](../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md)). Promtail, Redpanda, Loki, Grafana, Alloy and Prometheus are not part of the stack.
- Server: developers MAY run `cargo run -p <server crate>` in `apps/<product>/` and `npm run dev` in the UI package against Compose backing services.
- Desktop: `npm run desktop:dev` from the UI package (which runs `tauri dev` in `crates/desktop/`) using the OS webview (WebView2 / WKWebView / WebKitGTK 4.1). Document native prerequisites per OS without Homebrew-only steps ([platform-agnostic.md](../../rules/platform-agnostic.md)).
- Commit `Cargo.lock` and every `package-lock.json`. Use frozen installs in the pre-commit verifiers (`npm ci`, `cargo --locked`).

---

## Step 4: Tauri Security, IPC, and Host Boundaries

Load [`.agents/kb/tauri2.md`](../../kb/tauri2.md) before implementing this step. Apply the knowledge-base **Agent contract** on every desktop change.

**Trust boundary:** Webview inputs, deep links, imported files, and subprocess output are untrusted. Authorization is enforced in Rust, not by hiding UI. Prefer domain operations ("export this document") over arbitrary "read any path".

**Init-time ACL (required):**
1. List every `#[tauri::command]` in `crates/desktop/build.rs` `AppManifest::commands` (or document in `build.rs` why there are none).
2. Define application permissions under `crates/desktop/permissions/` (no plugin prefix) when commands exist.
3. Grant them only to intended window labels in `crates/desktop/capabilities/`.
4. Set `app.security.capabilities` to the explicit identifier list.
5. Configure a restrictive production CSP (`default-src 'self'`, IPC and custom-scheme origins only as required). Use a **separate** `devCsp` for the Vite origin; do not weaken production CSP for HMR.
6. Do not "fix" failures by broadening permissions, disabling CSP/TLS/signature checks, granting remote IPC, enabling production `devtools`, or exposing arbitrary shell/SQL/filesystem proxies.

**IPC rules:**
- Request/response: small owned inputs, typed Serde DTOs (`camelCase` JS keys by default), explicit `Result<T, E>` errors.
- Long work: IDs, status, cancellation, bounded queues; blocking/CPU work on blocking workers. `async fn` is not non-blocking.
- Managed state: `Send + Sync + 'static`, exact `State<'_, T>` type, no mutex guard across `.await`.
- `std::fs`, a custom `reqwest` client, and `std::process::Command` do **not** inherit plugin scopes. Custom commands MUST validate paths/URLs/ids themselves.

**Logging on desktop:** Desktop stdout is the developer terminal. If `tauri-plugin-log` is used, configure it to stdout only. MUST NOT add a file target. The desktop host never bundles a telemetry server.

**Shipping vs test:** Embedded WebDriver / `tauri-plugin-wdio-webdriver` / extra `devtools` belong behind a test-only Cargo feature (e.g. `test-webdriver`). Shipping builds MUST NOT register them.

---

## Step 5: Populate `apps/README.md`

Create or replace `apps/README.md` so it describes **this** stack (not FastAPI/Phoenix/WordPress). It MUST contain:

- The directory tree from Step 1 with the real product, distribution and crate names.
- A hosts table: how each host reaches the API, who sees plaintext, status.
- The rules for the directory: module crates have no Axum/Tokio-net/`tauri` dependency and build alone; hosts link the API crate; distributions depend on the default UI package by path in development and by published version for release; the unique local ports.
- Docker: root Compose runs Postgres, the opt-in telemetry sinks and optionally the server container. The desktop host is not a Compose service.
- Logging: unbuffered JSON stdout plus the activity journal and Monitor dashboard ([ADR 033](../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md)).
- Testing: each app keeps `unit/`, `integration/`, `ui/`, `features/`, `e2e/`, `performance/` and `allure-results/` under its own `tests/`; Cargo runs from the workspace directory; npm runs inside each UI package ([artifact-isolation.md](../../rules/artifact-isolation.md)).

---

## Step 6: Configure Testing Frameworks

Mirror the Python/JS taxonomy with Rust-native tools on the crates and JS tools on the UI packages ([ADR 017](../../../resources/architecture/017-test-runners-and-reporting.md)).

### 1. Rust (every crate under `apps/<product>/crates/`)
- **`cargo test`** for unit tests under each crate's `tests/unit/` and `#[cfg(test)]` modules. Declare `[[test]]` targets with explicit `path` values so Cargo does not assume a flat `tests/*.rs`.
- **Integration:** Axum tests via `oneshot` against `crates/server/tests/integration/` and `crates/api/tests/`, with Compose (or testcontainers) for real Postgres when the code talks to a database. Tauri protocol tests under `crates/desktop/tests/integration/` cover DTO casing, validation, and error shapes; they do **not** prove capability enforcement by themselves.
- **BDD:** [`cucumber`](https://crates.io/crates/cucumber) (Rust) executing Gherkin under `tests/features/`, writing `tests/allure-results/cucumber-junit.xml`. Do not add a second Gherkin runner for the same flow.
- **Performance:** **Criterion** benches under `tests/performance/` (explicit `[[bench]]` paths). HTTP load MAY use **k6** against the Axum service; do not require Locust.
- **ACL tests (desktop):** Real Tauri integration tests for allowed main-window access **and** denial from an ungranted view. Negative permission/scope tests are mandatory whenever a plugin or custom command is added.
- **Native E2E (desktop):** WebdriverIO **embedded** driver (`@wdio/tauri-service`, `driverProvider: 'embedded'`) under `crates/desktop/tests/e2e/`, covering Windows, Linux, and macOS. Do not claim the standalone `tauri-driver` supports macOS WKWebView. Keep WebDriver behind a test-only Cargo feature.
- Output JUnit XML into that crate's `tests/allure-results/`. Purge the folder before each run.

### 2. TypeScript (`apps/<product>/ui`, `apps/<distribution>`)
- **Vitest** for `tests/unit` and `tests/ui` (jsdom), with the JUnit reporter writing `tests/allure-results/vitest-junit.xml` and a `pretest` script that clears the folder.
- Mock Tauri with `@tauri-apps/api/mocks` (`mockIPC`, window mocks, `clearMocks` between tests). Mocks do **not** prove native permissions or packaging.
- **Playwright** for `tests/e2e` against the **built** UI served by the real server binary (`webServer.command` runs `npm run build` and then `cargo run -p <server crate>`), not the Vite dev server, to satisfy the UI Test Mandate (no hydration/SSR errors). `outputDir` is `tests/test-results`; the JUnit reporter writes `tests/allure-results/playwright-junit.xml`.
- **Cucumber.js** MAY drive Playwright for UI Gherkin, or Rust cucumber may call the same URLs. Prefer **one** Gherkin runner per flow.
- Browser Playwright is not proof of Windows/Linux/macOS webview behaviour.

### 3. Global test reporting
- Rust and TypeScript both emit JUnit XML into `tests/allure-results` of the owning crate or package. Open them in any JUnit or Allure viewer; never write results to the repo root.

### 4. Linting and static analysis
- **`cargo fmt --all --check`** and **`cargo clippy --workspace --all-targets --locked -- -D warnings`** from `apps/<product>/`.
- **`cargo deny`** and/or **`cargo audit`** in the pre-commit verifiers (documented in [ADR 019](../../../resources/architecture/019-coding-style-and-lint.md)).
- **ESLint** (flat config) and **Prettier** inside each UI package.

### 5. Pre-commit verification (no hosted CI)
- This repository does **not** use GitHub Actions or another hosted CI. Every gate runs locally through `scripts/pre-commit.sh` before a push. Do not add `.github/workflows/`.
- `scripts/verify_formatting.sh`: `cargo fmt --all --check` from the Cargo workspace and `prettier --check` in each UI package.
- `scripts/verify_tests.sh`: Clippy, workspace tests, `cargo test -p <crate>` for **each module crate alone** ([ADR 029](../../../resources/architecture/029-filesquid-unified-product-and-distributions.md)), `cargo doc`, then ESLint, `tsc --noEmit` and Vitest in each UI package. Playwright runs when `FILESQUID_E2E=1`; `FILESQUID_SKIP_TESTS=1` skips this step for work-in-progress commits.
- `scripts/verify_docs.sh`: the validators under `scripts/docs/` (ADR structure, PRD allocation, relative links, FileSquid/ImageSquid boundary) and task cleanup.
- Developers run `bash scripts/pre-commit.sh` or install it as the git `pre-commit` hook (see the root README).

### Best practices
- Lint and format **inside** `apps/<product>/` and each UI package before tests ([command-verification.md](../../rules/command-verification.md), [lint-enforce.md](../../rules/lint-enforce.md)).
- TDD: tests first ([tdd-enforce.md](../../rules/tdd-enforce.md)). Walkthrough files go under `apps/<module>/resources/tasks/` ([task-workflow.md](../../rules/task-workflow.md)).
- `npx playwright install chromium` is required once per developer machine; document it in the root README.
- `cargo build` alone does not run the UI or the Tauri bundler. Document `npm run desktop:build` separately. A successful macOS run does not establish Windows/Linux support.

---

## Step 7: Configure Documentation Generation

1. **Rust:** `///` and `//!` rustdoc on all public modules, types, and functions. Generate HTML with `cargo doc --workspace --no-deps` from `apps/<product>/`. Public items MUST be rustdoc-parseable ([ADR 018](../../../resources/architecture/018-inline-docs-generators.md)).
2. **TypeScript:** TSDoc-style comments on all exported modules, custom elements, the API client and its transport adapters, theme loaders and utilities. Types come from TypeScript; do not repeat them in braces.

---

## Step 8: Generate Inline Documentation Rule

Create or **update** [`.agents/rules/docs-enforce.md`](../../rules/docs-enforce.md) so it names rustdoc for every crate under `apps/<product>/crates/` and TSDoc-style comments for every UI package (not Python, Elixir, or PHP). The current file is the reference; keep its structure and change only the crate and package names when they change.

---

## Step 9: Generate Linting Rule

Create or **update** [`.agents/rules/lint-enforce.md`](../../rules/lint-enforce.md). It MUST run Prettier and ESLint inside each UI package, and `cargo fmt --all` plus `cargo clippy --workspace --all-targets --locked -- -D warnings` once from the Cargo workspace directory. Its "Based on" line MUST point at the real lint ADR ([ADR 019](../../../resources/architecture/019-coding-style-and-lint.md)), not a Python Ruff ADR.

---

## Step 10: Generate Architecture Decision Records

Use [adr-formatting.md](../../rules/adr-formatting.md). Write ADRs to `resources/architecture/` (numbered `NNN-slug.md`; there is no `ADRs/` subfolder). Be sure to include separate ADRs specifically for:

1. Test runners and reporting (cargo test, cucumber, Criterion, Vitest, Playwright, WebdriverIO embedded driver, JUnit/Allure) ([ADR 017](../../../resources/architecture/017-test-runners-and-reporting.md)).
2. Inline docs generators (`cargo doc`, TSDoc) ([ADR 018](../../../resources/architecture/018-inline-docs-generators.md)).
3. Coding style and linting (rustfmt, Clippy, ESLint, Prettier, cargo-deny/audit) ([ADR 019](../../../resources/architecture/019-coding-style-and-lint.md)).
4. Server host: Axum + Tokio + SQLx (or the chosen store) and stdout `tracing` ([ADR 001](../../../resources/architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md), [ADR 032](../../../resources/architecture/032-hosting-and-privacy-model.md)).
5. Desktop host: stable Tauri major 2, capability/permission model, shared static UI ([ADR 002](../../../resources/architecture/002-tauri2-desktop-shell.md)).
6. Product-local Cargo workspace of module crates versus duplicating domain in each host ([ADR 029](../../../resources/architecture/029-filesquid-unified-product-and-distributions.md)).
7. The single API and its transports ([ADR 030](../../../resources/architecture/030-filesquid-api-and-transports.md)).
8. UI packages, theming and white-label distributions ([ADR 031](../../../resources/architecture/031-vanilla-typescript-ui-and-white-label-theming.md)).
9. Activity journal, live health and optional OTLP sinks, and the split between containerized server stdout and native desktop stdout ([ADR 033](../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md)).
10. Distribution: server container/binary versus Tauri installers (NSIS/MSI, dmg/app, AppImage/deb/rpm), signing, and whether the updater plugin is in scope ([ADR 020](../../../resources/architecture/020-distribution-web-and-tauri.md)).

Verify current crate/npm names and versions before writing ADRs ([command-verification.md](../../rules/command-verification.md), [dependency-management.md](../../rules/dependency-management.md)). Search `resources/architecture/` before claiming an ADR does not exist ([adr-verification.md](../../rules/adr-verification.md)).

---

## Step 11: Configure Global `.gitignore`

The root `.gitignore` already exists. You MUST **append** (not overwrite) the following if not already present:

```text
# Rust build
**/target/

# Tauri generated schemas (rebuilt by tauri-build on every build)
**/crates/desktop/gen/schemas/

# Test reports & artifacts
**/artifacts/
**/playwright-report/
**/allure-results/
**/coverage/
**/wdio-logs/
**/test-results/

# Dependency directories
node_modules/
**/node_modules/

# Scratch files and transient tests
**/test-card.js
**/screenshot.spec.js

# Allow selective tracking in tests/
**/tests/*
!**/tests/features/
!**/tests/support/
!**/tests/unit/
!**/tests/ui/
!**/tests/e2e/
!**/tests/integration/
!**/tests/performance/
!**/tests/README.md
!**/tests/**/.gitkeep

# Local Compose service data
data/*_data/
```

---

## Step 12: Configure Observability (Compose)

The core of observability is the product's own activity journal, process registry and Monitor dashboard ([ADR 033](../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md)). Compose offers external sinks only as an opt-in profile:

1. `victorialogs` and `victoriametrics` services under `profiles: ["observability"]`, bound to `127.0.0.1`, with data under gitignored `data/*_data/`.
2. `config/victoriametrics/scrape.yml` volume-mounted read-only; it scrapes the server container's metrics endpoint.
3. Desktop processes are **not** in Docker; document that `npm run desktop:dev` stdout is attached in the developer terminal.

Application code still MUST NOT write log files.

---

## Step 13: Update Documentation

1. **Insert** platform-agnostic Rust, Node, and Tauri native-prereq instructions near the top of the root `README.md` (official rustup, Node version managers such as `asdf` or `mise`, and Tauri's OS prerequisite pages for MSVC/WebView2, Xcode CLT, WebKitGTK 4.1; **no Homebrew-only steps**). **MUST NOT** delete existing README content.
2. **Update** `## Run tests` with commands run from `apps/<product>/` (fmt, clippy, workspace tests, one crate alone) and inside each UI package (Prettier, ESLint, `npm run check`, `npm test`, `npm run test:e2e`), plus the documentation validators and `npx playwright install chromium`. **MUST NOT** delete unrelated README content.
3. **Update** `## View logs`: stdout JSON per process, the Monitor dashboard, and how to start the opt-in `observability` Compose profile. State clearly that the sinks receive **container** stdout (server). Desktop logs are process stdout.
4. **Update** `## Run the application`:
   - Server: Compose (if used), `cargo run -p <server crate>` in `apps/<product>/`, `npm run dev` in the UI package, documented URL/port.
   - Desktop: native prerequisites, `npm run desktop:dev` from the UI package, and that a packaged `npm run desktop:build` is required before claiming a release artifact.

Verify every documented command actually runs ([command-verification.md](../../rules/command-verification.md)). Report exactly which OS/arch checks ran; never claim compilation, notarization, or platform testing that did not occur.

---

## Success criteria

- [ ] `apps/<product>/Cargo.toml` is the only Cargo workspace; crates live under `apps/<product>/crates/`; no root `Cargo.toml` / `package.json`. `scripts/verify_workspace_boundaries.sh` passes.
- [ ] Module crates have no Axum/Tokio-net/`tauri` dependency and pass `cargo test -p <crate>` alone; hosts link the API crate; the UI does not reimplement privileged operations.
- [ ] `resources/srs.md` exists per crate and UI package with PRD parent links. `scripts/verify_srs_traceability.sh` passes.
- [ ] Every crate and UI package has `resources/tasks/` and the `tests/` taxonomy including `performance/`; JUnit output is under each app's `tests/allure-results`.
- [ ] `docs-enforce.md` and `lint-enforce.md` describe rustdoc/rustfmt/Clippy/ESLint/Prettier for the real crate and package paths, not Ruff, Mix, or PHPCS.
- [ ] ADRs exist for test, docs, lint, server, desktop, workspace, API, UI, observability, and distribution, and they do not contradict earlier accepted ADRs.
- [ ] Tauri stays on stable major 2; capabilities, production CSP, separate `devCsp`, and `frontendDist` are configured; no v1 allowlist/`devPath` APIs; `gen/schemas/` is gitignored.
- [ ] No `.github/workflows/`. `scripts/pre-commit.sh` runs formatting, Clippy, workspace and per-crate tests, UI lint/check/tests and the documentation validators, and passes.
- [ ] Root README explains rustup/Node/Tauri prerequisites, server vs desktop run/test/log paths, without erasing About this project.
- [ ] Relative links only; no absolute developer machine paths. `scripts/verify_relative_links.sh` passes.
- [ ] Compose does not containerize the Tauri host. Unique ports; `scripts/verify_isolated_ports.sh` passes.
