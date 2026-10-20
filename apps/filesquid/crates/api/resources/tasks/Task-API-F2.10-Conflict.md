# Conflict Artifact (open): Task API-F2.10, where the `fsquid` CLI journals

Raised 2026-10-20 by Task [API-F2.10](../plan.md) (global [Task FS-2.9](../../../../../../resources/implementation-plan.md#task-fs-2.9)) under [sync-logic.md](../../../../../../.agents/rules/sync-logic.md).

**Status: open; needs an owner decision.** Every other part of API-F2.10 is implemented and tested ([walkthrough](./Task-API-F2.10-Walkthrough.md)). The CLI part is not, because the documents do not say which journal store a host-less `fsquid` run writes to.

## 1. Conflict description

[API-S-031](../srs.md) says "the `fsquid` CLI MUST journal through this same path", and [PRD F-061](../../../../../../resources/prd.md#f-061-activity-journal-and-live-health-monitoring) says "the `fsquid` CLI MUST journal through the same `filesquid-api` path". In the code on `main`:

- `filesquid-cli` ([`crates/cli`](../../../cli/Cargo.toml)) depends on `filesquid-metadata` only (feature `cli`). Its one module's commands (`extract`, `get`, `set`, `export`, `import`, `validate`) read and write `metadata.md` in an OCFL object directory the caller names (`--object`), each edit committing a new OCFL version through the module's own `LocalObject`; they open no library and no host, and no `filesquid-api` code runs.
- The journal path of `filesquid-api` ends in a host-injected `JournalStore`. The durable stores live with the hosts: `filesquid_native::SqliteJournal` in the desktop's `<data dir>/filesquid.sqlite` (the Tauri app data directory) and, on the server, the store under `FILESQUID_DATA_DIR`.
- No document names the journal a CLI run writes to when no host runs, nor whether the CLI may write into a desktop host's `filesquid.sqlite` while that host is running.

Choosing a location, or making the CLI link `filesquid-api` and `filesquid-native`, is an architecture decision this task cannot make on its own.

## 2. PRD / SRS reference

- [API-S-031](../srs.md): "the `fsquid` CLI MUST journal through this same path".
- [PRD F-061](../../../../../../resources/prd.md#f-061-activity-journal-and-live-health-monitoring), Complete coverage: "the `fsquid` CLI MUST journal through the same `filesquid-api` path".
- [ADR 033](../../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md) Decision 7 table: "`fsquid` CLI operation | Same `filesquid-api` path → journal row, and PROV-O when it commits".

## 3. ADR reference

- [ADR 036](../../../../../../resources/architecture/036-single-fsquid-cli.md) Decision 2: "`filesquid-cli` is not a module: it may depend on every module, and no module depends on it." It names modules only; `filesquid-api` and `filesquid-native` are not modules.
- [ADR 033](../../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md) Decision 2: "Each host and each device keeps its own journal" in "its own operational store". A CLI run is neither a host nor tied to one.
- [src-isolation.md](../../../../../../.agents/rules/src-isolation.md): `filesquid-native` holds host-side native I/O (SQLite storage, data-directory checks).

## 4. Recommended resolution

Option A (recommended): `filesquid-cli` depends on `filesquid-api` (journal types and `JournalStore`) and `filesquid-native` with default features (no `desktop`), and each run journals one entry per command (`cli.<module>.<command>`, outcome, error code, opaque IDs only) to `<data dir>/filesquid.sqlite`, where the data directory is `FILESQUID_DATA_DIR` or else the desktop app's data directory for the platform; SQLite WAL mode lets it write beside a running desktop host. A command that cannot open the journal refuses to run, so no activity goes unjournaled. ADR 036 gains an amendment for the two new dependencies; README `## View logs` documents the variable.

Option B: the CLI journals only when it is pointed at a host through the API (a future `--host` mode), and stand-alone sidecar commands, which touch no library, are recorded as outside the journal's scope in `logging-enforce.md`. This would be a third recorded exception, which the owner decision of 2026-10-19 rules out unless the owner revises it.

Artifacts to update for option A: [ADR 036](../../../../../../resources/architecture/036-single-fsquid-cli.md) (amendment), [API-S-031](../srs.md) (the data-directory rule), `crates/cli/Cargo.toml` and `src/lib.rs`, the root `README.md` (`## View logs`), and a CLI test that a command run leaves one journal entry and that an unopenable journal refuses the command.

## 5. Impact assessment

Until this is decided, a `fsquid` command run is not journaled. Today that is the metadata module's commands on an OCFL object directory the caller names: an edit (`set`, `import`) commits a new OCFL version of that object with no journal entry and no PROV-O record, because it bypasses `filesquid-api`. A FileSquid library's objects are written by the hosts through the API, so a CLI edit of one is a change made outside FileSquid; the gap grows when OCFL and C2PA commands register with the CLI. Nothing else in API-F2.10 depends on it.
