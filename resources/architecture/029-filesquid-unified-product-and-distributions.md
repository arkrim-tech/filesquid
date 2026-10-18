# Architecture Decision Record - FileSquid

029-filesquid-unified-product-and-distributions.md

## Title

FileSquid is one product, a Cargo workspace of independent module crates; ImageSquid and Arkrim are UI-only distributions

## Status

Accepted

Accepted: 2026-10-15. Supersedes the "stand-alone product" and "own repository" parts of [ADR 023](./023-filesquid-ocfl-engine-and-parquet.md), [ADR 024](./024-filesquid-metadata-engine.md) and [ADR 026](./026-filesquid-provenance-engine.md). Amends [ADR 001](./001-shared-rust-core-with-tauri-and-axum-hosts.md) and [ADR 013](./013-cargo-svelte-workspaces.md).

Amended: 2026-10-18 by [ADR 037](./037-c2pa-in-release-2.md): the first consumer release ships the `ocfl` and `metadata` modules; the `c2pa` module ships with the ImageSquid release.

## Context / Requirement Reference

Until now this repository delivered ImageSquid, a photo and video manager. Three engines (`filesquid-ocfl`, `filesquid-metadata`, `filesquid-provenance`) were separate products that ImageSquid called, each meant to move to its own repository ([ADR 023](./023-filesquid-ocfl-engine-and-parquet.md), [ADR 024](./024-filesquid-metadata-engine.md), [ADR 026](./026-filesquid-provenance-engine.md)). A generic FileSquid product was planned for after ImageSquid 1.0.

