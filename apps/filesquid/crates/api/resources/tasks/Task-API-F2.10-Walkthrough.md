# Task API-F2.10 Walkthrough: Client events, journal query filters and journal export

Branch: `feat/api-f2.10-client-events-journal-export` (global [Task FS-2.9](../../../../../../resources/implementation-plan.md#task-fs-2.9); [ADR 033](../../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md) Decision 7). API contract `0.13.0`.

## API-S-030: Client events

## API-S-031: Journal query and export

Also API-S-017 (journal entry shape and redaction), PRD [F-061](../../../../../../resources/prd.md#f-061-activity-journal-and-live-health-monitoring) and [SRS-F-061](../../../../../../resources/srs.md#srs-f-061-activity-journal-and-live-health-monitoring) Activity journal and live health monitoring.

## What changed

- **Client events** (`src/client_events/`): `POST /api/v1/client-events`, action `client_events.write`. The body is read up to 16 KiB (`payload_too_large`, 413, past it); a batch of 1 to 20 events is parsed and checked as a whole (`invalid_request`, 400, on any fault); then the rate bound (60 requests in any rolling minute per credential digest, timed by a monotonic clock, `rate_limited`, 429, retryable, with `Retry-After`) applies to every request, refused ones included, except one lone `ui.console.overflow` event that names no other principal; an event whose `principal` differs from the credential's refuses the batch (403 `not_permitted`, journaled as `api.auth.denied`). Each event becomes its own entry (`ui.console.warn`, `ui.console.error`, `ui.console.overflow` with target `count:<n>`), written through the announcing journal store before the answer, which acknowledges every client event ID and names the credential's principal for the client to store with kept events. A console event's other fields (message, URL, file name, stack) are never read; its component, error code and IDs must be opaque tokens and its ID a UUID v4, stored in lowercase so a resend in other letter case is the same ID. The overflow event has its own closed schema (`kind`, `id`, `component`, `count` 1 to 1,000,000, optional `principal`).
- **De-duplication** (`JournalEntry::client_event_id`): both stores keep each client event ID at most once; `MemoryJournal` through an index map and `filesquid_native::SqliteJournal` through a new `client_event_id` column under a unique index (migrated forward on open), so a resent batch is acknowledged with `journaled: 0` and the dedup survives restarts. The ID is redacted like every other identifier.
- **Journal query** (`src/admin/journal.rs`): new filters `library`, `object`, `file` (the same as `object=urn:uuid:<file>`) and `operation_id`, validated as opaque tokens; `GET /api/v1/admin/journal/entries/{entry}`; every page entry and single entry is a `JournalEntryView` with `provenance` (`ProvenanceLink`: `journal_only`, or the operation ID, object reference, the version's PROV-O record path and the operation's history path; failed and refused entries and `file.download` are journal-only); `names=true` adds `display_name` through the host-installed `JournalNames` (resolved per caller at read time, at most 200 objects per page, never stored or exported). `JournalStore::entry` reads one entry; `JournalQuery` gains the three filters in both stores (SQLite through `json_each` over the targets and an object-reference prefix).
- **Export** (at most 50,000 entries per response, continued with `X-Journal-Next-Cursor`, in the requested `order`; `limit` and `names` are ignored): the same filters plus `format=jsonl` (default) or `format=csv` (`journal::csv`: RFC 4180, header row, CRLF, quoting of commas, quotes and line breaks, targets space-separated); every export is journaled as `journal.export` with its format and entry count (or `failed` with `internal`).
- **Engine warnings** (`trace_layer`): `activity_span(operation_id, object_ref)` names the operation for the events inside it; the journal layer now keeps those span fields and gives each event every field it does not name itself, from the nearest enclosing span that names it; an engine event that names its own `object` keeps it and takes no object reference from the span. A warning or error from a `filesquid_ocfl` target is journaled as `ocfl.engine.<module path>` (for example `ocfl.engine.write.lease`) with its `object` field as a target and `object@version` as object reference; its other fields (paths, holders) stay on stdout. The canonical snapshot and file-original commits, the repair provenance commit and the backfill commit run inside an activity span of their operation.
- **Library intents** (`filesquid-native` catalog): recording an intent writes a `library.intent.recorded` entry (operation ID, library, `operation:<name>`, `revision:<n>`) into the journal outbox in the intent's own transaction, once per operation, and holds it there while the intent is pending (no forward, not even by another commit's); the commit that clears the intent releases it, and a commit that changes nothing withdraws it with the intent (no activity, no entry: the owner's no-op exception).
- **Metrics snapshots**: `MonitorConfig::metrics_snapshot_every_ms` (default `None`, off); when set, `Monitor::tick` journals one `metrics.snapshot` entry per interval with one `<metric>:<value>` target per metric.
- **Contract**: `openapi.json` 0.13.0 (PR #94 takes 0.12.0): the routes above, `ErrorCode` gains `payload_too_large` and `rate_limited`, responses `PayloadTooLarge` and `RateLimited`, schemas `ProvenanceLink`, `JournalEntryView`, `JournalViewPage`, `ClientConsoleEvent`, `ClientOverflowEvent`, `ClientEventsRequest`, `ClientEventsAck`, and `JournalEntry.client_event_id`. `ui/src/api/schema.d.ts` regenerated; the UI client's exhaustive error-code list gains the two codes (the only hand edit in `@filesquid/ui`).

## Not delivered in this task, and why

- **`fsquid` CLI journaling** (API-S-031): needs an owner decision on where a host-less CLI run journals and whether `filesquid-cli` may link `filesquid-api` and `filesquid-native` ([Conflict Artifact](./Task-API-F2.10-Conflict.md)). The plan box stays open until it is resolved.
- **Engine warning context (open, raised to the owner in the [Conflict Artifact](./Task-API-F2.10-Conflict.md#6-also-open-engine-warnings-without-full-context)):** API-S-031 asks for every engine warning with both an operation ID and an object reference. The Git and storage-hierarchy guards return errors, which their callers already journal as failed operations. Two warnings still lack context: the copy guard's warning names only paths, so it gets the operation ID of its activity span and no object (the fix is in `filesquid-ocfl`, a module crate this lane does not edit); and the file routes (`/api/v1/files/...`) carry no operation ID yet (their version message is the caller's), so engine warnings during those commits carry none.
- **Host wiring of `JournalNames`:** the seam is in the API; the desktop host installs a resolver with the display-name work of API-F1.13 or DE-0.11. Until then `names=true` returns no names.

## SRS traceability

| Requirement | Tests |
|---|---|
| API-S-030 bounds (20 events, 16 KiB, exactly 16 KiB accepted) | `unit_client_events::more_than_twenty_events_are_refused_with_an_error_code`, `a_body_over_sixteen_kibibytes_is_refused_with_an_error_code`, `a_body_of_exactly_sixteen_kibibytes_is_accepted`; BDD "Client events are bounded" |
| API-S-030 one entry per event after 429 and retry; rolling window; refused requests counted | `malformed_and_oversized_requests_count_toward_the_rate_bound`, `a_lone_overflow_event_naming_another_principal_is_counted_toward_the_rate_bound`, `the_sixty_first_request_in_a_minute_gets_429_and_every_event_is_journaled_after_retry`, `the_rate_bound_rolls_with_the_window`, `a_full_batch_is_journaled_one_entry_per_event`; limiter unit tests |
| API-S-030 de-duplication by client event ID, across restarts | native `a_preset_id_repeating_a_held_client_event_id_answers_the_holder`, `a_resent_batch_is_acknowledged_without_new_entries`, `one_id_twice_in_one_request_is_journaled_once`; native `unit_journal::both_stores_store_each_client_event_id_once`, `client_event_de_duplication_survives_a_restart`, `an_older_journal_gains_the_client_event_id_column_on_open`; BDD "A client event resent after a lost acknowledgement adds no entry" |
| API-S-030 principal check | `an_event_raised_by_another_principal_is_refused_and_journaled`, `an_event_raised_by_the_same_principal_is_accepted`; BDD "An event raised by another principal is refused" |
| API-S-030 overflow schema and count bounds | `the_overflow_count_must_be_an_integer_from_one_to_a_million`, `the_overflow_event_carries_nothing_but_its_schema`; BDD "The overflow count is bounded and exempt from the rate bound" |
| API-S-030 overflow rate-bound exemption | `a_lone_overflow_event_is_exempt_from_the_rate_bound`, `lone_overflow_requests_do_not_count_toward_the_rate_bound` |
| API-S-030 message text never stored; opaque structural fields | `message_text_urls_file_names_and_stacks_never_reach_the_journal`, `structural_fields_that_are_not_opaque_tokens_are_refused`; BDD "Browser console errors reach the journal without their text" |
| API-S-030 acknowledgement after the durable write | `the_acknowledgement_follows_the_durable_write` |
| API-S-031 filters (library, object, file, operation ID) | `unit_journal_query::the_journal_filters_by_library`, `the_journal_filters_by_object_id_and_by_file`, `the_journal_filters_by_operation_id`, `the_new_filters_combine_with_the_existing_ones`, `invalid_filter_values_are_refused`, `the_store_query_filters_match_the_route_filters`; native `both_stores_filter_by_library_object_and_operation_id`; BDD "The journal filters by file and by operation ID" |
| API-S-031 one entry by ID | `one_entry_is_returned_by_id` |
| API-S-031 PROV-O link and journal-only | `committed_entries_link_to_their_prov_o_activity_and_others_are_journal_only`; BDD "An entry links to its PROV-O activity or is marked journal-only" |
| API-S-031 display names (read time, caller, never stored or exported) | `display_names_are_resolved_at_read_time_on_request_only`, `without_a_name_resolver_no_names_are_returned`, `the_export_takes_the_same_filters_and_never_carries_names`; BDD "Display names are shown for display and never exported" |
| API-S-031 export filters, CSV RFC 4180, `journal.export` | `the_csv_export_is_rfc_4180_with_one_row_per_entry`, `csv_fields_with_commas_quotes_or_line_breaks_are_quoted`, `a_csv_row_names_every_column_of_the_header_in_order`, `every_export_is_journaled_as_journal_export`, `an_unknown_export_format_is_refused`; BDD "The owner exports the journal as CSV", "A non-owner cannot export the journal" |
| API-S-031 engine warnings with operation and object reference | `unit_journal_coverage::an_engine_lease_warning_names_the_engine_operation_and_the_object_version`, `an_engine_guard_warning_inherits_the_object_reference_of_its_activity_span`, `an_event_field_wins_over_its_span_field`, `a_warning_outside_any_activity_span_carries_no_operation_id`, `an_engine_event_naming_its_object_never_takes_another_object_from_its_span`; BDD "OCFL engine warnings carry the operation and the object version" |
| API-S-031 `library.intent.recorded` | native `an_intent_recorded_before_its_commit_is_journaled_once`, native `unit_catalog::a_recorded_intent_is_journaled_as_library_intent_recorded`, `a_command_journals_its_intent_before_its_commit`, `a_commit_without_change_keeps_the_revision_and_stores_nothing`; `unit_canonical_commit::a_recorded_no_op_commits_no_version_and_journals_nothing` |
| API-S-031 `metrics.snapshot` off by default | `a_snapshot_interval_that_is_not_positive_journals_nothing`, `a_backward_clock_step_does_not_suppress_snapshots`, `a_snapshot_records_the_health_the_same_tick_evaluated`, `metrics_snapshots_are_off_by_default`, `enabled_metrics_snapshots_are_journaled_once_per_interval_with_opaque_values`; BDD "Metrics snapshots are off by default" |
| API-S-017 redaction of every new event type | `every_new_client_event_type_is_redacted_in_the_journal` (`ui.console.warn`, `ui.console.error`, `ui.console.overflow`), `every_export_is_journaled_as_journal_export` (`journal.export`), `a_recorded_intent_is_journaled_as_library_intent_recorded`, `enabled_metrics_snapshots_are_journaled_once_per_interval_with_opaque_values`, `an_engine_guard_warning_inherits_the_object_reference_of_its_activity_span`, native `a_client_event_id_that_is_not_opaque_is_dropped_and_its_entry_kept` |
| Contract (API-S-002) | `unit_contract` (every schema sampled; documented statuses and bodies, error codes 413 and 429), `ui` `contract.test.ts` |

## TDD: failing run before implementation

The three new API test files, built against `origin/main` (`c157658`):

```text
error: could not compile `filesquid-api` (test "unit_client_events") due to 3 previous errors
error: could not compile `filesquid-api` (test "unit_journal_coverage") due to 4 previous errors
error: could not compile `filesquid-api` (test "unit_journal_query") due to 2 previous errors
error[E0432]: unresolved import `filesquid_api::client_events`
error[E0432]: unresolved import `filesquid_api::journal::csv`
error[E0432]: unresolved import `filesquid_api::monitor::METRICS_SNAPSHOT_OPERATION`
error[E0432]: unresolved import `filesquid_api::trace_layer::activity_span`
error[E0432]: unresolved imports `filesquid_api::admin::JournalNames`, `filesquid_api::admin::EXPORT_OPERATION`
error[E0609]: no field `client_event_id` on type `&JournalEntry`
error[E0609]: no field `metrics_snapshot_every_ms` on type `MonitorConfig`
```

The two native intent tests failed against the catalog before `library.intent.recorded` existed:

```text
test a_recorded_intent_is_journaled_as_library_intent_recorded ... FAILED
test a_command_journals_its_intent_before_its_commit ... FAILED
test result: FAILED. 2 passed; 2 failed; 0 ignored; 0 measured; 29 filtered out; finished in 0.20s
```

## End-to-end check against the SQLite store

`filesquid-server` on a private port and data directory, the README commands:

```text
id,time,host,process_id,actor,operation,outcome,level,target_ids,duration_ms,error_code,operation_id,object_ref,client_event_id
1,2026-10-20T04:00:05.857Z,filesquid-server,,system,host.started,succeeded,info,filesquid-server,,,,,
3,2026-10-20T04:00:05.857Z,filesquid-server,,system,api.auth.token_issued,succeeded,info,administrator,,,,,
{"acknowledged":["5f0c3e44-2a7b-4c1d-9e8f-0123456789ab"],"journaled":1,"principal":"administrator"}
{"entries":[{"id":7,"time":"2026-10-20T04:00:11.404Z","host":"filesquid-server","actor":"administrator","operation":"ui.console.warn","outcome":"info","level":"warn","target_ids":["component:readme-check"],"error_code":"example","client_event_id":"5f0c3e44-2a7b-4c1d-9e8f-0123456789ab","provenance":{"journal_only":true}}],"next_cursor":null}
{"id":1,"time":"2026-10-20T04:00:05.857Z","host":"filesquid-server","actor":"system","operation":"host.started","outcome":"succeeded","level":"info","target_ids":["filesquid-server"],"provenance":{"journal_only":true}}
{"entries":[{"id":6,"time":"2026-10-20T04:00:11.395Z","host":"filesquid-server","actor":"administrator","operation":"journal.export","outcome":"succeeded","level":"info","target_ids":["format:csv","entries:2"],"provenance":{"journal_only":true}}],"next_cursor":null}
```

## Coverage

No coverage tool is configured for the Rust crates (none is named in the ADRs); every constraint above has a named test, and the pre-commit hook runs the full suite (`FULL=1`).

## Mocked Environment Variables

None.

## Raw test output

`cargo test -p filesquid-api --locked --test unit_client_events --test unit_journal_query --test unit_journal_coverage`:

```text
     Running tests/unit/client_events.rs (<target>/debug/deps/unit_client_events-23ddb5272caae5fb)
test a_caller_without_a_credential_is_refused ... ok
test a_body_over_sixteen_kibibytes_is_refused_with_an_error_code ... ok
test a_full_batch_is_journaled_one_entry_per_event ... ok
test a_body_of_exactly_sixteen_kibibytes_is_accepted ... ok
test a_resent_batch_is_acknowledged_without_new_entries ... ok
test an_event_raised_by_another_principal_is_refused_and_journaled ... ok
test an_event_raised_by_the_same_principal_is_accepted ... ok
test an_event_time_is_kept_and_a_far_future_time_is_refused ... ok
test every_new_client_event_type_is_redacted_in_the_journal ... ok
test malformed_and_oversized_requests_count_toward_the_rate_bound ... ok
test malformed_json_and_an_empty_batch_are_refused ... ok
test message_text_urls_file_names_and_stacks_never_reach_the_journal ... ok
test more_than_twenty_events_are_refused_with_an_error_code ... ok
test a_lone_overflow_event_naming_another_principal_is_counted_toward_the_rate_bound ... ok
test one_id_twice_in_one_request_is_journaled_once ... ok
test the_acknowledgement_follows_the_durable_write ... ok
test structural_fields_that_are_not_opaque_tokens_are_refused ... ok
test the_overflow_count_must_be_an_integer_from_one_to_a_million ... ok
test the_overflow_event_carries_nothing_but_its_schema ... ok
test a_lone_overflow_event_is_exempt_from_the_rate_bound ... ok
test lone_overflow_requests_do_not_count_toward_the_rate_bound ... ok
test the_sixty_first_request_in_a_minute_gets_429_and_every_event_is_journaled_after_retry ... ok
test the_rate_bound_rolls_with_the_window ... ok
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
     Running tests/unit/journal_coverage.rs (<target>/debug/deps/unit_journal_coverage-8c97066444502fba)
test a_snapshot_records_the_health_the_same_tick_evaluated ... ok
test a_warning_outside_any_activity_span_carries_no_operation_id ... ok
test a_backward_clock_step_does_not_suppress_snapshots ... ok
test a_snapshot_interval_that_is_not_positive_journals_nothing ... ok
test an_engine_event_naming_its_object_never_takes_another_object_from_its_span ... ok
test an_engine_guard_warning_inherits_the_object_reference_of_its_activity_span ... ok
test an_event_field_wins_over_its_span_field ... ok
test an_engine_lease_warning_names_the_engine_operation_and_the_object_version ... ok
test metrics_snapshots_are_off_by_default ... ok
test enabled_metrics_snapshots_are_journaled_once_per_interval_with_opaque_values ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/journal_query.rs (<target>/debug/deps/unit_journal_query-3c0068aa274fd662)
test csv_fields_with_commas_quotes_or_line_breaks_are_quoted ... ok
test a_csv_row_names_every_column_of_the_header_in_order ... ok
test an_unknown_export_format_is_refused ... ok
test committed_entries_link_to_their_prov_o_activity_and_others_are_journal_only ... ok
test display_names_are_resolved_at_read_time_on_request_only ... ok
test every_export_is_journaled_as_journal_export ... ok
test one_entry_is_returned_by_id ... ok
test invalid_filter_values_are_refused ... ok
test the_csv_export_is_rfc_4180_with_one_row_per_entry ... ok
test the_journal_filters_by_library ... ok
test the_export_takes_the_same_filters_and_never_carries_names ... ok
test the_store_query_filters_match_the_route_filters ... ok
test the_journal_filters_by_operation_id ... ok
test without_a_name_resolver_no_names_are_returned ... ok
test the_journal_filters_by_object_id_and_by_file ... ok
test the_new_filters_combine_with_the_existing_ones ... ok
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

`cargo test -p filesquid-native --features desktop --locked --test unit_journal --test unit_catalog --test unit_canonical_commit`:

```text
     Running tests/unit/canonical_commit.rs (<target>/debug/deps/unit_canonical_commit-9a549358e477d6e3)
test a_commit_without_journal_ids_publishes_and_warns_without_identifying_data ... ok
test a_command_publishes_one_snapshot_version_before_the_catalog_answers ... ok
test a_crash_after_the_snapshot_loses_no_acknowledged_state ... ok
test a_crash_after_the_snapshot_replays_the_commits_journal_entries_once ... ok
test a_crash_before_the_snapshot_leaves_the_work_pending_and_a_retry_completes_once ... ok
test a_library_directory_that_is_a_symbolic_link_is_refused ... ok
test a_head_without_its_version_inventory_is_refused_and_not_repaired ... ok
test a_failed_index_refresh_is_journaled_as_failed ... ok
test a_missing_snapshot_is_published_again_in_full_by_the_next_commit ... ok
test a_refresh_of_an_unknown_library_is_journaled_as_failed ... ok
test a_deleted_catalog_is_rebuilt_from_the_snapshot_with_its_receipts ... ok
test a_recorded_no_op_commits_no_version_and_journals_nothing ... ok
test a_snapshot_of_another_library_is_never_loaded ... ok
test a_recorded_index_refresh_that_changes_nothing_journals_nothing ... ok
test an_unrecorded_catalog_still_links_its_commits ... ok
test an_unpublished_version_left_by_a_crash_is_ignored_and_replaced ... ok
test every_library_keeps_its_ocfl_root_beside_its_catalog_file ... ok
test an_index_refresh_that_changes_nothing_journals_nothing ... ok
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running tests/unit/catalog.rs (<target>/debug/deps/unit_catalog-5f5bd07b48c02b9d)
test a_catalog_file_that_is_a_symbolic_link_is_not_followed ... ok
test a_commit_without_change_keeps_the_revision_and_stores_nothing ... ok
test a_failed_commit_leaves_the_catalog_unchanged ... ok
test a_command_journals_its_intent_before_its_commit ... ok
test a_library_id_that_is_not_a_uuid_is_refused_without_a_file ... ok
test a_command_that_changes_nothing_stores_no_receipt_or_journal_entry ... ok
test a_failed_create_leaves_no_catalog_file ... ok
test a_consistent_backup_holds_the_committed_library_and_is_journaled ... ok
test a_recorded_intent_is_journaled_as_library_intent_recorded ... ok
test a_renamed_catalog_file_is_not_served_under_another_library ... ok
test a_repeated_operation_id_answers_its_first_receipt_even_after_reopen ... ok
test a_receipt_in_one_library_answers_a_commit_naming_another ... ok
test an_unusable_root_fails_without_naming_its_path ... ok
test catalog_files_are_owner_only ... ok
test an_intent_recorded_after_its_commit_is_not_stored ... ok
test an_intent_recorded_before_its_commit_is_journaled_once ... ok
test a_version_1_catalog_is_migrated_and_brought_to_its_snapshot ... ok
test creating_an_existing_library_conflicts ... ok
test each_library_has_its_own_catalog_file_named_by_its_id ... ok
test deleting_a_collection_keeps_its_files_as_unfiled ... ok
test entries_the_host_journal_refused_are_forwarded_at_the_next_open ... ok
test every_commit_journal_entry_reaches_the_host_journal_redacted ... ok
test folders_are_indexed_and_read_back ... ok
test committed_state_survives_a_reopen ... ok
test libraries_list_by_repository_in_creation_order ... ok
test pending_intent_is_recorded_and_cleared_by_its_commit ... ok
test listings_page_by_order_then_id_and_hide_hidden_and_deleted_files ... ok
test removing_a_catalog_file_loses_only_the_rebuildable_index ... ok
test the_catalog_directory_name_is_stable ... ok
test the_schema_is_versioned_and_a_newer_catalog_is_refused ... ok
test revisions_start_at_one_advance_on_change_and_stale_ones_conflict ... ok
test deleting_a_virtual_collection_or_purging_a_file_removes_its_memberships ... ok
test the_sqlite_catalog_answers_like_the_api_reference_catalog ... ok
test no_more_than_the_library_limit_can_be_created ... ok
test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.63s
     Running tests/unit/journal.rs (<target>/debug/deps/unit_journal-4cbbdf1df5b36e5d)
test a_preset_id_repeating_a_held_client_event_id_answers_the_holder ... ok
test a_client_event_id_that_is_not_opaque_is_dropped_and_its_entry_kept ... ok
test an_older_journal_gains_the_client_event_id_column_on_open ... ok
test append_then_query_returns_entries_in_order_with_ids ... ok
test an_older_journal_gains_the_linkage_columns_on_open ... ok
test both_stores_assign_ids_before_the_entries_are_written ... ok
test both_stores_filter_by_library_object_and_operation_id ... ok
test both_stores_keep_the_operation_id_and_object_reference ... ok
test both_stores_store_each_client_event_id_once ... ok
test descending_queries_page_newest_first ... ok
test entries_serialize_rfc3339_utc_time_and_lowercase_enums ... ok
test client_event_de_duplication_survives_a_restart ... ok
test log_file_names_match_the_original_suffix_rule ... ok
test opaque_identifiers_are_kept ... ok
test domain_operations_are_journaled_in_the_callers_transaction ... ok
test identifiers_that_are_not_opaque_are_redacted_by_both_stores ... ok
test opening_creates_the_database_and_writes_no_log_files ... ok
test filters_and_cursor_paging ... ok
test retention_keeps_everything_by_default_and_journals_its_own_deletions ... ok
test the_sqlite_journal_and_its_wal_are_owner_only ... ok
test sqlite_journal_is_append_only ... ok
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

`cargo test -p filesquid-api --locked --test unit_contract --test unit_auth --test unit_admin`:

```text
     Running tests/unit/admin.rs (<target>/debug/deps/unit_admin-e871abc0e5591625)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/auth.rs (<target>/debug/deps/unit_auth-2556111050a7c4ab)
test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
     Running tests/unit/contract.rs (<target>/debug/deps/unit_contract-841ff359eb640360)
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.37s
```

`cargo test -p filesquid-api --locked --test features` (the durable journal feature; whole run `300 scenarios (194 passed, 106 skipped)`).

The skipped steps are steps with no definition yet, none of them in this task's scope: in this feature, the per-commit journal entries, the sealing scenarios (hourly, library close, app close, failed seal) and the database-deleted rebuild are bound by API-F2.11, CL-1.9 and DE-0.11; the UI queue, IndexedDB, Logs view and cross-account scenarios by UI-F2.15 and UI-F2.16; in other features, scenarios of operations not built yet (server accounts, import, search, comments), which predate this change. Every scenario of API-S-030 and API-S-031 that the API alone can run is bound and passes:

```text
Feature: Durable, complete and browsable activity journal
  Scenario: A committed operation carries its journal entries in the same OCFL commit
   ✔> Given a desktop library with recorded activity
   ?  When I rename a file
      Step skipped: tests/features/durable_journal_and_journal_tab_F-061.feature:18:5
  Scenario: Other events are sealed every hour
   ✔> Given a desktop library with recorded activity
   ?  Given an authorisation refusal and a health change were journaled
      Step skipped: tests/features/durable_journal_and_journal_tab_F-061.feature:23:5
  Scenario: Library events are sealed on library close
   ✔> Given a desktop library with recorded activity
   ?  Given library entries that are not yet sealed
      Step skipped: tests/features/durable_journal_and_journal_tab_F-061.feature:29:5
  Scenario: Host events are sealed on app close
   ✔> Given a desktop library with recorded activity
   ?  Given host entries that are not yet sealed
      Step skipped: tests/features/durable_journal_and_journal_tab_F-061.feature:34:5
  Scenario: Database deleted, rebuild restores the journal
   ✔> Given a desktop library with recorded activity
   ?  Given committed operations and other entries sealed at the last hourly seal
      Step skipped: tests/features/durable_journal_and_journal_tab_F-061.feature:39:5
  Scenario: A failed seal drops nothing
   ✔> Given a desktop library with recorded activity
   ?  Given the library's storage root is not writable during a seal
      Step skipped: tests/features/durable_journal_and_journal_tab_F-061.feature:49:5
  Scenario: A browser console warning reaches the journal
   ✔> Given a desktop library with recorded activity
   ✔  When the UI reports a console warning
   ✔  Then the journal should hold one "ui.console.warn" entry with the component, error code and opaque IDs
  Scenario: Browser console errors reach the journal without their text
   ✔> Given a desktop library with recorded activity
   ✔  When the UI reports a console error with a message that contains a file name
   ✔  Then the journal should hold one "ui.console.error" entry with the component, error code and opaque IDs
   ✔  And the entry should not contain the message text
  Scenario: Client events are bounded
   ✔> Given a desktop library with recorded activity
   ✔  When the UI sends 21 events in one request
   ✔  Then the request should be refused with an error code
   ✔  And a caller over the rate bound should receive status 429 with a Retry-After header
  Scenario: Rate limits delay client events but never merge them
   ✔> Given a desktop library with recorded activity
   ?  Given the UI has 150 console warnings queued
      Step skipped: tests/features/durable_journal_and_journal_tab_F-061.feature:68:5
  Scenario: A full client queue spills to IndexedDB and loses nothing
   ✔> Given a desktop library with recorded activity
   ?  Given the host has been unreachable and the UI's in-memory queue holds 10,000 console warnings
      Step skipped: tests/features/durable_journal_and_journal_tab_F-061.feature:75:5
  Scenario: Events queued at page close are persisted and delivered on the next open
   ✔> Given a desktop library with recorded activity
   ?  Given the UI has 2 console errors already delivered and 3 console errors the route has not yet accepted
      Step skipped: tests/features/durable_journal_and_journal_tab_F-061.feature:82:5
  Scenario: A refused IndexedDB write after the queue is full is counted at page close
   ✔> Given a desktop library with recorded activity
   ?  Given the host has been unreachable and the UI's in-memory queue holds 10,000 console warnings
      Step skipped: tests/features/durable_journal_and_journal_tab_F-061.feature:90:5
  Scenario: Persisted events are never delivered under another account's credential
   ✔> Given a desktop library with recorded activity
   ?  Given "ana" left 3 undelivered console errors persisted on this device for the server host
      Step skipped: tests/features/durable_journal_and_journal_tab_F-061.feature:97:5
  Scenario: Without IndexedDB the events the UI could not keep are counted at page close
   ✔> Given a desktop library with recorded activity
   ?  Given IndexedDB is unavailable in the page
      Step skipped: tests/features/durable_journal_and_journal_tab_F-061.feature:103:5
  Scenario: A client event resent after a lost acknowledgement adds no entry
   ✔> Given a desktop library with recorded activity
   ✔  When the UI sends a console warning twice with the same client event ID
   ✔  Then both requests should be acknowledged
   ✔  And the journal should hold one "ui.console.warn" entry for that client event ID
  Scenario: An event raised by another principal is refused
   ✔> Given a desktop library with recorded activity
   ✔  When the UI sends a console warning raised by another principal
   ✔  Then the request should be refused as not permitted and journaled as a refusal
   ✔  And no "ui.console.warn" entry should be written
  Scenario: The overflow count is bounded
   ✔> Given a desktop library with recorded activity
   ✔  When the UI sends one overflow event with the count 1000000
   ✔  Then the journal should hold one "ui.console.overflow" entry with the count 1000000
   ✔  And an overflow event with the count 0 should be refused with an error code
   ✔  And an overflow event with the count 1000001 should be refused with an error code
  Scenario: A lone overflow event is exempt from the rate bound
   ✔> Given a desktop library with recorded activity
   ✔  Given the caller has used up the rate bound
   ✔  When the UI sends one overflow event with the count 3
   ✔  Then the journal should hold one "ui.console.overflow" entry with the count 3
  Scenario: The journal filters by file and by operation ID
   ✔> Given a desktop library with recorded activity
   ✔  When I filter the journal by the file
   ✔  Then I should see only the entries about that file
   ✔  When I filter the journal by the operation ID of the rename
   ✔  Then I should see only the entries of that operation
  Scenario: An entry links to its PROV-O activity or is marked journal-only
   ✔> Given a desktop library with recorded activity
   ✔  When I open the journal entry of the committed rename
   ✔  Then it should link to the PROV-O record of its object version
   ✔  When I open the journal entry of the failed move
   ✔  Then it should be marked journal-only
  Scenario: Display names are shown for display and never exported
   ✔> Given a desktop library with recorded activity
   ✔  Given the host resolves display names for the owner
   ✔  When I read the journal with names and export it
   ✔  Then the page should show the file's display name
   ✔  And the export should carry opaque IDs only
  Scenario: OCFL engine warnings carry the operation and the object version
   ✔> Given a desktop library with recorded activity
   ✔  When the OCFL engine warns about a lease takeover inside an operation
   ✔  Then the journal entry should name the engine operation, the operation ID and the object version
  Scenario: Metrics snapshots are off by default
   ✔> Given a desktop library with recorded activity
   ✔  When the monitor ticks with its default settings
   ✔  Then no "metrics.snapshot" entry should be written
  Scenario: The owner browses and filters the Logs view
   ✔> Given a desktop library with recorded activity
   ?  When I open the Logs view and filter by operation "metadata.edit" and the last day
      Step skipped: tests/features/durable_journal_and_journal_tab_F-061.feature:158:5
  Scenario: The owner exports the journal as CSV
   ✔> Given a desktop library with recorded activity
   ✔  When I export the journal filtered by library and level "warn" as CSV
   ✔  Then a CSV file with one row per matching entry and opaque IDs only should be downloaded
   ✔  And the journal should record a "journal.export" entry
  Scenario: A non-owner cannot export the journal
   ✔> Given a desktop library with recorded activity
   ✔  Given a caller without the owner credential
   ✔  When it requests the journal export
   ✔  Then the request should be refused and journaled as a refusal
  Scenario: A non-owner cannot open the Logs view
   ✔> Given a desktop library with recorded activity
   ✔  Given a caller without the owner credential
   ?  When it opens the Logs view
      Step skipped: tests/features/durable_journal_and_journal_tab_F-061.feature:175:5
```
