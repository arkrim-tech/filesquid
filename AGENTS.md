# AI AGENT Principles

**Purpose:** Define the principles that AI agents working on this project must follow and implement.

**CRITICAL INSTRUCTION:** Before beginning any implementation, scaffolding, or configuration task, you MUST use the list_dir and view_file tools to read the contents of all markdown files in .agents/rules/. You must strictly adhere to these rules over any default framework behaviors.

**CRITICAL INSTRUCTION** When the user deletes or revises a file (or a deletion/revision appears in the workspace or git status), do **not** assume it was accidental. Do **not** restore previous content or re-add removed files without first asking whether the change was intentional and should stick. Restore only after explicit user confirmation. Tactically enforced via [`.agents/rules/no-silent-restore.md`](./.agents/rules/no-silent-restore.md).

## 1. Test-driven development

**Purpose:** Ensure that all code is well-tested which includes functional feature tests, unit tests integration tests, UI tests, and performance tests.

- **Linting Mandate:** All code MUST be linted and formatted using the tools specified in the project's Architecture Decision Records before tests are run or code is merged. The agent MUST resolve all linting errors automatically.
- **UI Test Mandate:** All frontend feature branches MUST include comprehensive UI tests alongside standard unit tests to ensure UI components render and behave correctly. Crucially, these UI tests MUST execute against built production artifacts (not just dev servers) and MUST include explicit, rigorous DOM assertions that verify components mount without triggering server-side rendering (SSR) or browser hydration errors. Superficial tests that lack strict visual DOM state assertions are strictly forbidden.
- **Integration Test Mandate:** Any backend task that interfaces with an external API, database, or filesystem MUST include integration tests utilizing the framework specified in the project's Architecture Decision Records.
- **Performance Test Mandate:** Any application intended for production deployment MUST include load/performance tests using the framework specified in the project's Architecture Decision Records to simulate expected user concurrency and data volume.
- **Test Reporting Mandate:** All test runs (Unit, UI, Integration, and BDD) MUST generate structured reports using the framework specified in the project's Architecture Decision Records to provide stakeholders with a clear, visual history of test passed/failed states and coverage. The agent MUST include explicit CLI instructions in the project README.md for how developers can execute the complete test suite (including specific commands for BDD feature tests) and how to generate/view the structured test reports.
- **Inline Documentation Mandate:** The AI agent MUST write comprehensive inline documentation for ALL classes, functions, modules, and complex code blocks. This documentation must clearly explain the purpose, arguments, return values, and intended usage of the code. This is a strict requirement for all languages and frameworks, enforced via the specific standards configured in the initializing project template.
- **Walkthrough Documentation:** When executing tests, the agent MUST copy the raw CLI output of the test runs into a permanent `apps/[Module Name]/resources/tasks/Task-[Task ID]-Walkthrough.md` file to provide immediate visual proof of passing tests, in addition to generating the structured test reports.
- **Unit Test Traceability:** When code changes make a unit test obsolete or require modifications to the underlying implementation, the agent MUST explicitly update, refactor, or remove the corresponding unit tests to remain in sync with the source code.
- **Strict Artifact Isolation:** This principle is tactically enforced on all AI agents via the instructions in `[artifact-isolation.md](./.agents/rules/artifact-isolation.md)`.
- **PR Review Mandate:** All proposed Pull Requests MUST be reviewed by an external AI agent acting as a Senior Software Engineer. The agent will analyze code changes, enforce project paradigms, check for logic flaws, and provide change requests. The external reviewer is Cubic. Any identified issues will be fixed and pushed to the PR, and the review-and-fix loop repeats until no Cubic comments remain. Agents wait for each review by polling Cubic's check run, never through PR-activity subscriptions or scheduled check-ins. For tasks inside the current milestone, agents MAY then merge their own PR into the main branch, and they stop for the owner's manual testing review once the milestone's gate task is merged. The owner sets each milestone in the "Current milestone" section of the rule. Tactically enforced via [`pr-review-loop.md`](./.agents/rules/pr-review-loop.md).

## 2. Branch-Per-Task

