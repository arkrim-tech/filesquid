# Implementation Plan Checklist: FileSquid

<!-- mvp-progress:start -->
![MVP progress: 55% complete](./assets/mvp-progress.svg)

**MVP progress: 55%**, about 536 of 968 estimated hours. 15 of 28 tasks are done and 6 more are under way. Each segment of the bar is one task below, as wide as its estimated hours in the implementation plan; an open task fills by the share of its module rows that are ticked. Below the bar, In progress lists the open tasks being worked on (global tasks with some but not all module rows ticked, and tasks with an open pull request) and Up next lists the next five new module tasks whose listed dependencies are done or in progress. Regenerate with `python3 scripts/docs/update_mvp_progress.py` after ticking boxes or opening a pull request; the check run by `scripts/verify_docs.sh` does not call GitHub and uses the pull requests recorded in the block. In a clone that registers the `mvp-progress` git merge driver (see the README), merging two pull requests that both redraw it merges their ticks and redraws it when it can, and otherwise leaves a conflict.

**In progress**

- [Task 0.5](./implementation-plan.md#task-0.5) Desktop catalog, jobs, and OCFL coordinator: 5 of 8 module rows ticked
    - [DE-0.4](../apps/filesquid/crates/desktop/resources/plan.md#phase-f0-catalog-jobs-ocfl-coordinator-global-task-05) Durable jobs and checkpoint-on-quit ([PR #95](https://github.com/arkrim-tech/filesquid/pull/95))
    - [DE-0.5](../apps/filesquid/crates/desktop/resources/plan.md#phase-f0-catalog-jobs-ocfl-coordinator-global-task-05) Bounded media workers (libvips stills)
    - [DE-0.8](../apps/filesquid/crates/desktop/resources/plan.md#phase-f0-catalog-jobs-ocfl-coordinator-global-task-05) Native-processing library boundary (`filesquid-native`)
- [Task FS-1.3](./implementation-plan.md#task-fs-1.3) Metadata properties and the import record through the API: 3 of 5 module rows ticked
    - [API-F1.4](../apps/filesquid/crates/api/resources/plan.md#phase-f1-the-api-global-tasks-fs-11fs-14-after-the-mvp-api-f18-in-fs-15-api-f19-in-fs-310-and-api-f15-in-fs-c1) Metadata property routes
    - [API-F1.13](../apps/filesquid/crates/api/resources/plan.md#phase-f1-the-api-global-tasks-fs-11fs-14-after-the-mvp-api-f18-in-fs-15-api-f19-in-fs-310-and-api-f15-in-fs-c1) Display names and download names
- [Task FS-1.4](./implementation-plan.md#task-fs-1.4) Jobs, events and the authorisation layer: 1 of 2 module rows ticked
    - [API-F1.6](../apps/filesquid/crates/api/resources/plan.md#phase-f1-the-api-global-tasks-fs-11fs-14-after-the-mvp-api-f18-in-fs-15-api-f19-in-fs-310-and-api-f15-in-fs-c1) Job and event routes
- [Task 1.1](./implementation-plan.md#task-1.1) Independent local library create, open, copy, and recover: 5 of 8 module rows ticked
    - [DE-1.1](../apps/filesquid/crates/desktop/resources/plan.md#phases-f2-i1-and-f5-library-import-gallery-organize-and-setup-global-tasks-1116) Create, open, copy, rebuild library
    - [UI-1.1](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) Library create/open/recover UI
    - [UI-F2.14](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) OCFL validation findings in the Monitor
- [Task FS-2.2](./implementation-plan.md#task-fs-2.2) Drive-style browser and sidebar panels: 1 of 3 module rows ticked
    - [UI-F2.3](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) Details and Versions panels
    - [UI-F2.4](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) `<fs-metadata-panel>`
- [Task FS-2.8](./implementation-plan.md#task-fs-2.8) PROV-O history ([PR #91](https://github.com/arkrim-tech/filesquid/pull/91)): 0 of 8 module rows ticked
    - [CO-0.11](../apps/filesquid/crates/core/resources/plan.md#phase-f2-prov-o-history-contracts-global-task-fs-28) PROV-O history contract types
    - [IO-1.5](../apps/filesquid/crates/ocfl/resources/plan.md#phase-f2-version-bound-files-for-prov-o-history-global-task-fs-28) Version-bound logical files in the change model
    - [PV-0.1](../apps/filesquid/crates/prov/resources/plan.md#phase-f2-prov-o-history-engine-global-task-fs-28) Crate, PROV-O model and profile `filesquid-prov/1`
    - [MD-0.9](../apps/filesquid/crates/metadata/resources/plan.md#phase-f2-commit-options-for-prov-o-history-global-task-fs-28) Commit options on the `ocfl` store
    - [PV-0.2](../apps/filesquid/crates/prov/resources/plan.md#phase-f2-prov-o-history-engine-global-task-fs-28) `ocfl` feature: records in OCFL objects and reconstruction
    - [API-F2.8](../apps/filesquid/crates/api/resources/plan.md#phase-f2-prov-o-history-global-task-fs-28) Journal and commit linkage for PROV-O history
    - [DE-0.10](../apps/filesquid/crates/desktop/resources/plan.md#phase-f0-catalog-jobs-ocfl-coordinator-global-task-05) Installation UUID, native journal columns, outbox events and coordinator commit linkage
    - [API-F2.9](../apps/filesquid/crates/api/resources/plan.md#phase-f2-prov-o-history-global-task-fs-28) PROV-O records, history routes and backfill

**Up next** (the next 5 new module tasks, once their listed dependencies are done or in progress)

- [DE-1.2](../apps/filesquid/crates/desktop/resources/plan.md#phases-f2-i1-and-f5-library-import-gallery-organize-and-setup-global-tasks-1116) File and folder import (part of [Task 1.2](./implementation-plan.md#task-1.2))
- [UI-1.2](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) Import UI with job progress (part of [Task 1.2](./implementation-plan.md#task-1.2))
- [API-F2.2](../apps/filesquid/crates/api/resources/plan.md#phase-f2-activity-and-health-monitoring-global-task-fs-24-and-drive-parity-operations-global-task-fs-25-api-f24-in-global-task-fs-27-after-the-mvp) View operation, preferences, storage use, copy, move and ZIP download (part of [Task FS-2.5](./implementation-plan.md#task-fs-2.5))
- [API-F2.5](../apps/filesquid/crates/api/resources/plan.md#phase-f2-activity-and-health-monitoring-global-task-fs-24-and-drive-parity-operations-global-task-fs-25-api-f24-in-global-task-fs-27-after-the-mvp) Version listing, labels, restore and history size (part of [Task FS-2.5](./implementation-plan.md#task-fs-2.5))
- [API-F2.10](../apps/filesquid/crates/api/resources/plan.md#phase-f2-prov-o-history-global-task-fs-28) Client events, journal query filters and journal export (part of [Task FS-2.9](./implementation-plan.md#task-fs-2.9))

Generated 2026-10-20T04:15:59Z (RFC 3339, UTC) from the ticked boxes, the module plans and the open pull requests at that time.
<!-- generated-at: 2026-10-20T04:15:59Z -->
<!-- in-progress-prs: DE-0.4=#95,FS-2.8=#91 -->
<!-- mvp-progress:end -->

This is a short, ordered view of [implementation-plan.md](./implementation-plan.md). Each numbered item is a global task in the order the dev team expects to finish it, and the nested items are the module tasks it dispatches, taken from each module's `resources/plan.md`. Task codes link to the full task description. A box is ticked when the matching box is ticked in the source plan.

Order follows the **Dependencies** fields and the [release sequence](./implementation-plan.md#release-sequence-assumptions-do-not-reorder) of the implementation plan; where the plan allows tasks to run in parallel, the order shown is the dev team's current dispatch order. Finished tasks come first in each release, in the order they were finished, even when one was finished ahead of a listed dependency (Task FS-2.4 was delivered before Tasks FS-2.1, 0.5 and FS-1.4); open tasks follow in dependency order. The implementation plan stays authoritative: if the two disagree, the implementation plan wins and this file is corrected.

## MVP

FileSquid MVP (early access) on the macOS desktop: phases F0 to F3, closed by gate Task FS-3.9 (15 of 28 done). PROV-O history (Task FS-2.8) and the durable journal with the Logs view and file panel tabs (Task FS-2.9) are in the MVP and do not depend on the `c2pa` module ([ADR 040](./architecture/040-prov-o-history.md)). C2PA is not in the MVP ([ADR 037](./architecture/037-c2pa-in-release-2.md)).

1. [x] [Task FS-0.0](./implementation-plan.md#task-fs-0.0) FileSquid product switch
    - [x] [API-F0.1](../apps/filesquid/crates/api/resources/plan.md#phase-f0-crate-health-route-and-transport-spike-global-task-fs-00) Health router, contract seed and transport spike S1
    - [x] [SV-F0.1](../apps/filesquid/crates/server/resources/plan.md#phase-f0-filesquid-server-host-global-task-fs-00) Mount the API router and serve the selected UI
    - [x] [DE-F0.1](../apps/filesquid/crates/desktop/resources/plan.md#phase-f0-api-through-the-custom-uri-scheme-global-task-fs-00) Serve `filesquid-api` through the custom URI scheme
    - [x] [UI-F0.1](../apps/filesquid/ui/resources/plan.md#phase-f0-shell-and-white-label-mechanism-global-task-fs-00) FileSquidClient and `<fs-app-shell>`
    - [x] [UI-F0.2](../apps/filesquid/ui/resources/plan.md#phase-f0-shell-and-white-label-mechanism-global-task-fs-00) White-label mechanism and completeness tests
2. [x] [Task 0.1](./implementation-plan.md#task-0.1) Portable domain contracts and terminology
    - [x] [CO-0.1](../apps/filesquid/crates/core/resources/plan.md#phase-f0-foundation-contracts-global-tasks-01-02) Opaque identifiers and terminology types
    - [x] [CO-0.2](../apps/filesquid/crates/core/resources/plan.md#phase-f0-foundation-contracts-global-tasks-01-02) File, resource, version, and recipe types
    - [x] [CO-0.3](../apps/filesquid/crates/core/resources/plan.md#phase-f0-foundation-contracts-global-tasks-01-02) Collection, virtual collection, and membership types
    - [x] [CO-0.4](../apps/filesquid/crates/core/resources/plan.md#phase-f0-foundation-contracts-global-tasks-01-02) Health status shared by hosts
3. [x] [Task 0.2](./implementation-plan.md#task-0.2) Command, job, crypto, and provenance contracts
    - [x] [CO-0.5](../apps/filesquid/crates/core/resources/plan.md#phase-f0-foundation-contracts-global-tasks-01-02) Command/query ports, operation IDs, and errors
    - [x] [CO-0.6](../apps/filesquid/crates/core/resources/plan.md#phase-f0-foundation-contracts-global-tasks-01-02) Job checkpoint protocol types
    - [x] [CO-0.7](../apps/filesquid/crates/core/resources/plan.md#phase-f0-foundation-contracts-global-tasks-01-02) Cryptographic envelope types
    - [x] [CO-0.8](../apps/filesquid/crates/core/resources/plan.md#phase-f0-foundation-contracts-global-tasks-01-02) C2PA and PROV-O record types
    - [x] [CO-0.9](../apps/filesquid/crates/core/resources/plan.md#phase-f0-foundation-contracts-global-tasks-01-02) Capability register types
    - [x] [CO-0.10](../apps/filesquid/crates/core/resources/plan.md#phase-f0-foundation-contracts-global-tasks-01-02) Isolation and rustdoc gate
4. [x] [Task 0.13](./implementation-plan.md#task-0.13) filesquid-metadata register and isolation
    - [x] [MD-0.1](../apps/filesquid/crates/metadata/resources/plan.md#phase-f0-register-global-task-013) `/terms/` register and isolation
5. [x] [Task 0.14](./implementation-plan.md#task-0.14) filesquid-metadata CLI and CSV
    - [x] [MD-0.3](../apps/filesquid/crates/metadata/resources/plan.md#phase-f0-stand-alone-cli-and-csv-global-tasks-014-016) CLI and CSV headers
6. [x] [Task FS-0.1](./implementation-plan.md#task-fs-0.1) One `fsquid` CLI
    - [x] [CLI-0.1](../apps/filesquid/crates/cli/resources/plan.md#phase-f0-one-fsquid-cli-global-task-fs-01) `fsquid` binary and command routing
7. [x] [Task 0.16](./implementation-plan.md#task-0.16) filesquid-metadata JSON sidecar
    - [x] [MD-0.5](../apps/filesquid/crates/metadata/resources/plan.md#phase-f0-stand-alone-cli-and-csv-global-tasks-014-016) JSON sidecar interchange
8. [x] [Task FS-2.4](./implementation-plan.md#task-fs-2.4) Activity journal, process registry and Monitor dashboard (finished early, ahead of its listed dependencies)
    - [x] [API-F2.1](../apps/filesquid/crates/api/resources/plan.md#phase-f2-activity-and-health-monitoring-global-task-fs-24-and-drive-parity-operations-global-task-fs-25-api-f24-in-global-task-fs-27-after-the-mvp) Activity journal, process registry, health and admin routes
    - [x] [UI-F2.7](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) Monitor dashboard
9. [x] [Task FS-1.1](./implementation-plan.md#task-fs-1.1) API contract, router and transports
    - [x] [API-F1.1](../apps/filesquid/crates/api/resources/plan.md#phase-f1-the-api-global-tasks-fs-11fs-14-after-the-mvp-api-f18-in-fs-15-api-f19-in-fs-310-and-api-f15-in-fs-c1) Versioned contract and generated TypeScript types
    - [x] [API-F1.2](../apps/filesquid/crates/api/resources/plan.md#phase-f1-the-api-global-tasks-fs-11fs-14-after-the-mvp-api-f18-in-fs-15-api-f19-in-fs-310-and-api-f15-in-fs-c1) Routes and the event channel
    - [x] [DE-F1.1](../apps/filesquid/crates/desktop/resources/plan.md#phase-f1-desktop-transport-global-task-fs-11) Per-launch token, event channel and ranged reads
10. [x] [Task 0.3](./implementation-plan.md#task-0.3) filesquid-ocfl read, validate, and git guard
    - [x] [IO-0.1](../apps/filesquid/crates/ocfl/resources/plan.md#phase-f0-read-validate-write-global-tasks-03-04) Storage backend trait
    - [x] [IO-0.2](../apps/filesquid/crates/ocfl/resources/plan.md#phase-f0-read-validate-write-global-tasks-03-04) Git guard
    - [x] [IO-0.3](../apps/filesquid/crates/ocfl/resources/plan.md#phase-f0-read-validate-write-global-tasks-03-04) SHA-256 digest and inventory types
    - [x] [IO-0.4](../apps/filesquid/crates/ocfl/resources/plan.md#phase-f0-read-validate-write-global-tasks-03-04) Read and browse
    - [x] [IO-0.5](../apps/filesquid/crates/ocfl/resources/plan.md#phase-f0-read-validate-write-global-tasks-03-04) Structural validation
11. [x] [Task 0.10](./implementation-plan.md#task-0.10) Private-client library (ADR 001)
    - [x] [CL-0.1](../apps/filesquid/crates/api/resources/plan.md#phase-f1-application-layer-global-task-010-cl-04-in-global-task-011-phase-f4-cl-05-in-global-task-012-phase-i1) Confirm crate name, SRS, and graph guards
    - [x] [CL-0.2](../apps/filesquid/crates/api/resources/plan.md#phase-f1-application-layer-global-task-010-cl-04-in-global-task-011-phase-f4-cl-05-in-global-task-012-phase-i1) Library domain commands and queries
    - [x] [CL-0.3](../apps/filesquid/crates/api/resources/plan.md#phase-f1-application-layer-global-task-010-cl-04-in-global-task-011-phase-f4-cl-05-in-global-task-012-phase-i1) Replica, cache identity, and pending mutations
12. [x] [Task 0.4](./implementation-plan.md#task-0.4) filesquid-ocfl write and logical-state enumerator
    - [x] [IO-0.6](../apps/filesquid/crates/ocfl/resources/plan.md#phase-f0-read-validate-write-global-tasks-03-04) Local write order and unpublished vN
    - [x] [IO-0.7](../apps/filesquid/crates/ocfl/resources/plan.md#phase-f0-read-validate-write-global-tasks-03-04) Generic object payload
    - [x] [IO-0.8](../apps/filesquid/crates/ocfl/resources/plan.md#phase-f0-read-validate-write-global-tasks-03-04) Opaque bytes; dual profile is a caller
    - [x] [IO-0.9](../apps/filesquid/crates/ocfl/resources/plan.md#phase-f0-read-validate-write-global-tasks-03-04) Enumerate committed logical state
    - [x] [IO-0.10](../apps/filesquid/crates/ocfl/resources/plan.md#phase-f0-read-validate-write-global-tasks-03-04) Forward-delta change model
13. [x] [Task 0.15](./implementation-plan.md#task-0.15) filesquid-metadata WASM build and OCFL-sourced extraction
    - [x] [MD-0.4](../apps/filesquid/crates/metadata/resources/plan.md#phase-f0-webassembly-build-global-task-015) WASM build and OCFL-sourced extraction
14. [x] [Task FS-1.2](./implementation-plan.md#task-fs-1.2) File operations through the API
    - [x] [IO-1.2](../apps/filesquid/crates/ocfl/resources/plan.md#phase-f1f2-view-api-and-file-browser-global-tasks-fs-12-19-superseded) UI view API
    - [x] [API-F1.3](../apps/filesquid/crates/api/resources/plan.md#phase-f1-the-api-global-tasks-fs-11fs-14-after-the-mvp-api-f18-in-fs-15-api-f19-in-fs-310-and-api-f15-in-fs-c1) File operation routes
15. [x] [Task FS-2.1](./implementation-plan.md#task-fs-2.1) Default UI shell, host adapter, tokens and accessibility
    - [x] [UI-0.1](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) Host adapter (HTTP vs custom URI scheme)
    - [x] [UI-0.2](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) Official branding and light/dark tokens
    - [x] [UI-0.5](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) Accessibility contracts in the shell
    - [x] [UI-F2.1a](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) States, and the navigation tree and breadcrumb elements
    - [x] [API-F1.10](../apps/filesquid/crates/api/resources/plan.md#phase-f2-host-descriptor-and-navigation-listing-for-the-default-ui-global-task-fs-21) Host descriptor route and change event
    - [x] [API-F1.11](../apps/filesquid/crates/api/resources/plan.md#phase-f2-host-descriptor-and-navigation-listing-for-the-default-ui-global-task-fs-21) Navigation listing route
    - [x] [UI-F2.1b](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) Host privacy indicator and live navigation data
16. [ ] [Task 0.5](./implementation-plan.md#task-0.5) Desktop catalog, jobs, and OCFL coordinator
    - [x] [DE-0.1](../apps/filesquid/crates/desktop/resources/plan.md#phase-f0-catalog-jobs-ocfl-coordinator-global-task-05) Capability expansion (deny-by-default)
    - [x] [DE-0.2](../apps/filesquid/crates/desktop/resources/plan.md#phase-f0-catalog-jobs-ocfl-coordinator-global-task-05) Per-library SQLite catalog
    - [x] [DE-0.3](../apps/filesquid/crates/desktop/resources/plan.md#phase-f0-catalog-jobs-ocfl-coordinator-global-task-05) Intent → OCFL commit → receipt reconcile
    - [ ] [DE-0.4](../apps/filesquid/crates/desktop/resources/plan.md#phase-f0-catalog-jobs-ocfl-coordinator-global-task-05) Durable jobs and checkpoint-on-quit
    - [ ] [DE-0.5](../apps/filesquid/crates/desktop/resources/plan.md#phase-f0-catalog-jobs-ocfl-coordinator-global-task-05) Bounded media workers (libvips stills)
    - [x] [DE-0.6](../apps/filesquid/crates/desktop/resources/plan.md#phase-f0-catalog-jobs-ocfl-coordinator-global-task-05) Stdout JSON logging
    - [ ] [DE-0.8](../apps/filesquid/crates/desktop/resources/plan.md#phase-f0-catalog-jobs-ocfl-coordinator-global-task-05) Native-processing library boundary (`filesquid-native`)
    - [x] [DE-0.9](../apps/filesquid/crates/desktop/resources/plan.md#phase-f0-catalog-jobs-ocfl-coordinator-global-task-05) Journal every refusal in the desktop shell
17. [ ] [Task FS-1.3](./implementation-plan.md#task-fs-1.3) Metadata properties and the import record through the API
    - [x] [MD-0.2](../apps/filesquid/crates/metadata/resources/plan.md#phase-f1-persist-through-ocfl-and-namespace-registry-global-task-fs-13) Commit statements through filesquid-ocfl
    - [x] [MD-0.7](../apps/filesquid/crates/metadata/resources/plan.md#phase-f1-persist-through-ocfl-and-namespace-registry-global-task-fs-13) Namespace registry and extension statements
    - [x] [MD-0.8](../apps/filesquid/crates/metadata/resources/plan.md#phase-f1-persist-through-ocfl-and-namespace-registry-global-task-fs-13) Import record
    - [ ] [API-F1.4](../apps/filesquid/crates/api/resources/plan.md#phase-f1-the-api-global-tasks-fs-11fs-14-after-the-mvp-api-f18-in-fs-15-api-f19-in-fs-310-and-api-f15-in-fs-c1) Metadata property routes
    - [ ] [API-F1.13](../apps/filesquid/crates/api/resources/plan.md#phase-f1-the-api-global-tasks-fs-11fs-14-after-the-mvp-api-f18-in-fs-15-api-f19-in-fs-310-and-api-f15-in-fs-c1) Display names and download names
18. [ ] [Task FS-1.4](./implementation-plan.md#task-fs-1.4) Jobs, events and the authorisation layer
    - [ ] [API-F1.6](../apps/filesquid/crates/api/resources/plan.md#phase-f1-the-api-global-tasks-fs-11fs-14-after-the-mvp-api-f18-in-fs-15-api-f19-in-fs-310-and-api-f15-in-fs-c1) Job and event routes
    - [x] [API-F1.7](../apps/filesquid/crates/api/resources/plan.md#phase-f1-the-api-global-tasks-fs-11fs-14-after-the-mvp-api-f18-in-fs-15-api-f19-in-fs-310-and-api-f15-in-fs-c1) Authentication and authorisation
19. [ ] [Task 0.8](./implementation-plan.md#task-0.8) Test reporting, capability register, and foundation evidence
    - [x] [UI-0.6](../apps/filesquid/ui/resources/plan.md#phase-f0-test-reporting-global-task-08) Playwright production suite and Allure
20. [ ] [Task 1.1](./implementation-plan.md#task-1.1) Independent local library create, open, copy, and recover
    - [x] [CL-1.1](../apps/filesquid/crates/api/resources/plan.md#phases-f2-i1-and-f5-application-meaning-global-tasks-11-14-18) Local library create/open/recover commands
    - [x] [API-F1.12](../apps/filesquid/crates/api/resources/plan.md#phase-f2-library-and-repair-routes-global-task-11) Library and repair routes
    - [ ] [DE-1.1](../apps/filesquid/crates/desktop/resources/plan.md#phases-f2-i1-and-f5-library-import-gallery-organize-and-setup-global-tasks-1116) Create, open, copy, rebuild library
    - [ ] [UI-1.1](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) Library create/open/recover UI
    - [x] [IO-1.1](../apps/filesquid/crates/ocfl/resources/plan.md#phase-f2-independent-recovery-global-task-11) Ordinary-tool copy and independent recovery fixtures
    - [x] [IO-1.4](../apps/filesquid/crates/ocfl/resources/plan.md#phase-f2-independent-recovery-global-task-11) Version-inventory repair for heads without one (W010)
    - [x] [API-F2.7](../apps/filesquid/crates/api/resources/plan.md#phase-f2-activity-and-health-monitoring-global-task-fs-24-and-drive-parity-operations-global-task-fs-25-api-f24-in-global-task-fs-27-after-the-mvp) OCFL validation findings in health and the Monitor
    - [ ] [UI-F2.14](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) OCFL validation findings in the Monitor
21. [ ] [Task FS-2.2](./implementation-plan.md#task-fs-2.2) Drive-style browser and sidebar panels
    - [x] [UI-F2.2](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) `<fs-file-browser>`
    - [ ] [UI-F2.3](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) Details and Versions panels
    - [ ] [UI-F2.4](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) `<fs-metadata-panel>`
22. [ ] [Task 1.2](./implementation-plan.md#task-1.2) Safe file and folder import with durable jobs
    - [ ] [DE-1.2](../apps/filesquid/crates/desktop/resources/plan.md#phases-f2-i1-and-f5-library-import-gallery-organize-and-setup-global-tasks-1116) File and folder import
    - [ ] [UI-1.2](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) Import UI with job progress
23. [ ] [Task FS-2.5](./implementation-plan.md#task-fs-2.5) Drive-parity views, search and version restore
    - [ ] [API-F2.2](../apps/filesquid/crates/api/resources/plan.md#phase-f2-activity-and-health-monitoring-global-task-fs-24-and-drive-parity-operations-global-task-fs-25-api-f24-in-global-task-fs-27-after-the-mvp) View operation, preferences, storage use, copy, move and ZIP download
    - [ ] [API-F2.3](../apps/filesquid/crates/api/resources/plan.md#phase-f2-activity-and-health-monitoring-global-task-fs-24-and-drive-parity-operations-global-task-fs-25-api-f24-in-global-task-fs-27-after-the-mvp) Search operation and operator grammar
    - [ ] [API-F2.5](../apps/filesquid/crates/api/resources/plan.md#phase-f2-activity-and-health-monitoring-global-task-fs-24-and-drive-parity-operations-global-task-fs-25-api-f24-in-global-task-fs-27-after-the-mvp) Version listing, labels, restore and history size
    - [ ] [UI-F2.8](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) Drive-style views, sorting, selection and preferences
    - [ ] [UI-F2.9](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) Global search bar and filter chips
    - [ ] [UI-F2.11](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) Versions panel labels, download and history size
24. [ ] [Task FS-2.8](./implementation-plan.md#task-fs-2.8) PROV-O history
    - [ ] [CO-0.11](../apps/filesquid/crates/core/resources/plan.md#phase-f2-prov-o-history-contracts-global-task-fs-28) PROV-O history contract types
    - [ ] [IO-1.5](../apps/filesquid/crates/ocfl/resources/plan.md#phase-f2-version-bound-files-for-prov-o-history-global-task-fs-28) Version-bound logical files in the change model
    - [ ] [PV-0.1](../apps/filesquid/crates/prov/resources/plan.md#phase-f2-prov-o-history-engine-global-task-fs-28) Crate, PROV-O model and profile `filesquid-prov/1`
    - [ ] [MD-0.9](../apps/filesquid/crates/metadata/resources/plan.md#phase-f2-commit-options-for-prov-o-history-global-task-fs-28) Commit options on the `ocfl` store
    - [ ] [PV-0.2](../apps/filesquid/crates/prov/resources/plan.md#phase-f2-prov-o-history-engine-global-task-fs-28) `ocfl` feature: records in OCFL objects and reconstruction
    - [ ] [API-F2.8](../apps/filesquid/crates/api/resources/plan.md#phase-f2-prov-o-history-global-task-fs-28) Journal and commit linkage for PROV-O history
    - [ ] [DE-0.10](../apps/filesquid/crates/desktop/resources/plan.md#phase-f0-catalog-jobs-ocfl-coordinator-global-task-05) Installation UUID, native journal columns, outbox events and coordinator commit linkage
    - [ ] [API-F2.9](../apps/filesquid/crates/api/resources/plan.md#phase-f2-prov-o-history-global-task-fs-28) PROV-O records, history routes and backfill
25. [ ] [Task FS-2.9](./implementation-plan.md#task-fs-2.9) Durable journal, Logs view and file panel tabs
    - [ ] [API-F2.10](../apps/filesquid/crates/api/resources/plan.md#phase-f2-prov-o-history-global-task-fs-28) Client events, journal query filters and journal export
    - [ ] [API-F2.11](../apps/filesquid/crates/api/resources/plan.md#phase-f2-prov-o-history-global-task-fs-28) Durable journal: per-commit entries and sealing service
    - [ ] [CL-1.9](../apps/filesquid/crates/api/resources/plan.md#phases-f2-i1-and-f5-application-meaning-global-tasks-11-14-18) Journal rebuild in the rebuild-from-files command
    - [ ] [DE-0.11](../apps/filesquid/crates/desktop/resources/plan.md#phase-f0-catalog-jobs-ocfl-coordinator-global-task-05) Host journal sealing and journal rebuild
    - [ ] [UI-F2.15](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) File panel tabs: File info, Description and Provenance
    - [ ] [UI-F2.16](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) Logs view and journal export
26. [ ] [Task FS-3.1](./implementation-plan.md#task-fs-3.1) macOS desktop installer (early access)
    - [ ] [DE-F3.1](../apps/filesquid/crates/desktop/resources/plan.md#phase-f3-installers-and-performance-global-tasks-fs-31-fs-34-and-after-the-mvp-fs-311) macOS early-access DMG and the FileSquid overlay
27. [ ] [Task FS-3.4](./implementation-plan.md#task-fs-3.4) MVP performance gate (macOS desktop)
    - [ ] [DE-F3.2](../apps/filesquid/crates/desktop/resources/plan.md#phase-f3-installers-and-performance-global-tasks-fs-31-fs-34-and-after-the-mvp-fs-311) Packaged desktop file-browsing performance
    - [ ] [UI-F3.1](../apps/filesquid/ui/resources/plan.md#phase-f3-mvp-performance-global-task-fs-34) Production-build browsing performance
28. [ ] [Task FS-3.9](./implementation-plan.md#task-fs-3.9) FileSquid MVP gate (macOS desktop)

## Release1

FileSquid release 1: server host, accounts and sharing, Linux and Windows installers, signing, the published `ocfl` and `metadata` crates and the `@filesquid/ui` package (phase F3R), closed by gate Task FS-3.5 and followed by the ImageSquid extraction in Task FS-3.8 (0 of 14 done). C2PA signing, edits, scans and crate publishing moved to phase C.

29. [ ] [Task FS-3.10](./implementation-plan.md#task-fs-3.10) Server host: HTTP binding, storage root, accounts and groups
    - [ ] [SV-F1.1](../apps/filesquid/crates/server/resources/plan.md#phase-f3r-multi-user-host-global-tasks-fs-310-and-fs-27-not-in-the-macos-desktop-mvp) HTTP and event-channel transport
    - [ ] [SV-F1.3](../apps/filesquid/crates/server/resources/plan.md#phase-f3r-multi-user-host-global-tasks-fs-310-and-fs-27-not-in-the-macos-desktop-mvp) Storage root configuration
    - [ ] [API-F1.9](../apps/filesquid/crates/api/resources/plan.md#phase-f1-the-api-global-tasks-fs-11fs-14-after-the-mvp-api-f18-in-fs-15-api-f19-in-fs-310-and-api-f15-in-fs-c1) Server accounts, groups and collection permissions in the authorisation layer
    - [ ] [SV-F1.2](../apps/filesquid/crates/server/resources/plan.md#phase-f3r-multi-user-host-global-tasks-fs-310-and-fs-27-not-in-the-macos-desktop-mvp) Accounts, groups, sessions and collection permissions
    - [ ] [BE-0.6](../apps/filesquid/crates/server/resources/plan.md#phase-f4-hosted-service-isolation-and-operational-foundation-global-task-06-be-06-in-global-task-fs-310) Allure reporting and static SPA serving
30. [ ] [Task FS-1.5](./implementation-plan.md#task-fs-1.5) Distribution manifest and module switches
    - [ ] [API-F1.8](../apps/filesquid/crates/api/resources/plan.md#phase-f1-the-api-global-tasks-fs-11fs-14-after-the-mvp-api-f18-in-fs-15-api-f19-in-fs-310-and-api-f15-in-fs-c1) Distribution manifest and module switches
31. [ ] [Task FS-3.11](./implementation-plan.md#task-fs-3.11) Linux and Windows desktop installers
    - [ ] [DE-F3.3](../apps/filesquid/crates/desktop/resources/plan.md#phase-f3-installers-and-performance-global-tasks-fs-31-fs-34-and-after-the-mvp-fs-311) Linux and Windows installers, ImageSquid overlay
    - [ ] [UI-F3.3](../apps/filesquid/ui/resources/plan.md#phase-f3r-linux-media-playback-global-task-fs-311-after-the-mvp) MediaSource video player for WebKitGTK
32. [ ] [Task FS-3.7](./implementation-plan.md#task-fs-3.7) Publish `@filesquid/ui` and a versioned API contract
    - [ ] [API-F3.1](../apps/filesquid/crates/api/resources/plan.md#phase-f3r-versioned-api-contract-global-task-fs-37-after-the-mvp) Version and publish the API contract
    - [ ] [UI-F3.2](../apps/filesquid/ui/resources/plan.md#phase-f3r-package-publishing-global-task-fs-37-after-the-mvp) Publish `@filesquid/ui` as a versioned package
33. [ ] [Task FS-2.3](./implementation-plan.md#task-fs-2.3) Users, groups and sharing UI on filesquid-server
    - [ ] [UI-F2.6](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) Users, groups and sharing UI
34. [ ] [Task FS-2.6](./implementation-plan.md#task-fs-2.6) Virtual exhibits through the API and the default UI
    - [ ] [API-F2.6](../apps/filesquid/crates/api/resources/plan.md#phase-f2-activity-and-health-monitoring-global-task-fs-24-and-drive-parity-operations-global-task-fs-25-api-f24-in-global-task-fs-27-after-the-mvp) Virtual exhibit operations
    - [ ] [UI-F2.13](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) Virtual exhibit view
35. [ ] [Task 8.5](./implementation-plan.md#task-8.5) Native installer signing, updater, and update recovery
    - [ ] [DE-8.5](../apps/filesquid/crates/desktop/resources/plan.md#phases-i5i7-f3-l-and-v-projects-advanced-media-sharing-publication-installers-global-tasks-61-7x-8x-9x-101) Installer signing, updater, and update recovery
    - [ ] [DE-8.6](../apps/filesquid/crates/desktop/resources/plan.md#phases-i5i7-f3-l-and-v-projects-advanced-media-sharing-publication-installers-global-tasks-61-7x-8x-9x-101) Native-window capability regression suite
36. [ ] [Task FS-2.7](./implementation-plan.md#task-fs-2.7) Activity stream, comments and sharing
    - [ ] [API-F2.4](../apps/filesquid/crates/api/resources/plan.md#phase-f2-activity-and-health-monitoring-global-task-fs-24-and-drive-parity-operations-global-task-fs-25-api-f24-in-global-task-fs-27-after-the-mvp) User activity stream, per-file activity and comments
    - [ ] [SV-F1.4](../apps/filesquid/crates/server/resources/plan.md#phase-f3r-multi-user-host-global-tasks-fs-310-and-fs-27-not-in-the-macos-desktop-mvp) Shares, roles, links, ownership transfer and administrator switches
    - [ ] [UI-F2.10](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) `<fs-activity-panel>` and the activity view
    - [ ] [UI-F2.12](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) `<fs-sharing-panel>` and the shares view
37. [ ] [Task FS-3.3](./implementation-plan.md#task-fs-3.3) Publish module crates and CLIs
    - [ ] [IO-D.1](../apps/filesquid/crates/ocfl/resources/plan.md#phase-f3r-publish-the-crate-and-cli-global-task-fs-33-after-the-mvp-task-104-superseded) Version the public engine API
    - [ ] [IO-D.3](../apps/filesquid/crates/ocfl/resources/plan.md#phase-f3r-publish-the-crate-and-cli-global-task-fs-33-after-the-mvp-task-104-superseded) Publish the library crate
    - [ ] [IO-D.4](../apps/filesquid/crates/ocfl/resources/plan.md#phase-f3r-publish-the-crate-and-cli-global-task-fs-33-after-the-mvp-task-104-superseded) CLI and GitHub Release binaries
    - [ ] [MD-F3.1](../apps/filesquid/crates/metadata/resources/plan.md#phase-f3r-publish-the-crate-global-task-fs-33-after-the-mvp) Version and publish `filesquid-metadata`
    - [ ] [PV-F3.1](../apps/filesquid/crates/prov/resources/plan.md#phase-f3r-publish-the-crate-and-its-commands-global-task-fs-33-after-the-mvp) Version and publish `filesquid-prov`; `fsquid prov` commands
38. [ ] [Task FS-3.2](./implementation-plan.md#task-fs-3.2) Multi-user server container and documentation site
    - [ ] [SV-F3.1](../apps/filesquid/crates/server/resources/plan.md#phase-f3r-release-1-container-and-performance-global-tasks-fs-32-fs-313) Container image and Compose service
39. [ ] [Task FS-3.13](./implementation-plan.md#task-fs-3.13) Multi-user server load test
    - [ ] [SV-F3.2](../apps/filesquid/crates/server/resources/plan.md#phase-f3r-release-1-container-and-performance-global-tasks-fs-32-fs-313) Multi-user load test
40. [ ] [Task FS-3.6](./implementation-plan.md#task-fs-3.6) Optional OTLP export and reference observability profile
    - [ ] [SV-F3.3](../apps/filesquid/crates/server/resources/plan.md#phase-f3r-optional-otlp-export-global-task-fs-36-not-in-the-macos-desktop-mvp) Store-and-forward OTLP export and reference observability profile
41. [ ] [Task FS-3.5](./implementation-plan.md#task-fs-3.5) FileSquid release 1 gate
42. [ ] [Task FS-3.8](./implementation-plan.md#task-fs-3.8) Extract ImageSquid to its own repository

## Later

Everything after FileSquid release 1, in the phase order of the implementation plan (0 of 56 done). Phase A and phase L wait on the owner's order decision in Task 9.0.


### Encrypted backup and sync (phase F4)

43. [ ] [Task FS-4.1](./implementation-plan.md#task-fs-4.1) Ciphertext-only hosted service at `*.filesquid.tech`
    - [ ] [SV-F4.1](../apps/filesquid/crates/server/resources/plan.md#phase-f4-ciphertext-only-hosted-service-binary-global-task-fs-41) Hosted-service binary and dependency-graph gate
    - [ ] [SV-F4.2](../apps/filesquid/crates/server/resources/plan.md#phase-f4-ciphertext-only-hosted-service-binary-global-task-fs-41) `*.filesquid.tech` deployment
    - [ ] [API-F4.1](../apps/filesquid/crates/api/resources/plan.md#phase-f4-backup-and-sync-control-global-task-fs-41) Backup and sync control routes
44. [ ] [Task 0.6](./implementation-plan.md#task-0.6) Hosted mailbox isolation and operational schema
    - [ ] [BE-0.1](../apps/filesquid/crates/server/resources/plan.md#phase-f4-hosted-service-isolation-and-operational-foundation-global-task-06-be-06-in-global-task-fs-310) Isolation gate
    - [ ] [BE-0.2](../apps/filesquid/crates/server/resources/plan.md#phase-f4-hosted-service-isolation-and-operational-foundation-global-task-06-be-06-in-global-task-fs-310) Stdout JSON logs and Prometheus /metrics
    - [ ] [BE-0.3](../apps/filesquid/crates/server/resources/plan.md#phase-f4-hosted-service-isolation-and-operational-foundation-global-task-06-be-06-in-global-task-fs-310) PostgreSQL operational schema
    - [ ] [BE-0.4](../apps/filesquid/crates/server/resources/plan.md#phase-f4-hosted-service-isolation-and-operational-foundation-global-task-06-be-06-in-global-task-fs-310) Hosted-safe core path dependency
    - [ ] [BE-0.5](../apps/filesquid/crates/server/resources/plan.md#phase-f4-hosted-service-isolation-and-operational-foundation-global-task-06-be-06-in-global-task-fs-310) Versioned HTTPS command/query surface
45. [ ] [Task 5.1](./implementation-plan.md#task-5.1) Client encryption, key custody, and independent crypto vectors
    - [ ] [CO-5.1](../apps/filesquid/crates/core/resources/plan.md#phase-f4-protection-contracts-global-tasks-51-52) Key hierarchy and access-record types
    - [ ] [DE-5.1](../apps/filesquid/crates/desktop/resources/plan.md#phase-f4-keys-sync-hosted-ocfl-global-tasks-51-52-54-de-53-in-global-task-56-phase-i5) Key custody
    - [ ] [UI-5.1](../apps/filesquid/ui/resources/plan.md#phase-f4-protection-ui-global-tasks-5153) Unlock, key recovery, and lock/logout
46. [ ] [Task 0.11](./implementation-plan.md#task-0.11) Browser-to-canonical commit protocol
    - [ ] [CL-0.4](../apps/filesquid/crates/api/resources/plan.md#phase-f1-application-layer-global-task-010-cl-04-in-global-task-011-phase-f4-cl-05-in-global-task-012-phase-i1) Browser-to-canonical commit types
47. [ ] [Task 5.2](./implementation-plan.md#task-5.2) Personal sync, encrypted backup, and verified restore
    - [ ] [CO-5.2](../apps/filesquid/crates/core/resources/plan.md#phase-f4-protection-contracts-global-tasks-51-52) Sync operation, conflict, and tombstone types
    - [ ] [DE-5.2](../apps/filesquid/crates/desktop/resources/plan.md#phase-f4-keys-sync-hosted-ocfl-global-tasks-51-52-54-de-53-in-global-task-56-phase-i5) Sync and backup clients
    - [ ] [BE-5.1](../apps/filesquid/crates/server/resources/plan.md#phase-f4-accounts-sync-relay-snapshots-global-tasks-5255) Encrypted change relay and snapshot receipts
    - [ ] [UI-5.2](../apps/filesquid/ui/resources/plan.md#phase-f4-protection-ui-global-tasks-5153) Sync and backup status UI
    - [ ] [IO-5.1](../apps/filesquid/crates/ocfl/resources/plan.md#phase-f4-s3-compatible-ciphertext-backend-global-task-52) S3-compatible backend with CAS on inventory heads
48. [ ] [Task 5.3](./implementation-plan.md#task-5.3) Accounts, entitlements, and scoped job billing (F-032)
    - [ ] [BE-5.2](../apps/filesquid/crates/server/resources/plan.md#phase-f4-accounts-sync-relay-snapshots-global-tasks-5255) Accounts, entitlements, membership, and cancellation
    - [ ] [UI-5.3](../apps/filesquid/ui/resources/plan.md#phase-f4-protection-ui-global-tasks-5153) Accounts, entitlements, and job billing
49. [ ] [Task 5.4](./implementation-plan.md#task-5.4) Hosted ciphertext OCFL preservation
    - [ ] [DE-5.4](../apps/filesquid/crates/desktop/resources/plan.md#phase-f4-keys-sync-hosted-ocfl-global-tasks-51-52-54-de-53-in-global-task-56-phase-i5) Hosted OCFL via filesquid-ocfl S3 backend
    - [ ] [BE-5.3](../apps/filesquid/crates/server/resources/plan.md#phase-f4-accounts-sync-relay-snapshots-global-tasks-5255) Hosted object-store receipts (no OCFL writer)
50. [ ] [Task 5.5](./implementation-plan.md#task-5.5) Hosted-protection readiness close (C-2)
    - [ ] [BE-5.5](../apps/filesquid/crates/server/resources/plan.md#phase-f4-accounts-sync-relay-snapshots-global-tasks-5255) Hosted-protection readiness operational evidence
51. [ ] [Task FS-4.2](./implementation-plan.md#task-fs-4.2) Encrypted backup and sync release gate

### Content Credentials, release 2 (phase C)

C2PA moved out of the MVP and release 1 ([ADR 037](./architecture/037-c2pa-in-release-2.md)). CP-0.1, CP-0.2 and CP-0.3 were finished before or alongside the move.

52. [ ] [Task 0.17](./implementation-plan.md#task-0.17) filesquid-c2pa read and validate
    - [x] [CP-0.1](../apps/filesquid/crates/c2pa/resources/plan.md#phase-c-read-and-validate-global-task-017) Scaffold, isolation, crypto backend and oracle harness
    - [x] [CP-0.2](../apps/filesquid/crates/c2pa/resources/plan.md#phase-c-read-and-validate-global-task-017) JUMBF and CBOR manifest store parsing
    - [x] [CP-0.3](../apps/filesquid/crates/c2pa/resources/plan.md#phase-c-read-and-validate-global-task-017) Hard bindings and JPEG/PNG handlers
    - [ ] [CP-0.4](../apps/filesquid/crates/c2pa/resources/plan.md#phase-c-read-and-validate-global-task-017) Signatures, certificate chains, timestamps and revocation
    - [ ] [CP-0.5](../apps/filesquid/crates/c2pa/resources/plan.md#phase-c-read-and-validate-global-task-017) Validation report and `fsquid` read/verify
53. [ ] [Task 0.18](./implementation-plan.md#task-0.18) filesquid-c2pa manifest builder and prepared records
    - [ ] [CP-0.6](../apps/filesquid/crates/c2pa/resources/plan.md#phase-c-manifest-building-global-task-018) Manifest builder and prepared records
54. [ ] [Task FS-3.12](./implementation-plan.md#task-fs-3.12) filesquid-c2pa signing and embedding
    - [ ] [CP-0.7](../apps/filesquid/crates/c2pa/resources/plan.md#phase-c-signing-and-embedding-global-task-fs-312) Signer interface, signing and embedding
55. [ ] [Task 2.6](./implementation-plan.md#task-2.6) filesquid-c2pa edits, formats, scans and PROV-O
    - [ ] [CP-2.1](../apps/filesquid/crates/c2pa/resources/plan.md#phase-c-edits-formats-scans-and-prov-o-global-task-26) Edits, update manifests and redaction
    - [ ] [CP-2.2](../apps/filesquid/crates/c2pa/resources/plan.md#phase-c-edits-formats-scans-and-prov-o-global-task-26) Additional format handlers
    - [ ] [CP-2.3](../apps/filesquid/crates/c2pa/resources/plan.md#phase-c-edits-formats-scans-and-prov-o-global-task-26) Collection scans and reports
    - [ ] [CP-2.4](../apps/filesquid/crates/c2pa/resources/plan.md#phase-c-edits-formats-scans-and-prov-o-global-task-26) PROV-O projection feature
56. [ ] [Task FS-C.1](./implementation-plan.md#task-fs-c.1) Content Credentials through the API and the default UI
    - [ ] [API-F1.5](../apps/filesquid/crates/api/resources/plan.md#phase-f1-the-api-global-tasks-fs-11fs-14-after-the-mvp-api-f18-in-fs-15-api-f19-in-fs-310-and-api-f15-in-fs-c1) Content Credentials routes
    - [ ] [UI-F2.5](../apps/filesquid/ui/resources/plan.md#phase-f2-default-ui-global-tasks-fs-21-fs-22-fs-24-fs-25-11-12-after-the-mvp-ui-f26-in-global-task-fs-23-ui-f210-and-ui-f212-in-global-task-fs-27-ui-f213-in-global-task-fs-26-ui-f25-in-global-task-fs-c1-phase-c) `<fs-credentials-panel>`
57. [ ] [Task FS-C.2](./implementation-plan.md#task-fs-c.2) Publish `filesquid-c2pa`
    - [ ] [CP-F3.1](../apps/filesquid/crates/c2pa/resources/plan.md#phase-c-publish-the-crate-global-task-fs-c2) Version and publish `filesquid-c2pa`
58. [ ] [Task FS-C.3](./implementation-plan.md#task-fs-c.3) Content Credentials gate

### ImageSquid capabilities, part 1 (phase I1)

59. [ ] [Task 0.12](./implementation-plan.md#task-0.12) Foundation feasibility spikes (recorder, publisher, recipe/color)
    - [ ] [DE-0.7](../apps/filesquid/crates/desktop/resources/plan.md#phase-f0-catalog-jobs-ocfl-coordinator-global-task-05) Recorder, publisher, and recipe/color feasibility
    - [ ] [CL-0.5](../apps/filesquid/crates/api/resources/plan.md#phase-f1-application-layer-global-task-010-cl-04-in-global-task-011-phase-f4-cl-05-in-global-task-012-phase-i1) Recipe and color contracts (feasibility)
60. [ ] [Task 1.3](./implementation-plan.md#task-1.3) Paged catalog queries and derivative handles for gallery views
    - [ ] [DE-1.3](../apps/filesquid/crates/desktop/resources/plan.md#phases-f2-i1-and-f5-library-import-gallery-organize-and-setup-global-tasks-1116) Paged catalog queries and derivative handles
61. [ ] [Task 1.4](./implementation-plan.md#task-1.4) Organize commands: albums, favorites, search, activity, and deletion
    - [ ] [CL-1.4](../apps/filesquid/crates/api/resources/plan.md#phases-f2-i1-and-f5-application-meaning-global-tasks-11-14-18) Organize, search, and hide/delete semantics
    - [ ] [DE-1.4](../apps/filesquid/crates/desktop/resources/plan.md#phases-f2-i1-and-f5-library-import-gallery-organize-and-setup-global-tasks-1116) Organize commands (albums, favorites, hide/delete, search)
    - [ ] [UI-I1.1](../apps/filesquid/ui/resources/plan.md#phases-i1i7-media-capabilities-in-the-default-ui-global-tasks-14-23-31-41-61-75) Default-UI previews, selection forms and media-kind filters
62. [ ] [Task 1.5](./implementation-plan.md#task-1.5) Export, metadata extract, C2PA import records, and local OCFL recover
    - [ ] [DE-1.5](../apps/filesquid/crates/desktop/resources/plan.md#phases-f2-i1-and-f5-library-import-gallery-organize-and-setup-global-tasks-1116) Export, metadata extract, C2PA init, local OCFL recover

### Encrypted browser host (phase F5)

63. [ ] [Task FS-5.1](./implementation-plan.md#task-fs-5.1) WASM API host in the browser
    - [ ] [API-F5.1](../apps/filesquid/crates/api/resources/plan.md#phase-f5-browser-host-global-task-fs-51) WASM build of the API for the browser host
    - [ ] [UI-F5.1](../apps/filesquid/ui/resources/plan.md#phase-f5-browser-host-global-tasks-fs-51-16-18) Browser-host bootstrap and adapter
64. [ ] [Task 1.6](./implementation-plan.md#task-1.6) Browser uploaded-content workspace, setup, consent, and exit
    - [ ] [UI-1.6](../apps/filesquid/ui/resources/plan.md#phase-f5-browser-host-global-tasks-fs-51-16-18) Browser workspace, mobile-web layout, consent, setup
    - [ ] [DE-1.6](../apps/filesquid/crates/desktop/resources/plan.md#phases-f2-i1-and-f5-library-import-gallery-organize-and-setup-global-tasks-1116) Setup, diagnostics, and redacted logs
65. [ ] [Task 1.8](./implementation-plan.md#task-1.8) Minimum browser auth, uploaded-ciphertext keys, replica bootstrap, and mutation publication
    - [ ] [BE-1.1](../apps/filesquid/crates/server/resources/plan.md#phase-f5-browser-upload-mailbox-global-task-18) Ciphertext object sessions for browser uploads
    - [ ] [BE-1.2](../apps/filesquid/crates/server/resources/plan.md#phase-f5-browser-upload-mailbox-global-task-18) Minimum account/device auth for browser uploads
    - [ ] [CL-1.6](../apps/filesquid/crates/api/resources/plan.md#phases-f2-i1-and-f5-application-meaning-global-tasks-11-14-18) Browser replica queries (WASM)
    - [ ] [CL-1.8](../apps/filesquid/crates/api/resources/plan.md#phases-f2-i1-and-f5-application-meaning-global-tasks-11-14-18) Mutation publication as pending proposals
    - [ ] [UI-1.8](../apps/filesquid/ui/resources/plan.md#phase-f5-browser-host-global-tasks-fs-51-16-18) Minimum uploaded-ciphertext unlock and desktop-off path

### ImageSquid capabilities, part 2 (phases I1 to I7)

66. [ ] [Task 1.7](./implementation-plan.md#task-1.7) MVP capability verification gate
67. [ ] [Task 2.1](./implementation-plan.md#task-2.1) Multiple libraries, watched folders, and complete organize/export
    - [ ] [DE-2.1](../apps/filesquid/crates/desktop/resources/plan.md#phase-i2-archive-and-migration-global-tasks-2125) Multiple libraries, watched folders, relocation
    - [ ] [UI-2.1](../apps/filesquid/ui/resources/plan.md#phase-i2-multiple-libraries-global-task-21) Multiple libraries and complete organize/export
68. [ ] [Task 2.2](./implementation-plan.md#task-2.2) Saved-query evaluation, bursts, metadata write, and C2PA scans
    - [ ] [DE-2.2](../apps/filesquid/crates/desktop/resources/plan.md#phase-i2-archive-and-migration-global-tasks-2125) Saved-query evaluation, bursts, metadata write, C2PA scans
69. [ ] [Task 2.3](./implementation-plan.md#task-2.3) Migration engine and file/export routes (M-A)
    - [ ] [DE-2.3](../apps/filesquid/crates/desktop/resources/plan.md#phase-i2-archive-and-migration-global-tasks-2125) Migration engine (M-A file/export)
    - [ ] [UI-I2.1](../apps/filesquid/ui/resources/plan.md#phases-i1i7-media-capabilities-in-the-default-ui-global-tasks-14-23-31-41-61-75) Default-UI import routes with preservation matrix
70. [ ] [Task 2.4](./implementation-plan.md#task-2.4) Source-aware web clipping and HTTP evidence (F-041)
    - [ ] [DE-2.4](../apps/filesquid/crates/desktop/resources/plan.md#phase-i2-archive-and-migration-global-tasks-2125) HTTP recorder and encrypted evidence
71. [ ] [Task 2.5](./implementation-plan.md#task-2.5) M-B native Apple acquisition (PhotoKit)
    - [ ] [DE-2.5](../apps/filesquid/crates/desktop/resources/plan.md#phase-i2-archive-and-migration-global-tasks-2125) M-B PhotoKit acquisition
72. [ ] [Task 3.1](./implementation-plan.md#task-3.1) Recipe documents and render workers (composition, tone, detail, RAW)
    - [ ] [CL-3.1](../apps/filesquid/crates/api/resources/plan.md#phases-i3-i4-and-l-later-meaning-slices-global-tasks-31-41-92) Recipe documents for the studio
    - [ ] [DE-3.1](../apps/filesquid/crates/desktop/resources/plan.md#phase-i3-editing-workers-global-task-31) Recipe render worker (composition, tone, detail, RAW)
    - [ ] [UI-I3.1](../apps/filesquid/ui/resources/plan.md#phases-i1i7-media-capabilities-in-the-default-ui-global-tasks-14-23-31-41-61-75) Default-UI recipe indicator, version revert and RAW pairs
73. [ ] [Task 4.1](./implementation-plan.md#task-4.1) People, pets, places, trips, and semantic search indexes
    - [ ] [CL-4.1](../apps/filesquid/crates/api/resources/plan.md#phases-i3-i4-and-l-later-meaning-slices-global-tasks-31-41-92) Identity and discovery corrections
    - [ ] [DE-4.1](../apps/filesquid/crates/desktop/resources/plan.md#phase-i4-discovery-and-byo-ai-global-tasks-41-42) Local indexes (people, OCR, places)
    - [ ] [UI-I4.1](../apps/filesquid/ui/resources/plan.md#phases-i1i7-media-capabilities-in-the-default-ui-global-tasks-14-23-31-41-61-75) Default-UI identities, content search and location
74. [ ] [Task 4.2](./implementation-plan.md#task-4.2) BYO AI connections and per-task model choice
    - [ ] [DE-4.2](../apps/filesquid/crates/desktop/resources/plan.md#phase-i4-discovery-and-byo-ai-global-tasks-41-42) BYO model connections
75. [ ] [Task 5.6](./implementation-plan.md#task-5.6) Collaboration qualify (shared albums and family)
    - [ ] [DE-5.3](../apps/filesquid/crates/desktop/resources/plan.md#phase-f4-keys-sync-hosted-ocfl-global-tasks-51-52-54-de-53-in-global-task-56-phase-i5) Shared library key envelopes
76. [ ] [Task 6.1](./implementation-plan.md#task-6.1) Memory, slideshow, print, and project rendering
    - [ ] [DE-6.1](../apps/filesquid/crates/desktop/resources/plan.md#phases-i5i7-f3-l-and-v-projects-advanced-media-sharing-publication-installers-global-tasks-61-7x-8x-9x-101) Memories and print/slideshow render
    - [ ] [UI-I5.1](../apps/filesquid/ui/resources/plan.md#phases-i1i7-media-capabilities-in-the-default-ui-global-tasks-14-23-31-41-61-75) Default-UI rendered memories and print or PDF
77. [ ] [Task 7.1](./implementation-plan.md#task-7.1) AI-assisted cleanup and styles (F-014)
    - [ ] [DE-7.1](../apps/filesquid/crates/desktop/resources/plan.md#phases-i5i7-f3-l-and-v-projects-advanced-media-sharing-publication-installers-global-tasks-61-7x-8x-9x-101) AI-assisted cleanup worker (F-014)
78. [ ] [Task 7.2](./implementation-plan.md#task-7.2) Portrait, depth, Cinematic, and spatial media (F-015)
    - [ ] [DE-7.2](../apps/filesquid/crates/desktop/resources/plan.md#phases-i5i7-f3-l-and-v-projects-advanced-media-sharing-publication-installers-global-tasks-61-7x-8x-9x-101) Portrait, depth, Cinematic, spatial workers (F-015)
79. [ ] [Task 7.3](./implementation-plan.md#task-7.3) Remaining Live Photo, burst, and motion workflows (F-016)
    - [ ] [DE-7.3](../apps/filesquid/crates/desktop/resources/plan.md#phases-i5i7-f3-l-and-v-projects-advanced-media-sharing-publication-installers-global-tasks-61-7x-8x-9x-101) Remaining Live Photo and motion workers (F-016)
80. [ ] [Task 7.4](./implementation-plan.md#task-7.4) Video and audio editing (F-017)
    - [ ] [DE-7.4](../apps/filesquid/crates/desktop/resources/plan.md#phases-i5i7-f3-l-and-v-projects-advanced-media-sharing-publication-installers-global-tasks-61-7x-8x-9x-101) LGPL FFmpeg video/audio worker (F-017)
81. [ ] [Task 7.5](./implementation-plan.md#task-7.5) Markup flattening and companion annotation (F-018 / F-036)
    - [ ] [DE-7.5](../apps/filesquid/crates/desktop/resources/plan.md#phases-i5i7-f3-l-and-v-projects-advanced-media-sharing-publication-installers-global-tasks-61-7x-8x-9x-101) Markup flattening worker (F-018)
    - [ ] [UI-I6.1](../apps/filesquid/ui/resources/plan.md#phases-i1i7-media-capabilities-in-the-default-ui-global-tasks-14-23-31-41-61-75) Default-UI advanced media: AI marks, fallback previews, motion pairs, video properties, annotations
82. [ ] [Task 8.1](./implementation-plan.md#task-8.1) System sharing, native camera adapters, platform matrix, and M-D audit
    - [ ] [DE-8.1](../apps/filesquid/crates/desktop/resources/plan.md#phases-i5i7-f3-l-and-v-projects-advanced-media-sharing-publication-installers-global-tasks-61-7x-8x-9x-101) System sharing, remaining camera adapters, M-D
83. [ ] [Task 8.2](./implementation-plan.md#task-8.2) C-7 paid C2PA publication and watermarking
    - [ ] [DE-8.2](../apps/filesquid/crates/desktop/resources/plan.md#phases-i5i7-f3-l-and-v-projects-advanced-media-sharing-publication-installers-global-tasks-61-7x-8x-9x-101) C-7 publication coordinator
    - [ ] [BE-8.1](../apps/filesquid/crates/server/resources/plan.md#phases-i7-and-v-publication-extensions-load-tests-global-tasks-82-83-101) Publication job status API
    - [ ] [CO-8.1](../apps/filesquid/crates/core/resources/plan.md#phases-i7-and-l-publication-institutional-and-extension-types-global-tasks-82-84-92-95-superseded) Publication job types (C-7)
84. [ ] [Task 8.3](./implementation-plan.md#task-8.3) Optional service extensions (F-035)
    - [ ] [BE-8.2](../apps/filesquid/crates/server/resources/plan.md#phases-i7-and-v-publication-extensions-load-tests-global-tasks-82-83-101) Optional service entitlement stubs
85. [ ] [Task 8.4](./implementation-plan.md#task-8.4) Professional Article 50 disclosure (C-024)
    - [ ] [DE-8.4](../apps/filesquid/crates/desktop/resources/plan.md#phases-i5i7-f3-l-and-v-projects-advanced-media-sharing-publication-installers-global-tasks-61-7x-8x-9x-101) Professional Article 50 disclosure rendering
    - [ ] [CO-8.2](../apps/filesquid/crates/core/resources/plan.md#phases-i7-and-l-publication-institutional-and-extension-types-global-tasks-82-84-92-95-superseded) Article 50 disclosure types

### Arkrim (phase A)

86. [ ] [Task 9.0](./implementation-plan.md#task-9.0) Post-consumer order of IIIF/GLAM, ProofMode, FileSquid, and Arkrim
87. [ ] [Task AR-1.1](./implementation-plan.md#task-ar-1.1) Arkrim distribution definition and scaffold
88. [ ] [Task AR-1.2](./implementation-plan.md#task-ar-1.2) Curation jobs module
    - [ ] [API-A.1](../apps/filesquid/crates/api/resources/plan.md#phase-a-arkrim-modules-global-tasks-ar-12ar-14) Curation job module and routes
89. [ ] [Task AR-1.3](./implementation-plan.md#task-ar-1.3) Parquet projection
    - [ ] [IO-A.1](../apps/filesquid/crates/ocfl/resources/plan.md#phase-a-parquet-projection-global-task-ar-13) Optional `parquet` feature
    - [ ] [API-A.2](../apps/filesquid/crates/api/resources/plan.md#phase-a-arkrim-modules-global-tasks-ar-12ar-14) Parquet projection routes
90. [ ] [Task AR-1.4](./implementation-plan.md#task-ar-1.4) FAIR reporting
    - [ ] [API-A.3](../apps/filesquid/crates/api/resources/plan.md#phase-a-arkrim-modules-global-tasks-ar-12ar-14) FAIR report routes
91. [ ] [Task AR-1.5](./implementation-plan.md#task-ar-1.5) Arkrim dashboard distribution

### Later candidates (phase L)

92. [ ] [Task 9.1](./implementation-plan.md#task-9.1) Confirm funded institutional interoperability profiles
93. [ ] [Task 9.2](./implementation-plan.md#task-9.2) IIIF-2 bounded institutional archive import
    - [ ] [CO-9.1](../apps/filesquid/crates/core/resources/plan.md#phases-i7-and-l-publication-institutional-and-extension-types-global-tasks-82-84-92-95-superseded) Institutional collection and IIIF transport types
    - [ ] [CL-9.1](../apps/filesquid/crates/api/resources/plan.md#phases-i3-i4-and-l-later-meaning-slices-global-tasks-31-41-92) Institutional collection mapping
    - [ ] [DE-9.1](../apps/filesquid/crates/desktop/resources/plan.md#phases-i5i7-f3-l-and-v-projects-advanced-media-sharing-publication-installers-global-tasks-61-7x-8x-9x-101) IIIF-2 bounded import worker
    - [ ] [IO-9.1](../apps/filesquid/crates/ocfl/resources/plan.md#phase-l-later-objects-after-the-post-imagesquid-order-decision-global-task-92) Institutional import objects in OCFL
94. [ ] [Task 9.3](./implementation-plan.md#task-9.3) IIIF-3 institutional publishing and annual service
    - [ ] [DE-9.3](../apps/filesquid/crates/desktop/resources/plan.md#phases-i5i7-f3-l-and-v-projects-advanced-media-sharing-publication-installers-global-tasks-61-7x-8x-9x-101) IIIF-3 projection and publish worker
    - [ ] [BE-9.1](../apps/filesquid/crates/server/resources/plan.md#phase-l-later-do-not-start-without-gates-global-tasks-93-94) Institutional service terms API
95. [ ] [Task 9.4](./implementation-plan.md#task-9.4) Later hosted private OSS inference (F-048)
    - [ ] [BE-9.2](../apps/filesquid/crates/server/resources/plan.md#phase-l-later-do-not-start-without-gates-global-tasks-93-94) Hosted private OSS inference service

### Verification and synthesis (phase V)

96. [ ] [Task 10.1](./implementation-plan.md#task-10.1) Performance and load tests
    - [ ] [DE-10.1](../apps/filesquid/crates/desktop/resources/plan.md#phases-i5i7-f3-l-and-v-projects-advanced-media-sharing-publication-installers-global-tasks-61-7x-8x-9x-101) Packaged desktop performance evidence
    - [ ] [BE-10.1](../apps/filesquid/crates/server/resources/plan.md#phases-i7-and-v-publication-extensions-load-tests-global-tasks-82-83-101) k6 load tests
97. [ ] [Task 10.2](./implementation-plan.md#task-10.2) README, Dockerization, and production handoff
98. [ ] [Task 10.3](./implementation-plan.md#task-10.3) Architecture synthesis from codebase reality
