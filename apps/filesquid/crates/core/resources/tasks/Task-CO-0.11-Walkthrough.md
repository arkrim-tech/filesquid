# Task CO-0.11 Walkthrough: PROV-O history contract types

Branch: `feat/prov-o-history` (global Task FS-2.8; [ADR 040](../../../../../../resources/architecture/040-prov-o-history.md) Decision 10).

## CORE-S-010: PROV-O history contract

Also CORE-S-008 (the retained/ephemeral gate of `ProvOSidecar` is kept).

## What changed

- New module `history`:
  - `OperationKind` with stable tokens. These follow the spelling of the F-067 feature: `import`, `upload`, `copy`, `rename`, `move`, `hide`, `unhide`, `delete`, `restore`, `purge`, `metadata.edit`, `label`, `revert`, `reinstate`, `snapshot`, `catalog`, `repair`, `export`, `scrub`, `backfill`, `unknown`.
  - `OperationMessage` (`filesquid.operation.v1 {"operation_id","kind","revision"?,"label"?}`): `format` writes compact JSON with a fixed member order. `parse` returns `Ok(None)` for a message without the prefix. A prefixed message that cannot be read is refused, not dropped: unknown or duplicate members, a wrong type, an unknown kind, trailing data or a bad escape each give `Unreadable {code}`. Messages are bounded by `MAX_OPERATION_MESSAGE_BYTES` (4096), and errors name no stored value. Core still depends only on `serde`, so the bounded JSON reader is hand-written.
  - `InstallationId`: a UUID v4, lower-cased, rendered `urn:uuid:<uuid>`. It is parsed only; no generation and no I/O.
  - `VersionRef` (object ID and OCFL version name; `object_ref()` = `<object-id>@vN`), `JournalLink` (operation ID and at most 256 opaque journal entry IDs) and `HistoryEntry`.
- `ProvOSidecar` keeps its gate and gains `with_version`, `with_journal`, `version()`, `operation()` and `journal()`. Records without the links still deserialize.
- `PROV_O_CONTEXT` is corrected to the PROV namespace IRI `http://www.w3.org/ns/prov#`. `PROV_O_PROFILE` (`filesquid-prov/1`) is named by `PROV_O_PROFILE_VERSION` 1.

## SRS traceability

| Requirement | Tests |
|---|---|
| CORE-S-010 | `unit_history` (15 tests); BDD `prov_o_history_contract_F-067.feature` (5 scenarios) |
| CORE-S-008 | `unit_provenance` (unchanged, passes), `the_sidecar_keeps_its_gate_and_links_version_operation_and_journal` |

## TDD: failing run before implementation

```text
      1 error: could not compile `filesquid-core` (test "unit_history") due to 4 previous errors
      1 error[E0432]: unresolved imports `filesquid_core::HistoryEntry`, `filesquid_core::InstallationId`, `filesquid_core::JournalLink`, `filesquid_core::OperationKind`, `filesquid_core::OperationMessage`, `filesquid_core::OperationMessageError`, `filesquid_core::VersionRef`, `filesquid_core::OPERATION_MESSAGE_PREFIX`, `filesquid_core::PROV_O_PROFILE`
      1 error[E0599]: no method named `version` found for struct `ProvOSidecar` in the current scope
      2 error[E0599]: no method named `with_version` found for struct `ProvOSidecar` in the current scope
```

## Mocked Environment Variables

None.

## Raw test output

`cargo test -p filesquid-core --locked --test unit_history`:

```text
error[E0432]: unresolved imports `filesquid_core::HistoryEntry`, `filesquid_core::InstallationId`, `filesquid_core::JournalLink`, `filesquid_core::OperationKind`, `filesquid_core::OperationMessage`, `filesquid_core::OperationMessageError`, `filesquid_core::VersionRef`, `filesquid_core::OPERATION_MESSAGE_PREFIX`, `filesquid_core::PROV_O_PROFILE`
 --> crates/core/tests/unit/history.rs:5:5
  |
5 |     HistoryEntry, InstallationId, JournalLink, OperationId, OperationKind, OperationMessage,
  |     ^^^^^^^^^^^^  ^^^^^^^^^^^^^^  ^^^^^^^^^^^               ^^^^^^^^^^^^^  ^^^^^^^^^^^^^^^^ no `OperationMessage` in the root
  |     |             |               |                         |
  |     |             |               |                         no `OperationKind` in the root
  |     |             |               no `JournalLink` in the root
  |     |             no `InstallationId` in the root
  |     no `HistoryEntry` in the root
6 |     OperationMessageError, ProvOSidecar, Rfc3339Utc, VersionRef, OPERATION_MESSAGE_PREFIX,
  |     ^^^^^^^^^^^^^^^^^^^^^                            ^^^^^^^^^^  ^^^^^^^^^^^^^^^^^^^^^^^^ no `OPERATION_MESSAGE_PREFIX` in the root
  |     |                                                |
  |     |                                                no `VersionRef` in the root
  |     no `OperationMessageError` in the root
7 |     PROV_O_CONTEXT, PROV_O_PROFILE, PROV_O_PROFILE_VERSION,
  |                     ^^^^^^^^^^^^^^ no `PROV_O_PROFILE` in the root
  |
help: a similar name exists in the module
  |
5 -     HistoryEntry, InstallationId, JournalLink, OperationId, OperationKind, OperationMessage,
5 +     HistoryEntry, InstallationId, JournalLink, OperationId, OperationId, OperationMessage,
  |

error[E0599]: no method named `with_version` found for struct `ProvOSidecar` in the current scope
   --> crates/core/tests/unit/history.rs:245:10
    |
244 |       let sidecar = ProvOSidecar::retained(time("2026-10-19T10:00:01Z"))
    |  ___________________-
245 | |         .with_version(version.clone())
    | |         -^^^^^^^^^^^^ method not found in `ProvOSidecar`
    | |_________|
    |

error[E0599]: no method named `with_version` found for struct `ProvOSidecar` in the current scope
   --> crates/core/tests/unit/history.rs:252:77
    |
252 |     let cache = ProvOSidecar::ephemeral_cache(time("2026-10-19T10:00:01Z")).with_version(version);
    |                                                                             ^^^^^^^^^^^^ method not found in `ProvOSidecar`

error[E0599]: no method named `version` found for struct `ProvOSidecar` in the current scope
   --> crates/core/tests/unit/history.rs:261:23
    |
261 |     assert_eq!(parsed.version(), None);
    |                       ^^^^^^^ method not found in `ProvOSidecar`

Some errors have detailed explanations: E0432, E0599.
For more information about an error, try `rustc --explain E0432`.
error: could not compile `filesquid-core` (test "unit_history") due to 4 previous errors
```

`cargo test -p filesquid-core --locked --test features` (new feature, then totals; the 4 skipped scenarios belong to earlier features):

```text

```