**All development must follow the Git Branch per Task workflow:**

- **Active Enforcement:** The Branch-Per-Task and Execution Documentation requirements are tactically enforced on all AI agents via the instructions in `[task-workflow.md](./.agents/rules/task-workflow.md)`.

## 3. Standalone Production Code

**Purpose:** Ensure that the core application codebase is completely self-contained, deployment-ready, and not permanently entangled with its surrounding project-management or AI-development tooling.

- **Active Enforcement:** This principle is tactically enforced on all AI agents via the instructions in `[src-isolation.md](./.agents/rules/src-isolation.md)`.

## 4. Strict Command Verification

**Purpose:** Guarantee that any CLI commands, build steps, or execution scripts documented for humans are actually functional and tested against current dependencies.

- **Active Enforcement:** This principle is tactically enforced on all AI agents via the instructions in `[command-verification.md](./.agents/rules/command-verification.md)`.

## 5. Local Verification, No Hosted CI

**Purpose:** Keep every quality gate fast, deterministic and under the developer's control. This repository does NOT use GitHub Actions or any other hosted CI; every gate runs locally through the repository's pre-commit hook before a push.

- **No hosted CI:** Agents MUST NOT add `.github/workflows/` or any other hosted CI configuration. Pull request status comes from the local hook, Cubic review and human review.
- **Pre-commit is the gate:** `scripts/pre-commit.sh` runs the branch, port, document-sync, traceability, link, workspace-boundary, logging, formatting, documentation and lint/test verifiers. A commit that fails the hook MUST NOT be pushed.
- **Active Enforcement:** This principle is tactically enforced on all AI agents via the instructions in `[local-verification.md](./.agents/rules/local-verification.md)`.

## 6. Pull Request Documentation Updates

**Purpose:** Guarantee that the permanent project documentation stays strictly synchronized with the codebase whenever new features, architecture changes, or schema updates are introduced.

- **Active Enforcement:** This principle is tactically enforced on all AI agents via the instructions in `[pr-documentation-update.md](./.agents/rules/pr-documentation-update.md)`.

## 7. Sync Logic Verification
**Purpose:** Guarantee that upstream documentation (PRD, SRS, ADRs, Implementation Plan) is consistently aligned before writing implementation code, and to require explicit human review for any detected contradictions.

- **Active Enforcement:** This principle is tactically enforced on all AI agents via the instructions in `[sync-logic.md](./.agents/rules/sync-logic.md)`.
- **Search Before Denial:** AI agents MUST explicitly search `resources/architecture/ADRs/` before denying technology use or claiming an ADR doesn't exist. This is tactically enforced via `[adr-verification.md](./.agents/rules/adr-verification.md)`.

## 8. Portable Documentation (Relative Links)

**Purpose:** Ensure that the project's documentation remains universally portable, readable across any environment, and correctly navigable natively within Git hosting services like Github.

- **Active Enforcement:** This principle is tactically enforced on all AI agents via the instructions in `[links-enforce.md](./.agents/rules/links-enforce.md)`.
- **Strictly Relative Links:** Anytime an AI agent generates documenting artifacts, readmes, technical specs, or logs that cross-reference another file in this project, it MUST use standard Markdown relative linking (e.g., `[PRD](../prd.md)` or `[Task 1.1](./tasks/Task-1.1-Plan.md)`).
- **Prohibited Absolute Paths:** The agent MUST NEVER write absolute file paths native to its current execution environment (e.g., `/Users/johnathondoe/...`) into the permanent project repository files. Absolute paths break immediately upon push to a remote repository or clone by another developer.

## 9. Platform-Agnostic Documentation

**Purpose:** Ensure that the project does not alienate users on different operating systems by hardcoding platform-specific assumptions (e.g., assuming macOS and Homebrew are the default).

- **Active Enforcement:** This principle is tactically enforced on all AI agents via the instructions in `[platform-agnostic.md](./.agents/rules/platform-agnostic.md)`.

## 10. Standards Compliance

**Purpose:** Ensure the project adheres to established industry standards for interoperability, requirements engineering, and documentation.

