# Conflict Artifact (decided): Task API-F2.10, where the `fsquid` CLI journals

Raised 2026-10-20 by Task [API-F2.10](../plan.md) (global [Task FS-2.9](../../../../../../resources/implementation-plan.md#task-fs-2.9)) under [sync-logic.md](../../../../../../.agents/rules/sync-logic.md).

**Status: decided 2026-10-20, see the owner decisions** (PR #102: [ADR 036](../../../../../../resources/architecture/036-single-fsquid-cli.md) Decision 8 and [API-S-031](../srs.md)). The CLI journaling is option A, implemented by [CLI-0.2](../../../cli/resources/plan.md); the engine-warning gaps of section 6 are follow-up tasks, the copy guard's object reference [IO-1.6](../../../ocfl/resources/plan.md) and the file-route operation IDs [API-F2.12](../plan.md); no journal exception is added. The sections below keep the original analysis.

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

Option A (recommended): `filesquid-cli` depends on `filesquid-api` (journal types and `JournalStore`) and `filesquid-native` with default features (no `desktop`), and each run that changes state or fails journals one entry per command (`cli.<module>.<command>`, outcome, error code, opaque IDs only); a run that succeeds without a change (such as `set` reporting "unchanged; no version committed") journals nothing, as the owner's no-op exception in `logging-enforce.md` requires to `<data dir>/filesquid.sqlite`, where the data directory is `FILESQUID_DATA_DIR` or else the desktop app's data directory for the platform; SQLite WAL mode lets it write beside a running desktop host. A command that cannot open the journal refuses to run, so no activity goes unjournaled. ADR 036 gains an amendment for the two new dependencies; README `## View logs` documents the variable.

Option B: the CLI journals only when it is pointed at a host through the API (a future `--host` mode), and stand-alone sidecar commands, which touch no library, are recorded as outside the journal's scope in `logging-enforce.md`. This would be a third recorded exception, which the owner decision of 2026-10-19 rules out unless the owner revises it.

Artifacts to update for option A: [ADR 036](../../../../../../resources/architecture/036-single-fsquid-cli.md) (amendment), [API-S-031](../srs.md) (the data-directory rule), `crates/cli/Cargo.toml` and `src/lib.rs`, the root `README.md` (`## View logs`), and a CLI test that a command run leaves one journal entry and that an unopenable journal refuses the command.

## 5. Impact assessment

Until this is decided, a `fsquid` command run is not journaled. Today that is the metadata module's commands on an OCFL object directory the caller names: an edit (`set`, `import`) commits a new OCFL version of that object with no journal entry and no PROV-O record, because it bypasses `filesquid-api`. A FileSquid library's objects are written by the hosts through the API, so a CLI edit of one is a change made outside FileSquid; the gap grows when OCFL and C2PA commands register with the CLI. Nothing else in API-F2.10 depends on it.

## 6. Also open: engine warnings without full context

[API-S-031](../srs.md) says engine warnings (Git, copy and storage-hierarchy guards, lease takeovers) MUST be journaled with operation and object reference. API-F2.10 journals every `filesquid_ocfl` warning as `ocfl.engine.<module>` with the operation ID of the enclosing activity span and the engine's `object@version` when it names them (lease takeovers do). Two cases cannot be completed from `filesquid-api`:

- the copy guard (`filesquid_ocfl::read::copy_guard`) warns with paths only, so its entry has no object reference; naming the object there is a `filesquid-ocfl` change (modules engineer);
- the file routes (`/api/v1/files/...`, API-F1.3) commit with the caller's version message and no operation ID, so their engine warnings carry none; giving those commits a `filesquid.operation.v1` message with an operation ID is an API-F2.8/API-F2.9 follow-up.

The Git and storage-hierarchy guards refuse with errors that their callers journal as failed operations. Recommended resolution: a modules task to add `object` (and `version` where known) to the copy guard's warning, and an API task to give file-route commits an operation ID; or the owner accepts these two cases as journaled with partial context.