The owner reversed that order. FileSquid is now the product this repository delivers ([FileSquid vision](../product-vision.md)). It is one Rust backend with one API and three modules: `ocfl`, `metadata` and `c2pa`. ImageSquid and Arkrim become distributions: UI layers on top of FileSquid ([FE-076](../product-vision.md#platform-api-and-hosts), [FE-077](../product-vision.md#platform-api-and-hosts), [U-24](../product-vision.md#one-api-hosts-and-distributions), [U-25](../product-vision.md#one-api-hosts-and-distributions)).

Requirements that drive this decision:

- One Cargo workspace of module crates, with no default cross-module dependency except the `metadata` module's optional `ocfl` store, and each crate building and testing alone ([T-052](../product-vision.md#api-and-hosts)).
- Release order: FileSquid first, encrypted backup and sync second, ImageSquid after that ([C-025](../product-vision.md#release-sequence), [FE-078](../product-vision.md#platform-api-and-hosts)). Arkrim and other milestones later ([C-052](../product-vision.md#release-sequence)).
- Distributions reuse FileSquid through its API and add no backend ([C-026](../product-vision.md#release-sequence)). Arkrim's curation jobs, Parquet and FAIR features are FileSquid modules ([FE-079](../product-vision.md#institutional-interoperability), [C-051](../product-vision.md#platforms-originals-and-service-limits)).
- The module engines stay independently usable by programs that never run FileSquid ([FE-069](../product-vision.md#independent-library), [FE-070](../product-vision.md#independent-library), [FE-072](../product-vision.md#independent-library), [C-047](../product-vision.md#module-rules), [C-048](../product-vision.md#module-rules), [C-054](../product-vision.md#module-rules)).
- Each distribution MUST be extractable to its own repository ([Constraint requirements](../product-vision.md#constraint-requirements)).

## Decision

**1. One product.** FileSquid is one product. It inherits every requirement of the three former module visions. Those visions are deleted; git history keeps them. The FileSquid vision, PRD, SRS, architecture document, ADRs and implementation plan live in root `resources/`.

**2. This repository is the FileSquid repository.** ImageSquid is the distribution in `apps/imagesquid/`. Its vision, PRD, SRS and plan live in `apps/imagesquid/resources/`. FileSquid MUST NOT depend on any distribution's code. Each distribution MUST be extractable to its own repository. A distribution consumes FileSquid only through the published API ([ADR 030](./030-filesquid-api-and-transports.md)) and the `@filesquid/ui` package ([ADR 031](./031-vanilla-typescript-ui-and-white-label-theming.md)). `apps/filesquid/` MUST also stay extractable as a unit.

**3. Cargo workspace of module crates.** FileSquid is one product-local Cargo workspace at `apps/filesquid/` (`Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`). The repository root has no `Cargo.toml`. Crates live under `apps/filesquid/crates/`:

| Crate | Directory | Role | Status at writing |
|---|---|---|---|
| `filesquid-core` | `crates/core/` | Shared vocabulary and contracts (opaque IDs, repository, library, collection, virtual collection, virtual exhibit, file, jobs, crypto envelopes) | Exists (was `imagesquid_core`) |
| `filesquid-ocfl` | `crates/ocfl/` | Module: storage facts, OCFL 1.1 ([ADR 016](./016-rust-ocfl-library.md), [ADR 023](./023-filesquid-ocfl-engine-and-parquet.md)) | Exists |
| `filesquid-metadata` | `crates/metadata/` | Module: every other file fact ([ADR 024](./024-filesquid-metadata-engine.md), [ADR 027](./027-filesquid-metadata-pluggable-stores.md)) | Exists |
| `filesquid-c2pa` | `crates/c2pa/` | Module: Content Credentials ([ADR 026](./026-filesquid-provenance-engine.md)) | Planned; docs and feature file only |
| `filesquid-api` | `crates/api/` | The single API: routes, contract, auth, job and event bus ([ADR 030](./030-filesquid-api-and-transports.md)) | Being added |
| `filesquid-server` | `crates/server/` | Host: Axum ([ADR 032](./032-hosting-and-privacy-model.md)) | Exists (was `imagesquid_backend`) |
| `filesquid-desktop` | `crates/desktop/` | Host: Tauri 2 ([ADR 002](./002-tauri2-desktop-shell.md)) | Exists (was `imagesquid_desktop`) |

**4. Module independence.** A module crate (`ocfl`, `metadata`, `c2pa`, and later modules) MUST NOT take a default dependency on another module crate. The one exception is the `metadata` module's optional `ocfl` store ([ADR 027](./027-filesquid-metadata-pluggable-stores.md)). Each module crate MUST build, test and publish on its own (`cargo test -p filesquid-ocfl`). Module crates MUST NOT depend on `filesquid-api`, a host, Axum, Tauri or any distribution. `filesquid-api` depends on the modules; hosts depend on `filesquid-api`. The engine rules inherited from ADR 016, 023, 024, 026 and 027 (generic OCFL API, closed `/terms/` register, from-scratch C2PA, default features, WASM builds) are unchanged.

**5. Distributions are UI layers only.** A distribution is a UI bundle, design tokens, a label table, brand assets and a module set, described by a `distribution.json` manifest ([ADR 031](./031-vanilla-typescript-ui-and-white-label-theming.md)). A distribution MUST NOT contain Rust, server code or its own API. If a distribution needs a backend capability FileSquid lacks (for ImageSquid: media derivatives, editing recipes, people, migration routes, AI connections, publication), that capability MUST be built as a FileSquid module behind the API and switched on by the distribution's module set. A test of placement: a capability that stores, computes or enforces anything, including application meaning such as people or recipes, is a FileSquid module behind the API; only how a distribution names and presents it (views, labels, layout) belongs in its UI layer.

**6. Arkrim.** Arkrim is the digital-archives and records-management distribution. Its curation jobs (for example transcoding), Parquet projection and FAIR reporting are FileSquid modules exposed through the API to an Arkrim dashboard ([FE-079](../product-vision.md#institutional-interoperability)).

**7. Release order.**

1. FileSquid: the API and the default Drive-style UI, with the `ocfl` and `metadata` modules, on the desktop and `filesquid-server` hosts (the `c2pa` module moved to the ImageSquid release by [ADR 037](./037-c2pa-in-release-2.md)).
2. Encrypted backup and sync at `*.filesquid.tech` ([ADR 032](./032-hosting-and-privacy-model.md)).
3. ImageSquid, as a FileSquid distribution, with FileSquid's `c2pa` module.
4. Arkrim and the other later milestones, in an order the owner chooses ([C-052](../product-vision.md#release-sequence)).

**8. Module rename.** The C2PA module is `filesquid-c2pa`, in `apps/filesquid/crates/c2pa/`. It replaces the name `filesquid-provenance`. [ADR 026](./026-filesquid-provenance-engine.md) keeps its filename as history; its technical content applies to `filesquid-c2pa`.

## Rationale

Nearly all code and documentation in this repository is platform: every Rust crate, ADRs 001–028, the tests and the plans. After this decision the ImageSquid layer is views, tokens, labels and brand only. Making this the FileSquid repository keeps the large part in place and lets the small part leave.

One workspace gives one lockfile, one toolchain and one CI run for crates that ship together. The module-independence rule keeps what the stand-alone-product decisions were really protecting: an institution can still use `filesquid-ocfl` without the rest of FileSquid.

Putting every backend capability in FileSquid means a FileSquid user gets thumbnails or a transcoding job without installing ImageSquid or Arkrim. It also means one place to test, secure and document backend behaviour.

## Assumptions

- A product-local Cargo workspace does not stop a member crate from being published on crates.io on its own, provided its default dependencies stay inside the rules in Decision 4.
- Distributions can express every photo- or archive-specific need as views over the internal vocabulary plus enabled modules. If one cannot, that is a new ADR, not an exception.
- `filesquid-metadata` currently has an unconditional dependency on `filesquid-ocfl`. Making it the optional `ocfl` feature is existing planned work (Task 0.15 in the [implementation plan](../implementation-plan.md); MD-0.4 in the [metadata plan](../../apps/filesquid/crates/metadata/resources/plan.md)). Until then the crate does not yet satisfy Decision 4.
- FileSquid's open product decisions (hosting region, server datastore) are out of scope for this code layout.

## Alternatives Considered

- **Keep three stand-alone engine products and build FileSquid later (ADR 023, 024, 026):** Puts the generic product after the photo product and splits issues, reviews and releases over four repositories. Rejected.
- **One crate with feature flags per module:** Cargo features unify across a build graph, so a module could not be guaranteed to build or publish alone ([ADR 001](./001-shared-rust-core-with-tauri-and-axum-hosts.md)). Rejected.
- **Separate top-level crates under `apps/` with no workspace (ADR 013 layout):** Several lockfiles and toolchains for crates that release together. Rejected.
- **Distributions with their own backend code:** Creates parallel backends and breaks [C-026](../product-vision.md#release-sequence). Rejected.
- **Extract FileSquid to a new repository and keep this one as ImageSquid:** Moves the large part to keep the small part. Rejected.
- **Keep the name `filesquid-provenance`:** The module scope is C2PA; the owner chose the plain name. Rejected.

## Consequences / Implications

Easier: one product to build, test and release; distributions are cheap to add; module crates still stand alone; the capability placement test settles where new work goes.

Harder: FileSquid grows large, because every distribution's backend needs become FileSquid modules. Mitigation: module crates with no default cross-dependencies, a module set per distribution, and FileSquid release 1 limited to the `ocfl` and `metadata` modules ([ADR 037](./037-c2pa-in-release-2.md)). CI MUST build and test each module crate alone, as well as the workspace. Rules, scripts and READMEs that assumed separate app directories (`apps/core`, `apps/backend`, `apps/desktop`, `apps/frontend`, `apps/client`, `apps/filesquid-*`) MUST be updated; those directories no longer exist.

## Related Decisions / Notes

- Supersedes in part: [ADR 023](./023-filesquid-ocfl-engine-and-parquet.md), [ADR 024](./024-filesquid-metadata-engine.md), [ADR 026](./026-filesquid-provenance-engine.md) (stand-alone product and own-repository wording; technical content unchanged).
- Amends: [ADR 001](./001-shared-rust-core-with-tauri-and-axum-hosts.md) (hosts link `filesquid-api`; Axum is also `filesquid-server`), [ADR 013](./013-cargo-svelte-workspaces.md) (product-local Cargo workspace; Vite and TypeScript without Svelte).
- Related: [ADR 030](./030-filesquid-api-and-transports.md), [ADR 031](./031-vanilla-typescript-ui-and-white-label-theming.md), [ADR 032](./032-hosting-and-privacy-model.md), [ADR 027](./027-filesquid-metadata-pluggable-stores.md), [ADR 028](./028-filesquid-ui-packages-and-panel-composition.md).
- Requirements: [FE-074–FE-078](../product-vision.md#platform-api-and-hosts), [FE-079](../product-vision.md#institutional-interoperability), [T-052](../product-vision.md#api-and-hosts), [C-025, C-026, C-052](../product-vision.md#release-sequence), [C-044–C-055](../product-vision.md#module-rules).
- Component diagram: [component-diagram.html](./component-diagram.html).
- PRD: [F-050](../prd.md#f-050-filesquid-platform-and-distributions), [F-058](../prd.md#f-058-white-label-distributions), [F-060](../prd.md#f-060-archival-curation-jobs-and-datasets).
