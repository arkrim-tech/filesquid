# Architecture Decision Record - FileSquid

005-sqlite-local-postgres-hosted.md

## Title

SQLite for the local operational catalog; PostgreSQL and object storage for hosted operations

## Status

Accepted

Accepted: 2026-09-16T01:01:07Z. Revised: 2026-09-22T04:34:37Z.

Amended 2026-10-18T06:52:39Z by [ADR 033](./033-activity-journal-and-live-health-monitoring.md): until its PostgreSQL operational store lands (SV-F1.2), `filesquid-server` keeps its activity journal in the SQLite store of `filesquid-native` (`<data dir>/filesquid.sqlite`). PostgreSQL remains the server target.

Amended 2026-10-18 (owner decision): the Rust access library is chosen per database. Local SQLite stores in `filesquid-native` use `rusqlite` with the bundled SQLite, as the code has since CL-0.1 (the activity journal) and DE-0.2 (per-library catalogs). PostgreSQL on `filesquid-server` and the hosted service keeps SQLx as planned in [ADR 004](./004-axum-hosted-ciphertext-mailbox.md), to be confirmed when SV-F1.2 starts. This replaces the earlier sentence "SQLx is the Rust access library for both databases" (Decision, Rationale, Alternatives Considered and Consequences below).

## Context / Requirement Reference

