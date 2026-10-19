# Task docs-adr-005-rusqlite Walkthrough: `rusqlite` for local SQLite stores

Documentation-only task on branch `docs/adr-005-rusqlite-local-stores`. TDD-ENFORCE: Not applicable - documentation update.

## Owner decision (2026-10-18, option A)

[ADR 005](../architecture/005-sqlite-local-postgres-hosted.md) said "SQLx is the Rust access library for both databases." The code has used `rusqlite` 0.40 with the `bundled` feature in `filesquid-native` for every local SQLite store since CL-0.1 (the activity journal), and DE-0.2 added the per-library catalogs on it. No crate depends on SQLx yet. The owner chose to amend the ADR to match:

- Local SQLite stores in `filesquid-native` use `rusqlite` with the bundled SQLite: a synchronous API that matches the host's intent → OCFL commit → receipt flow, a pinned SQLite compiled in (no system SQLite dependency), and no async runtime or build-time database for query checking. The file format is standard SQLite 3, readable by any SQLite tool.
- PostgreSQL on `filesquid-server` keeps SQLx as planned, to be confirmed when SV-F1.2 starts.

## Changes

- [ADR 005](../architecture/005-sqlite-local-postgres-hosted.md): dated amendment (2026-10-18, owner decision) in Status, in the style of [ADR 027](../architecture/027-filesquid-metadata-pluggable-stores.md); Decision now chooses the access library per database (replacing the "both databases" sentence) and notes the WAL `-wal`/`-shm` companion files and `VACUUM INTO` as the supported consistent copy; Rationale explains `rusqlite`; Alternatives Considered rejects SQLx for the local SQLite stores; Consequences records two query styles and the bundled-SQLite build cost; Related Decisions links ADR 033 and the pinned manifest. Other sections are unchanged.
- [ADR index](../architecture/README.md): the ADR 005 status cell names the amendment.
- [architecture.md](../architecture.md): the D-04 local database row names `rusqlite` with a bundled SQLite.
- [PRD Technology Stack](../prd.md#technology-stack): one parenthetical naming `rusqlite` for local SQLite and SQLx as the planned PostgreSQL library.
- [Rust briefing](../references/rust-language-dev-briefing.md): "SQLx (Postgres/SQLite)" now reads SQLx for PostgreSQL and `rusqlite` for local SQLite stores.
- [DE-0.2 walkthrough](../../apps/filesquid/crates/desktop/resources/tasks/Task-DE-0.2-Walkthrough.md): its open note asking the owner to confirm `rusqlite` now records the resolution.

## Cascade check

A search for `SQLx`/`sqlx` across `resources/`, the crate `resources/` folders, READMEs and `.agents/` found no other statement that SQLx serves SQLite. Left unchanged on purpose:

- [ADR 004](../architecture/004-axum-hosted-ciphertext-mailbox.md) and [ADR 001](../architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md) name SQLx for PostgreSQL only (server and hosted service), which still holds, so no pointer is needed.
- [architecture.md](../architecture.md) server runtime and D-05 rows, the [server plan](../../apps/filesquid/crates/server/resources/plan.md) and the briefing's `query!` example are PostgreSQL mentions.
- The api and c2pa isolation lists (`rusqlite`, `sqlx*`) forbid every SQLite driver and are unaffected.
- The `init-rust-js` skill template already says "SQLx (or an ADR-justified alternative)".
- The FileSquid SRS and module SRS files do not name an access library; their requirements are unchanged. The root README "About this project" section needs no change.

## Hook

The DE-0.2 walkthrough is inside the desktop crate, so `scripts/changed_scope.sh` classifies this change as `rust:filesquid-desktop`, a scoped run (the desktop crate and its dependents). Because the scope includes `rust:`, the rules require the full suite on the head commit, which the explicit `FULL=1` override forces. Raw output of `FULL=1 bash scripts/pre-commit.sh` started at 2026-10-19T01:02:28Z, exit 0 (first and last lines):

```text
changed_scope: full (FULL=1 is set)
========================================
Running Pre-Commit Hooks... (test scope: full)
...
✅ Vitest passed (3s).
ℹ️  Playwright e2e skipped for apps/imagesquid (set FILESQUID_E2E=1 to run).
ℹ️  Desktop packaged-window e2e skipped (set FILESQUID_DESKTOP_E2E=1 to run).
✅ Lint and test validation passed (test scope: full).
========================================
✅ All Pre-Commit Hooks Passed! Test scope: full (176s)
========================================
```