- **Agent Skills 1.0:** The project template and agent tools comply with the Agent Skills 1.0 specification (including progressive disclosure, multi-tool support, and vendor neutrality). The framework principles are defined in [`.agents/kb/agent-skills-1.0.md`](./.agents/kb/agent-skills-1.0.md).
- **Conventional Commits:** All git commits MUST use the Conventional Commits format (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`). The exact formatting rules are located in [`.agents/kb/conventional-commits.md`](./.agents/kb/conventional-commits.md).
- **ISO/IEC/IEEE 29148:** The Software Requirements Specification (SRS) MUST be Unambiguous, Complete, Verifiable, Consistent, and Traceable. Specific agent instructions for these criteria are defined in [`.agents/kb/iso-29148-srs.md`](./.agents/kb/iso-29148-srs.md).
- **ISO/IEC/IEEE 42010:** Architecture Decision Records (ADRs) MUST include the 9 specific information items required by the ISO 42010 standard for architecture descriptions as defined in [`.agents/kb/iso-42010-adr.md`](./.agents/kb/iso-42010-adr.md). ADR location, naming, and formatting rules are tactically enforced via [`adr-formatting.md`](./.agents/rules/adr-formatting.md).
- **RFC 2119:** All requirements documentation (PRD, SRS) must use explicit RFC 2119 terminology (e.g., MUST, MUST NOT, REQUIRED, SHALL, SHALL NOT, SHOULD, SHOULD NOT, RECOMMENDED, MAY, OPTIONAL) to define the significance of each requirement. Exact definitions are located in [`.agents/kb/rfc-2119-req-terms.md`](./.agents/kb/rfc-2119-req-terms.md).
- **RFC 3339:** All date and time specifications in projects MUST use the RFC 3339 standard. UTC MUST be used as the default timezone for all logs, databases, and internal timestamps. Exact definitions are located in [`.agents/kb/rfc-3339-date-and-time.md`](./.agents/kb/rfc-3339-date-and-time.md).

## 11. Centralized Logging & Observability

**Purpose:** Ensure that the application treats logs as event streams, writing unbuffered to `stdout` rather than managing logfiles, adhering to modern centralized observability methodologies. This provides the foundation for a modern event streaming and observability stack.

- **Stdout Mandate:** Applications MUST NEVER route or store logs in files themselves. Each running process writes its event stream unbuffered to `stdout`.
- **Observability Stack:** The applications MUST be instrumented and infrastructure-configured to route these stdout streams into a centralized observability tool.
- **Technical stack** The recommended stack of observability tools is:
  - **Promtail:** Captures the container `stdout` streams natively.
  - **Redpanda:** Acts as the high-throughput, Kafka-compatible message queue to buffer and distribute log events sequentially without loss.
  - **Loki:** Indexes and stores the log streams for long-term archival.
  - **Prometheus:** Scrapes time-series metrics emitted by the apps and services.
  - **Grafana:** Provides the centralized UI to dashboard, introspect, and correlate the logs and metrics.
- **Active Enforcement:** This principle is tactically enforced on all AI agents via the instructions in `[logging-enforce.md](./.agents/rules/logging-enforce.md)`.

## 12. Decoupled Execution Timelines

**Purpose:** Guarantee architectural specs can survive aggressive timeline refactoring without requiring massive document rewrites by isolating execution phases from the core requirements.

- **Active Enforcement:** This principle is tactically enforced on all AI agents via the instructions in `[decoupled-execution.md](./.agents/rules/decoupled-execution.md)`.

## 13. Security by Design

**Purpose:** Protect user data with architecture, not policy.

- **Client-side encryption:** private content is encrypted on the owner's device before it reaches any hosted service. The `*.filesquid.tech` backup and sync service stores and relays ciphertext only and never processes private content ([ADR 004](./resources/architecture/004-axum-hosted-ciphertext-mailbox.md), [ADR 032](./resources/architecture/032-hosting-and-privacy-model.md)).
- **Owner-held keys:** key custody and recovery stay with the owner. No hosted service can decrypt a library ([ADR 009](./resources/architecture/009-cryptography-webcrypto-compatible-suite.md)).
- **Deny by default:** every API call is authorised by `filesquid-api` before it touches files, SQL or keys. The desktop host opens no TCP listener and grants Tauri capabilities explicitly ([ADR 002](./resources/architecture/002-tauri2-desktop-shell.md), [ADR 030](./resources/architecture/030-filesquid-api-and-transports.md)).
- **Reviewed cryptography only:** the WebCrypto-compatible suite in ADR 009, with no home-grown primitives.

## 14. Privacy by Design

**Purpose:** Keep the owner's identity, content and activity private unless the owner explicitly shares them.

- **No account, no telemetry:** the desktop app works fully offline with no account and no phone-home. Nothing leaves the device without an explicit, scoped authorisation.
- **Honest operators:** a self-run `filesquid-server` operator can read what it stores, and the UI says so. The hosted service cannot ([ADR 032](./resources/architecture/032-hosting-and-privacy-model.md)).
- **No server-rendered private pages:** UIs are client-rendered ([ADR 031](./resources/architecture/031-vanilla-typescript-ui-and-white-label-theming.md)).
- **Scoped external processing:** AI providers and the paid C2PA publisher receive only the inputs the owner selects, under saved scoped permissions, and every such use is disclosed.
- **Privacy on export and sharing:** metadata exports offer scrubbing of location and identity fields, and sharing always requires new destination permissions.

## 15. Modularity

**Purpose:** Ensure that the application is modular and can be easily extended in the future by dividing functionality into modules. Each module should be self-contained and have a clear interface.

- **One workspace of independent module crates:** `core`, `ocfl`, `metadata`, `prov`, `c2pa`, `api` and the hosts live in one Cargo workspace. A module crate MUST NOT take a default dependency on another module; the only exceptions are two optional ones, the metadata module's `ocfl` store and the prov module's `ocfl` feature, and neither may add `filesquid-native` or `filesquid-c2pa` ([ADR 027](./resources/architecture/027-filesquid-metadata-pluggable-stores.md), [ADR 029](./resources/architecture/029-filesquid-unified-product-and-distributions.md), [ADR 040](./resources/architecture/040-prov-o-history.md)).
- **Stand-alone modules:** each module builds, tests and publishes on its own and is usable by programs that never run FileSquid.
- **Distributions are UI layers only:** ImageSquid and Arkrim are views, tokens, labels and brand with no backend code. A capability a distribution needs is added to FileSquid as a module.
- **Composable panels:** UI panels plug into named sidebar slots and never depend on each other ([ADR 028](./resources/architecture/028-filesquid-ui-packages-and-panel-composition.md)).

## 16. Interface-First Design

**Purpose:** Describe how components communicate and how their API access enables future extensibility.

- **One versioned API:** every feature is reachable through `filesquid-api`. UIs never call modules, the filesystem or a database directly ([ADR 030](./resources/architecture/030-filesquid-api-and-transports.md)).
- **Contract in the repository:** the API contract is OpenAPI 3.1 owned by the `api` crate. TypeScript types are generated from it, and a test fails when they drift.
- **Transport-agnostic router:** the same router is mounted unchanged by the desktop host (custom URI scheme) and by `filesquid-server` (HTTP), with one event channel for jobs and changes.
- **Integration through published surfaces:** third parties and distributions integrate through the published API and the `@filesquid/ui` package, never through internal crates.

## 17. Minimize Dependencies

**Purpose:** Ensure the application has minimal dependencies on external services, libraries, and vendor lock-in.

- **No external database service:** SQLite locally and PostgreSQL for multi-user servers ([ADR 005](./resources/architecture/005-sqlite-local-postgres-hosted.md)). Canonical state is plain OCFL on disk or S3-compatible storage, readable with ordinary tools ([ADR 010](./resources/architecture/010-ocfl-1-1-dual-profile.md)).
- **No external authentication service:** accounts, groups and sessions live in `filesquid-server`.
- **No UI framework:** UIs are vanilla TypeScript with standard custom elements, built with Vite ([ADR 031](./resources/architecture/031-vanilla-typescript-ui-and-white-label-theming.md)). npm packages are limited to build and test tooling.
- **Reviewed native adapters:** media work uses libvips and licence-cleared native adapters ([ADR 006](./resources/architecture/006-libvips-native-stills-not-imagemagick.md), [ADR 011](./resources/architecture/011-native-media-adapters-and-ffmpeg-license.md)). Every dependency is pinned.

## 18. Scale-to-Zero

**Purpose:** Ensure the application can scale to zero when idle.

- **No server required:** the desktop app is fully usable with every service off.
- **Static UIs:** every UI is a static build served by its host, with no server-side rendering.
- **Ciphertext-only hosted storage:** the optional paid service stores ciphertext in S3-compatible object storage and runs no always-on processing of private content ([ADR 012](./resources/architecture/012-s3-compatible-ciphertext-storage.md), [ADR 032](./resources/architecture/032-hosting-and-privacy-model.md)).
- **One process per server:** a self-run `filesquid-server` is a single process serving the API and the static UI.

## 19. Local-First

**Purpose:** Describe how the app works offline, prioritizes local data, and syncs data when back online.

- **OCFL on disk is the source of truth:** the operational database and search indexes are rebuildable from it ([ADR 010](./resources/architecture/010-ocfl-1-1-dual-profile.md), [ADR 023](./resources/architecture/023-filesquid-ocfl-engine-and-parquet.md)).
- **Fully offline:** the desktop app works without a connection. Backup and sync are optional additions, and sync is never treated as a backup.
- **Portable libraries:** owners can inspect and copy a library with ordinary file tools and reconstruct it without the app.
- **Safe resumption:** encrypted backup and sync resume after interruption. Conflicting writers rebase or report a conflict, never silently overwrite ([ADR 016](./resources/architecture/016-rust-ocfl-library.md)).

## 20. Zero-Friction Setup

**Purpose:** Guarantee that the application is as easy as possible to install and run locally, minimizing "time to first value" for new developers and users.

- **Automated Initialization:** If the application requires a local database, the initialization script MUST automatically check if it exists on startup. If not, it MUST seamlessly create the database, build the required schema/tables, and seed initial data without requiring manual CLI commands from the user.
- **Interactive Auth Prompts:** If the application requires API keys or authentication secrets, the application MUST automatically detect if they are missing on startup and interactively prompt the user for them (or generate a `.env` template automatically) rather than simply crashing with a stack trace.
- **Create Installers:** Whenever possible, create installers for the application that can be used to install the application on a developer and production system. 

## 21. Keep code linear
No deeply nested conditionals and early-exit chains. No hidden jumps. Keep the logic flat at one level as much as possible. Code should read top to bottom like a well-constructed argument — premise, reasoning, conclusion.

##  22. Bound every loop
Ensure there are maximum caps for polling loops, retry logic, and recursive crawlers. Provide a safe path forward for when the cap is hit.

## 23. Close every resource you open
Every resource you open, you must close. Every connection you borrow, you must return. Resource lifetime must be declared, not assumed — and it must be correct on the error path too, not just the happy one. Don’t acquire what you can’t account for. Follow the code through every exit path and confirm it closes what it opened.

## 24. One function, one job
Each function should do exactly one thing — small enough to be describable in a sentence without the word “and.” Do not generate monolithic functions that do many things at once because you're optimizing for task completion. You should optimize for maintainability. Apply decomposition upfront, not as a refactoring afterthought. No function longer than 60 lines.

## 25. Every exception is caught
Every error must be logged, raised, or explicitly returned. You cannot generate empty catch blocks or unchecked return values.

## 26. Clear separation of side-effect operations
I/O, mutations, and network calls should be named and obvious at the call site. Not hidden inside helpers, not wrapped in abstractions with meaningless names, not multiple layers deep in a chain of decorators. If a function writes to a database, its name and call site should make that clear. Do not bury API calls inside what appear to be utility functions. Ensure you have a clear structural separation between pure computation and side-effectful operations.

---
© Pieter Van Garderen (2026)