Committed local media, metadata and relationships MUST live in owner-readable files and OCFL snapshots. An operational database MUST be rebuildable and MUST NOT be the only copy of accepted user data ([FE-001](../product-vision.md#independent-library), [FE-065](../product-vision.md#independent-library), [U-03](../product-vision.md#platform-ownership-and-access), [C-028](../product-vision.md#platforms-originals-and-service-limits)). Hosted accounts, devices, membership and ciphertext transport need a concurrent operational store plus object storage.

These are different jobs. They MUST NOT be used as an excuse for a second application language.

## Decision

- **Local:** SQLite is the embedded operational catalog, job journal and index beside the canonical OCFL tree. Each library has its own SQLite catalog file (or a documented small set of files) on the owner's disk. A user or group MAY own more than one library on a repository ([FE-002](../product-vision.md#independent-library)). Rebuild-from-OCFL remains mandatory ([FE-065](../product-vision.md#independent-library)).
- **Hosted:** PostgreSQL holds accounts, devices, membership, entitlements, ciphertext object receipts and hosted job rows. S3-compatible object storage holds ciphertext bodies. Shared buckets with enforced per-user prefixes are the default; dedicated buckets remain justified exceptions.

SQLite and the OCFL filesystem **do not share an atomic transaction**. Cross-store completion is architectural. This ADR **adopts** the commit sequence: record intent (operation ID and expected revision), make canonical content and OCFL snapshots durable, then update indexes, outbox and job completion from the **canonical receipt** ([T-005](../product-vision.md#canonical-ocfl-state-and-evidence), [T-035](../product-vision.md#canonical-ocfl-state-and-evidence)). Crash recovery reconciles pending intent and committed snapshots idempotently. A crash before the snapshot leaves work pending even if bytes were verified. A crash after the snapshot is durable cannot lose acknowledged collection state even when the database is absent, because the canonical files allow rebuild ([T-007](../product-vision.md#canonical-ocfl-state-and-evidence)). Hosted PostgreSQL receipts and object-storage commits need the corresponding reconciliation.

Copying a live SQLite file is **not** a supported backup. WAL can hold committed pages not yet in the main file ([SQLite WAL](https://www.sqlite.org/wal.html)). Owners copy a **consistent SQLite backup** (controlled shutdown or a documented backup API) plus referenced OCFL snapshots. A local catalog in WAL mode has `-wal` and `-shm` companion files beside its main file; `filesquid-native` writes its supported consistent copy with SQLite's `VACUUM INTO`. SQLite WAL checkpointing remains SQLite's flush term; it is not an OCFL snapshot.

The Rust access library is chosen per database (amendment 2026-10-18):

- **Local SQLite:** every local SQLite store in `filesquid-native` (the activity journal and the per-library catalogs) uses `rusqlite` with its `bundled` feature, so a pinned SQLite is compiled into the binary and no system SQLite is required. The file format is standard SQLite 3, readable by any SQLite tool.
- **PostgreSQL:** `filesquid-server` and the hosted service use SQLx for PostgreSQL as planned ([ADR 004](./004-axum-hosted-ciphertext-mailbox.md)), to be confirmed when SV-F1.2 starts.

Schema migrations are versioned per database. Job table columns are specified when jobs are implemented; recovery MUST NOT wait on that schema ([ADR 007](./007-durable-jobs-and-interactive-performance.md)).

This commit/recovery protocol is a **prerequisite** to ingestion and mutation implementation.

No additional application database (MongoDB, Elasticsearch, graph database, vector database) is introduced by this ADR.

## Rationale

SQLite matches a single-owner offline app: no daemon, no network, and a library whose OCFL snapshot can be copied with documented procedures. PostgreSQL matches concurrent hosted accounts. Object storage matches large ciphertext. Using both is not redundancy; using FastAPI for one and Phoenix for the other would be.

`rusqlite` for the local stores (amendment 2026-10-18): its synchronous API matches the host's intent → OCFL commit → receipt flow, where each step is a short transaction that completes before the next step starts. The `bundled` feature compiles a pinned SQLite into the binary, so behaviour does not depend on the SQLite version of the owner's operating system. It needs no async runtime and no build-time database for query checking. SQLx stays the plan for PostgreSQL, where an async, concurrent server is the right fit.

## Assumptions

- Filesystems used for SQLite are suitable for its locking model, or the implementation documents and tests the limitation.
- Hosted PostgreSQL and object storage providers, geography and cost remain later ADRs.
- Encrypted hosted OCFL inventories describe ciphertext; they are not a public file catalog.

## Alternatives Considered

- **SQLite for hosted multi-tenant:** Rejected; concurrent writers and operational tooling are the wrong fit.
- **PostgreSQL as the local desktop catalog:** Rejected; forces a server on every owner machine.
- **Only object storage, no hosted SQL:** Insufficient for accounts, devices, billing and job rows.
- **SQLx for the local SQLite stores:** Rejected (amendment 2026-10-18). Its async API needs a runtime the desktop's synchronous transactional flow does not use; its compile-time checked queries need a database or prepared query metadata at build time; and `filesquid-native` already used `rusqlite` for the activity journal (CL-0.1) and the per-library catalogs (DE-0.2), so switching would add a second SQLite driver or a rewrite with no gain for an embedded, single-owner store.
- **Canonical data only in SQLite:** Forbidden by [FE-001](../product-vision.md#independent-library) and [U-03](../product-vision.md#platform-ownership-and-access).

## Consequences / Implications

Easier: clear durability story; standard tools. Harder: two schemas to migrate; every mutation path must implement intent → canonical commit → receipt reconcile; hosted SQL MUST NOT cache private titles. Index rebuild performance is part of [T-010](../product-vision.md#performance-acceptance) evidence. Live-file copy of SQLite is forbidden as a backup story. Two Rust access libraries (`rusqlite` locally, SQLx for PostgreSQL) mean two query styles and no shared query code between the local and server stores; schemas were already separate. The bundled SQLite adds to the build time and binary size of crates that enable it, and its version moves only when `rusqlite` is updated. Direct inspection and copy of a completed snapshot MUST NOT authorize in-place inventory or signed-original edits ([T-036](../product-vision.md#canonical-ocfl-state-and-evidence)).

## Related Decisions / Notes

- [ADR 001](./001-shared-rust-core-with-tauri-and-axum-hosts.md), [ADR 004](./004-axum-hosted-ciphertext-mailbox.md), [ADR 007](./007-durable-jobs-and-interactive-performance.md), [ADR 016](./016-rust-ocfl-library.md), [ADR 033](./033-activity-journal-and-live-health-monitoring.md)
- Amendment 2026-10-18: `rusqlite` 0.40 with the `bundled` feature is declared in [`filesquid-native`'s manifest](../../apps/filesquid/crates/native/Cargo.toml), and the workspace `Cargo.lock` pins the exact version. The SQLx crate version for PostgreSQL is pinned when SV-F1.2 (server operational store) starts; [ADR 004](./004-axum-hosted-ciphertext-mailbox.md) notes the line verified at writing.
