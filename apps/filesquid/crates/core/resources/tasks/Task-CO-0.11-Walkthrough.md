# Task CO-0.11 Walkthrough: PROV-O history contract types

Branch: `feat/prov-o-history` (global Task FS-2.8; ADR 040 Decision 10).

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
| CORE-S-010 | `unit_history` (19 tests); BDD `prov_o_history_contract_F-067.feature` (5 scenarios) |
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

`cargo test -p filesquid-core --locked --test unit_history` (after review round 1: the catalog receipt message, decimal-string revisions, RFC 3339 deletion times, unbounded version names and the single-source sidecar operation are covered):

```text
test a_journal_link_names_an_operation_and_opaque_entry_ids ... ok
test a_version_ref_renders_the_object_reference ... ok
test a_history_entry_serializes_one_activity_for_the_api ... ok
test a_message_without_the_prefix_is_not_a_filesquid_message ... ok
test an_oversized_message_is_refused ... ok
test an_operation_message_formats_and_parses_back ... ok
test an_unreadable_prefixed_message_is_refused_not_dropped ... ok
test installation_id_is_a_uuid_v4_rendered_as_a_urn ... ok
test prov_context_is_the_prov_namespace_iri ... ok
test optional_members_are_left_out ... ok
test every_operation_kind_has_a_stable_token ... ok
test installation_ids_that_are_not_uuid_v4_are_refused ... ok
test refusal_errors_name_no_stored_value ... ok
test the_catalog_receipt_message_is_read ... ok
test the_revision_is_written_as_a_decimal_string_and_read_back ... ok
test the_sidecar_operation_is_the_journal_links_only ... ok
test unicode_escapes_and_surrogate_pairs_are_decoded ... ok
test the_sidecar_keeps_its_gate_and_links_version_operation_and_journal ... ok
test version_names_beyond_eighteen_digits_are_valid_object_versions ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`cargo test -p filesquid-core --locked --test features` (the new feature's scenarios, then totals; the 4 skipped scenarios belong to earlier features):

```text
Feature: PROV-O history contract types
  Scenario: The version message of a commit round-trips
  Scenario: A message another tool wrote is not a FileSquid message
  Scenario: An unreadable FileSquid message is refused, not dropped
  Scenario: The installation agent is a UUID v4 URN
  Scenario: The PROV namespace is the W3C namespace IRI
12 features
68 scenarios (64 passed, 4 skipped)
221 steps (217 passed, 4 skipped)
```
