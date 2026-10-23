# FileSquid foundation evidence

The foundation record that [Task 0.8](./implementation-plan.md#task-0.8) produces. It holds the test-reporting evidence ([NFR-008](./prd.md#nfr-008-maintainability-and-verified-engineering), [ADR 017](./architecture/017-test-runners-and-reporting.md)), the **proposed** quantitative budgets of [NFR-002](./prd.md#nfr-002-benchmark-envelope-and-responsiveness), [NFR-003](./prd.md#nfr-003-bounded-processing-and-migration-endurance) and [RC-002](./prd.md#rc-002-performance-and-resource-bounds), the performance-profile declarations that [F-040](./prd.md#f-040-compatibility-and-parity-register) asks for, the platform and provider limits of [TC-003](./prd.md#tc-003-platform-and-provider-limits) and the architecture-versus-scaffold record of [TC-004](./prd.md#tc-004-decision-status-and-repository-alignment). The item-level capability register is a separate document: [capability register](./capability-register.md).

**Snapshot:** `main` at the merge-base of Task 0.8 (2026-10-23). Every statement below about what exists is limited to what a ticked task in the [implementation plan checklist](./implementation-plan-checklist.md) shows.

> **Nothing in this document is a measured result.** Every budget, bound and pilot figure below is **proposed**. None has been measured on a packaged `filesquid-desktop` build, on `filesquid-server` or on the encrypted browser host, and none has been achieved. FileSquid makes **no claim of [FileSquid U-11](./product-vision.md#platform-ownership-and-access)**. Accepted stack direction is not evidence that NFR-002, NFR-003 or U-11 were measured ([TC-004](./prd.md#tc-004-decision-status-and-repository-alignment)).

## 1. Test reporting (NFR-008, ADR 017)

Lint and format run before unit, integration, BDD and built-production UI tests, in the local pre-commit hook (`bash scripts/pre-commit.sh`); there is no hosted CI ([local-verification.md](../.agents/rules/local-verification.md)). Each workspace writes structured results into its own `tests/allure-results/` directory, which is cleared before each run, and each task walkthrough pastes the raw output of its runs. The commands are in the README section [Run tests](../README.md#run-tests).

| Workspace | Runners | Results | Cleared | Report | Delivered by |
|---|---|---|---|---|---|
| `apps/filesquid` Cargo workspace (every crate) | `cargo fmt`, `cargo clippy -D warnings`, `cargo test` with the `cucumber` BDD features in each crate's `tests/features/` | `apps/filesquid/crates/<crate>/tests/allure-results/cucumber-junit.xml` (JUnit) | The feature harness removes and recreates `tests/allure-results/` before each run | JUnit, read by Allure or any JUnit viewer | Earlier F0 tasks; kept by every crate task |
| `apps/filesquid/ui` (`@filesquid/ui`) | Prettier, ESLint, `tsc --noEmit`, Vitest (jsdom), Playwright against the built UI served by the real `filesquid-server` and against `vite build` plus `vite preview` (port 4173) | `tests/allure-results/vitest/`, `vitest-junit.xml`, `tests/allure-results/playwright/` and its JUnit | `npm test` clears all of `tests/allure-results/`; `npm run test:e2e` clears only the Playwright results | `npm run report` writes `tests/allure-report/`; `npx allure open tests/allure-report` | [UI-0.6](../apps/filesquid/ui/resources/plan.md) ([walkthrough](../apps/filesquid/ui/resources/tasks/Task-UI-0.6-Walkthrough.md)) |
| `apps/imagesquid` (`@imagesquid/ui`) | The same runners on port 4174 | The same paths under `apps/imagesquid/tests/allure-results/` | The same | The same | UI-0.6 |
| `apps/filesquid/crates/desktop/tests/e2e` (packaged window) | WebdriverIO with the Tauri service and the embedded driver, under Xvfb on Linux | `apps/filesquid/crates/desktop/tests/allure-results/wdio/` (JUnit) | Cleared and rewritten by `npm run test:e2e` | JUnit | Task DE-F1.1 ([ADR 017](./architecture/017-test-runners-and-reporting.md) amendment) |

Not yet in place: load and performance tests (the NFR-002 harness belongs to [Task FS-3.4](./implementation-plan.md#task-fs-3.4), [Task FS-3.13](./implementation-plan.md#task-fs-3.13) and [Task 10.1](./implementation-plan.md#task-10.1)), and the server half of the Task 0.8 reporting work (BE-0.6), which moved to [Task FS-3.10](./implementation-plan.md#task-fs-3.10) with the rest of the server host ([ADR 035](./architecture/035-macos-first-desktop-mvp.md)).

## 2. Proposed budgets: NFR-002 benchmark envelope and responsiveness

Status of every row: **proposed, not measured**. "Measured" is `none` until the named task records a run on a packaged build; the measuring task then records the result beside the budget, and a missed budget stays visible.

### Test profiles

| Profile | Assets | Resources | Bytes | Albums | Memberships | Measured |
|---|---|---|---|---|---|---|
| Small | 10,000 | 20,000 | 100 GB | 100 | — | none |
| Medium | 100,000 | 200,000 | 1 TB | 5,000 | — | none |
| Large | 1,000,000 | 2,000,000 | 10 TB | 50,000 | 10,000,000 | none |

Every profile includes duplicate bytes, pairs, Unicode names, dense memberships, late sidecars, high decoded-pixel counts and varied supported formats. Support tests add a **100 GB single resource** and a **250 GB archive part**, both streamed rather than read into memory-sized allocations; compressed size alone never establishes a bounded decoding cost.

### Budgets

| # | Budget | Proposed value | Conditions | Measured | Measuring task |
|---|---|---|---|---|---|
| B-1 | Warm viewport interaction to visible feedback | p95 ≤ 100 ms | Declared desktop reference machine: 4 cores, 16 GB RAM, SSD; at least 100 interactions after a declared warm-up; cold behaviour reported separately | none | Task FS-3.4 (macOS desktop), Task 10.1 |
| B-2 | Indexed metadata search | p95 ≤ 2 s | Same machine; at least 100 queries after warm-up | none | Task FS-3.4, Task 10.1 |
| B-3 | First useful local viewport | ≤ 3 s | Same machine; Large catalog | none | Task FS-3.4, Task 10.1 |
| B-4 | Warm cached control feedback, mobile and browser | p95 ≤ 100 ms | Separately declared 4 GB device and network profile | none | [Task FS-5.1](./implementation-plan.md#task-fs-5.1), Task 10.1 |
| B-5 | Completed common browse and search work, mobile and browser | p95 ≤ 2 s | Same profile; downloads of unavailable originals excluded from the budget but still shown in end-to-end reports | none | Task FS-5.1, Task 10.1 |
| B-6 | Resident prepared thumbnail or viewer image, and prepared next slide, paints the correct image | p95 ≤ 100 ms from the request or scheduled presentation time | Every supported desktop, browser and mobile profile; a placeholder, stale image or spinner never passes; preparation coverage across the exercised collection reported | none | Task FS-3.4, [Task 6.1](./implementation-plan.md#task-6.1) (slides), Task 10.1 |
| B-7 | 60 Hz scrolling and presentation frame budget | 16.7 ms per frame | Missed-frame rate, slideshow transition lateness, p95 and p99 sample sizes, clock boundaries, warm-up and cache contents and error limits are defined per profile before release acceptance; other refresh rates need their own budget; reports show stalls and p95/p99, never averages alone | none | Task FS-3.4, Task 10.1 |
| B-8 | Per-feature readiness matrix | Useful-preview and full-quality deadlines for cold, warm, evicted and missing-derivative cases | A nonempty format, device and local-or-remote matrix before the feature ships; first useful viewport, warm feedback and full quality pass independently; tests run idle and under concurrent import or migration, thumbnail generation, indexing, provenance scans and batch metadata work, with navigation reversals and jumps, cache loss and resource pressure | none | The task that ships each feature |

Input acknowledgement, first useful preview and final requested-quality readiness are three separate measurements with separate pass results. A fast spinner never compensates for a missed image-readiness target, and labelling every demanding format unsupported never removes it from the evidence.

## 3. Proposed bounds: NFR-003 bounded processing and migration endurance

Status of every row: **proposed, not measured**.

| # | Bound | Proposed value | Measured | Measuring task |
|---|---|---|---|---|
| P-1 | Desktop default staging cap | 10 GiB | none | [Task 1.2](./implementation-plan.md#task-1.2), [Task 2.3](./implementation-plan.md#task-2.3) |
| P-2 | Staging reserve | max(2 GiB, 5 % of destination capacity); preflight picks a larger cap or a segmented path when a resource cannot fit; a streamed large resource needs no full extra library copy | none | Task 1.2, Task 2.3 |
| P-3 | Default active transfers | ≤ 4 | none | Task 1.2, Task 2.3 |
| P-4 | CPU-heavy workers | ≤ 2; every queue has an explicit finite limit and backpressure | none | Task 1.2, [Task 1.3](./implementation-plan.md#task-1.3) |
| P-5 | Attempts per resource | at most 5 in total, the first included, then quarantine; exponential backoff capped at 60 s plus jitter; a manual retry starts a new recorded recovery cycle under the same cap | none | Task 1.2, Task 2.3 |
| P-6 | Migration process memory | ≤ 2 GiB, excluding separately budgeted ML models | none | Task 2.3, Task 10.1 |
| P-7 | Restart to a resumable job state | ≤ 30 s on the reference desktop, without rescanning completed media; repeated bytes per route measured | none | Task 1.2, Task 2.3 |
| P-8 | Soak | 72 hours of mixed and fault workload with no unbounded growth; post-warm-up non-cache memory growth ≤ 10 % | none | Task 10.1 |
| P-9 | Throughput | Reported as verified bytes per unit time with hardware, network, small-file mix and sidecar reconstruction time; no WAN speed is promised | none | Task 2.3, Task 10.1 |

Before implementation, each scheduler profile defines a measurable background-service guarantee or a visible pause policy, and each device or browser profile defines finite memory and disk cache budgets and prefetch item, byte and time limits. Caches key on identity, content, recipe and metadata revision, view, sort and filter and permission scope; stale work is cancelled; eviction never deletes originals, history or pinned offline media. Durable completed-item checkpoints extend to import and upload, thumbnail and index, provenance-scan and metadata batches; a restart revalidates source, version and authorisation and never restarts a whole batch, duplicates events or marks media falsely complete.

What exists today toward these bounds, without any measurement: the job and checkpoint contracts in `filesquid-core` ([Task 0.2](./implementation-plan.md#task-0.2), [`job_checkpoint_protocol_NFR-003.feature`](../apps/filesquid/crates/core/tests/features/job_checkpoint_protocol_NFR-003.feature)), the desktop job and checkpoint store ([Task 0.5](./implementation-plan.md#task-0.5)) and the journal's bounded, backpressured write queue ([Task FS-2.4](./implementation-plan.md#task-fs-2.4)).

## 4. RC-002 verification (proposed)

RC-002 passes only when the assigned NFR-002 and NFR-003 profiles pass with declared hardware, browser, codec, cache and network conditions and complete structured reports. The verification that the measuring tasks will run:

- p95 and p99 interaction and image readiness, frame stalls, missed-frame rate, slide lateness and preparation coverage, in cold, warm, evicted and missing-cache states, idle and under concurrent import, migration, thumbnail, index, scan and metadata work;
- direction changes, sort, filter and permission changes, high decoded-pixel counts, missing derivatives, browser quota loss and resource pressure;
- peak decoded, cache and process memory, staging, queue and concurrency caps, cache hit, miss, eviction and redecode rates, speculative bytes, background progress, resume time, completed-work reuse, repeated bytes, throughput and the 72-hour soak.

Cloud pilots additionally load-test a proposed **100 concurrently active sessions with 20 concurrent resumable transfers** and disclose the infrastructure sizing. This is a **proposed pilot test, not a capacity promise** ([Task FS-3.13](./implementation-plan.md#task-fs-3.13)). Absent limits, or a difficult-media profile in which every format is unsupported, do not pass.

## 5. Performance-profile declarations (F-040)

Each supported performance profile declares device, browser or webview, media format and dimensions, cache and readiness, and concurrency coverage, with actual evidence and unresolved limits. None is declared with evidence yet:

| Profile | Order | Device | Browser or webview | Formats and dimensions | Cache and readiness | Concurrency | Evidence | Declaring task |
|---|---|---|---|---|---|---|---|---|
| macOS desktop (packaged, early access) | First | 4-core, 16 GB, SSD reference (proposed) | WKWebView | Undeclared | Undeclared | Undeclared | none | Task FS-3.4 |
| `filesquid-server` multi-user | Second | Undeclared | Chromium, Firefox and Safari through the static UI | Undeclared | Undeclared | Proposed pilot of 100 sessions and 20 transfers | none | Task FS-3.13 |
| Linux and Windows desktop | Mandatory at maturity | Undeclared | WebKitGTK, WebView2 | Undeclared | Undeclared | Undeclared | none | [Task FS-3.11](./implementation-plan.md#task-fs-3.11), Task 10.1 |
| Encrypted browser host and mobile web | Mandatory at maturity | 4 GB device and network (proposed) | Major desktop and mobile browsers | Undeclared | Undeclared | Undeclared | none | Task FS-5.1, Task 10.1 |

## 6. Platform and provider limits (TC-003)

No route that depends on these limits exists on `main`, so none is verified. Each is verified per supported version when its route lands, and the [capability register](./capability-register.md) then carries the version matrix.

| Limit | Status | Verified by |
|---|---|---|
| PhotoKit permissions and library coverage (macOS) | Unverified; no PhotoKit route | [Task 2.5](./implementation-plan.md#task-2.5) |
| Google export and API restrictions (Takeout, Picker) | Unverified; no Google route | Task 2.3 |
| Microsoft placeholders and range semantics (OneDrive) | Unverified; no OneDrive route | Task 2.3 |
| Browser lifetime and filesystem constraints | Unverified; no browser host | Task FS-5.1, [Task 1.6](./implementation-plan.md#task-1.6) |
| Media licensing (HEIC, FFmpeg) | Unverified; the licence and patent review of the DMG's linked inventory is part of the MVP gate ([ADR 011](./architecture/011-native-media-adapters-and-ffmpeg-license.md)) | [Task FS-3.9](./implementation-plan.md#task-fs-3.9), [Task 7.4](./implementation-plan.md#task-7.4) |
| Capture dependencies (cameras, devices) | Unverified; no device adapter | [Task 8.1](./implementation-plan.md#task-8.1) |

Apple proprietary edit history, Apple rendering and undocumented Apple APIs are never assumed.

## 7. Architecture versus scaffold (TC-004)

The target architecture below is **hypothetical until each part ships**: it is the direction set by ADRs 001 to 016 and 022 to 028 as amended or superseded by ADRs 029 to 032, with the later ADRs 033 to 041, in [resources/architecture/](./architecture/README.md). Stack candidates become reviewed ADRs before implementation. The repository supplies module crates, two hosts and UI shells; it does not yet supply the product features the PRD specifies. `node scripts/docs/validate-adrs.mjs` reads `resources/architecture/` (its `adrDir`) and passes on this snapshot.

| Part | Target (ADR) | On `main` at the snapshot |
|---|---|---|
| One Cargo workspace of module crates | ADR 029, ADR 001 | **Present:** `core`, `api`, `native`, `ocfl`, `metadata`, `c2pa`, `prov`, `cli`, `server`, `desktop` in `apps/filesquid/crates/`; each module crate builds and tests alone (hook step "module crate alone") |
| `filesquid-ocfl` engine | ADR 016, ADR 023, ADR 010 | **Present for local storage:** read, validate, write, change model, git guard; SHA-256 is the digest (`crates/ocfl/src/digest.rs`); S3-compatible storage and the hosted ciphertext profile are absent |
| Git is not the OCFL library | TC-004, ADR 016 | **Kept:** OCFL objects are written by `filesquid-ocfl`; the Git guard refuses Git metadata in the storage root, the storage hierarchy and each object root ([module README](../apps/filesquid/crates/ocfl/README.md#git-guard)) |
| `filesquid-metadata` | ADR 024, ADR 027, ADR 022 | **Present:** register, CLI, CSV, WASM build, OCFL-sourced extraction, JSON sidecar, the `ocfl` store and the import record; the `sqlite` store is absent |
| `filesquid-prov` | ADR 040, ADR 026 | **Scaffold:** model and serialisation (PV-0.1); OCFL records, commit-path linkage and history routes are absent |
| `filesquid-c2pa` | ADR 037, ADR 015 | **Partial and unlinked:** parsing and hard bindings (CP-0.1 to CP-0.3); no host links it before phase C |
| `filesquid-api` with OpenAPI 3.1 contract | ADR 030 | **Present:** contract, generated TypeScript types with a drift test, transport-agnostic router, file, metadata, job, event, journal, admin and library routes, deny-by-default authorisation |
| Desktop host (Tauri 2, custom URI scheme, no TCP listener) | ADR 002, ADR 035 | **Present from source:** per-launch token, refusals journaled, SQLite catalog and journal through `filesquid-native`; no installer (Task FS-3.1) |
| `filesquid-server` (HTTP, static UI, accounts, PostgreSQL) | ADR 030, ADR 005, ADR 032 | **Partial:** HTTP transport, static UI and admin token on `127.0.0.1:8083` with the SQLite journal store; accounts, groups, storage root and PostgreSQL are absent (Task FS-3.10) |
| Default UI and distributions (vanilla TypeScript custom elements) | ADR 031, ADR 028 | **Partial:** shell, tokens, navigation, file browser and Monitor in `@filesquid/ui`; ImageSquid overrides tokens and labels; the panels, Logs view and drive-parity views are open |
| Activity journal and Monitor | ADR 033 | **Present:** journal, process registry, health, metrics, admin routes, Monitor, export; OTLP export absent (Task FS-3.6) |
| Native media adapters (libvips, FFmpeg) | ADR 006, ADR 011 | **Absent:** no media module or worker |
| Encrypted browser host (WASM) | ADR 008, ADR 025 | **Absent** (Task FS-5.1) |
| Ciphertext-only hosted service `*.filesquid.tech` | ADR 004, ADR 012, ADR 032 | **Absent** (Task FS-4.1, Task 0.6) |
| Client encryption and key custody | ADR 009 | **Contracts only** in `filesquid-core` (Task 0.2); no encryption or keys |

Later work that stays open ([T-034](./product-vision.md#later-gates)): the named cloud region, the payment processor, the recorder implementation, job SQL, SPA-asset signing and the independent cryptography audit. Rules specific to another product or stack never silently override FileSquid's local-first requirements; synthesising the architecture document from codebase reality is [Task 10.3](./implementation-plan.md#task-10.3).

## Maintenance

- A task that measures a budget records the result, the hardware, the build and the walkthrough link beside the proposed value in this document; it never deletes a missed budget.
- A task that ships a part named in section 7 updates its row in the same pull request ([pr-documentation-update.md](../.agents/rules/pr-documentation-update.md)).
