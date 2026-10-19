# Task API-F1.3 Walkthrough: File operation routes

Branch: `feat/api-f1.3-file-operation-routes`. Module: `filesquid-api`, with host wiring in `filesquid-native`, `filesquid-server` and `filesquid-desktop`, and the regenerated contract types in `@filesquid/ui`.

**Status:** complete. Acceptance gate (1) passes. Acceptance gate (2) is settled by owner decision (2026-10-19): on WebKitGTK `<img>` and ranged reads through content handles pass, and a `<video>` does not play from the `filesquid://` custom scheme. That is a known limitation for release 1, recorded in ADR 030; the planned fix is the MediaSource player of [UI-F3.3](../../../../ui/resources/plan.md), and the loopback port is rejected. WKWebView is checked by hand in Task FS-3.9 (if it fails the same way, UI-F3.3 moves into the MVP), and WebView2 with Task FS-3.11.

## What changed

- **File routes** (`crates/api/src/files/`): `GET /api/v1/files/listing` (logical-path prefix, opaque cursor, 1 to 1,000 entries, default 200), `GET /api/v1/files/details`, `GET /api/v1/files/history`, `GET /api/v1/files/versions`, `GET` and `HEAD /api/v1/files/content` (one `Range` per request; `206`, `416` with `bytes */<length>`, at most 16 MiB per answer from at most two 8 MiB view reads; a malformed or multi-range header, repeated `Range` fields included, is ignored; the `bytes` unit is case-insensitive; a `HEAD` with a `Range` answers the status and headers of that `GET`, and one without describes the whole file), `PUT /api/v1/files/upload` (at most 64 MiB), `POST /api/v1/files/folders` (writes nothing; answers `pending`), `POST /api/v1/files/moves`, `POST /api/v1/files/removals` and `POST /api/v1/files/restorations`. Reads call the IO-1.2 view API on an opened object; every write calls the view API's version-producing call on the `filesquid-ocfl` change model, so it publishes exactly one version or is refused without publishing. Answers carry logical paths, versions and digests, never a filesystem path, storage key or OCFL content path. The host injects each library's root, leases and staging backend (`FileLibrary`); the API does no storage I/O. Content lengths are cached by content digest in a bounded, best-effort cache (it is emptied when it reaches its limit), so the filesystem backend's whole-file `head` usually runs once per content.
- **Content handles** (`crates/api/src/content/`): `POST /api/v1/content-handles` (action `files.read`, like the ranged read) and `GET` and `HEAD /api/v1/content/{handle}` (action `content.read`, guarded by the handle itself). 32 bytes from the OS CSPRNG as 43 base64url characters; only the SHA-256 digest is kept, in memory; 300 s sliding and 3,600 s absolute expiry checked at request start; 1,024 per credential with LRU eviction; expired handles removed at mint; the minting credential (by digest) and its `files.read` permission checked again at every request; one identical `403` `invalid_credentials` denial for every unusable request under `/api/v1/content/`, including the 404 and 405 fallbacks; the four privacy headers on every `/api/v1/content/{handle}` answer and on the mint answer; `api.content_handle.issued` journaled without the handle.
- **Original file names** (owner requirement, 2026-10-19; `crates/api/src/files/names.rs`): listings and details carry `display_name`, the owner's original name from the host's file records (`DisplayNames`, installed with `FileService::install_display_names`), or the stored name when no record holds one; every content answer, on both content routes, carries `Content-Disposition` (`inline`, or `attachment` with `download=true`) with an ASCII `filename` fallback and the display name as RFC 5987 `filename*` UTF-8. Listings, details and headers all use the one safe transformation API-S-005 defines (controls removed, `/` and `\` as `_`, trimmed, at most 1,024 bytes, `file` when nothing is left), applied to the stored name too. Tests: `listings_and_details_show_the_original_name_not_the_stored_one`, `reads_and_downloads_are_named_by_the_original_name` (`Été à Nice 2023.jpg`), `a_hostile_original_name_never_reaches_a_header_as_a_path`, the `names` unit tests, and the BDD scenario *Users see and download a file under its original name*. **Dependency:** the records come from DE-0.3 (PR #77, now on `main`: each original is stored as `<file-uuid>.<ext>` beside its `<file-uuid>.metadata.md` record). Installing a `DisplayNames` that reads those records is host work that follows this task; until a host installs one, the display name is the stored name.
- **Auth and errors**: actions `files.read`, `files.write`, `content.read`; `Authenticator::authenticate_digest`; error code `range_not_satisfiable` (416).
- **Contract 0.9.0** (`openapi.json`; 0.7.0 before `main` moved to 0.8.0): the routes, parameters, responses and schemas; `x-filesquid-credential: content-handle` and `security: []` on the content route; regenerated `ui/src/api/schema.d.ts` and the new code in `ui/src/api/client.ts`.
- **Hosts**: `filesquid-native` gains `HostFiles`, `install_file_service` and `open_fs_library` (leases and staging under `<data dir>/files/<library>/`); both hosts install the service at startup; the desktop's test-instrumented build opens `FILESQUID_E2E_LIBRARY`.
- **Packaged window**: `tests/e2e/specs/content-handles.e2e.mjs` and `tests/e2e/support/media-library.mjs` (builds a 105 MB VP8 WebM and a PNG with `ffmpeg` into an OCFL root).
- **Docs**: ADR 030 (owner-decision amendment, S1 (a) measurement, the rejected loopback alternative), the UI-F3.3 follow-up row with its checklist entry and the FS-3.11 and FS-3.9 notes, product SRS-F-056 test list, README test commands, the API-F1.4 media-type note, the plan and checklist boxes (API-F1.3 and global FS-1.2) and the MVP progress.

## SRS mapping

## API-S-004: Every call is authenticated and authorised before files, SQL or keys are touched

File routes declare `files.read` or `files.write` and run behind the guard; the content route's only credential is its handle, a handle is never a Bearer token and a token never a handle; every path under `/api/v1/content/` gets the content denial, never the 401, 404 or 405 fallback. Tests: `file_routes_need_the_token_and_refusals_are_journaled`, `a_content_handle_cannot_mint_another_or_act_as_a_bearer_token`, `an_authorization_header_never_replaces_the_handle`, `the_denial_is_never_401_or_404_even_without_any_handle`, `unit_auth`, `unit_routes`, server `the_server_answers_the_content_route_with_its_one_denial`.

## API-S-005: Responses carry opaque handles, never raw filesystem paths or SQL

`details_answer_storage_facts_without_any_storage_path`, `no_answer_or_journal_entry_carries_a_filesystem_path`, `a_mint_answers_a_43_character_handle_its_url_and_an_rfc3339_expiry`, BDD `The owner lists a folder by its logical-path prefix`.

## API-S-008: Every content or organisation change commits an OCFL version through the filesquid-ocfl change model; there is no second write path

`an_upload_commits_exactly_one_version_and_is_journaled`, `an_upload_that_changes_nothing_is_refused_without_a_version`, `creating_a_folder_commits_nothing_and_stays_pending`, `a_file_added_beneath_a_pending_folder_persists_it`, `a_move_renames_a_file_or_a_whole_folder_in_one_version`, `a_remove_and_a_restore_each_publish_one_version`, native `a_write_through_the_host_service_publishes_one_version`, BDD `file_operation_routes_API-S-008.feature`.

## API-S-017: The activity journal records every authorisation refusal, credential issue and content handle minted

Refusals journaled (`a_limit_outside_1_to_1000_or_a_malformed_cursor_is_refused`, `every_unusable_handle_gets_one_identical_denial_and_no_file_is_read`); `api.content_handle.issued` with actor, file ID, version and expiry, never the handle (`a_mint_is_journaled_without_the_handle`, `no_log_line_journal_entry_or_debug_output_carries_a_handle`).

## API-S-018: Media elements read file bytes through short-lived, read-only content handles

Every clause through the router with an injected clock (`integration_content_handles`, 26 tests): minting and its refusal (`minting_is_refused_exactly_like_the_ranged_read`), handle form, content route `GET`/`HEAD`/`206`/`416`, one version only (`a_handle_serves_only_the_version_it_names`), idle and absolute expiry, a read that outlives its handle, eviction at 1,024, expired removal, credential end, relaunch, policy re-check, identical denials with no file read, redaction, privacy headers; BDD `content_handles_F-056.feature` (29 scenarios). Packaged window: see acceptance gate (2) below.

## DE-S-005: Every API request through the scheme carries a per-launch token, except the liveness probe and the content route

`content_route_answers_pass_through_the_scheme_unchanged` (denial and `206` with privacy headers through `api_protocol::handle`), the packaged-window spec.

## OCFL-S-012: The engine exposes a serializable UI view API that filesquid-api routes to every UI

The routes call `list_folder`, `file_details`, `file_history`, `object_versions`, `read_range`, `add_file`, `move_path`, `remove_path` and `restore`; page bounds 1 to 1,000; created folders are not persisted.

## Acceptance gates

1. **Passes.** Unit, integration and BDD tests for every API-S-018 clause through the router, including expiry with an injected clock, a range read that outlives its handle's expiry, eviction at 1,024, invalidation when the credential ends, identical denials for unknown, malformed, expired, evicted, revoked and policy-failed handles, a handle never serving another version, `HEAD` answers and the response headers.
2. **Settled by owner decision (2026-10-19).** ADR 030 S1 item (a) on WebKitGTK: minting, ranged reads through a handle with no token, the denial and an `<img>` pass; a `<video>` fails with `MEDIA_ERR_SRC_NOT_SUPPORTED` after one probe range (`bytes=0-1445`, answered `206`). The same bytes play from a `blob:` URL, and the privacy headers make no difference. Recorded in ADR 030 as a known limitation for release 1, fixed by the MediaSource player of UI-F3.3 (follow-up row in the UI plan and the release-1 checklist); WKWebView is checked in the Task FS-3.9 manual test and WebView2 with Task FS-3.11.

## Follow-ups

- **Media type.** API-S-018 names the file's stored media type; nothing stores one until the file record of API-F1.4 (MD-0.8, ADR 038), so content answers carry the type of the extension (`crates/api/src/files/media.rs`), never sniffed. The API-F1.4 row now says to switch to the record's type.
- **Engine `head`.** `FsBackend::head` hashes the whole file for its entity tag, and `file_details` reads the size through it. The API caches each content's length by digest (`a_file_length_is_read_once_then_cached`), so the hash runs once per content; a size-only storage call in `filesquid-ocfl` would remove even that (modules engineer).
- **Takeover validation.** Since PR #78 a commit receipt carries `took_over`; once API-F2.7 is on `main`, the file routes' writes should pass it to the validation service.

## Cubic review

Round 1 on `0d8bd1e`: 47 findings (P1 0, P2 36, P3 11), fixed in `2fad2d3` except three answered on their threads (the server's file-service handle waits for SV-F1.3, an unavailable read is logged at INFO because the refusal is journaled at error, and an unreadable object has no ID to match).

Round 2 on `2692d1d`: 25 findings (P1 1, P2 21, P3 3), fixed in the next commit: the storage root is opened by its canonical path and refused when it is a symbolic link (`a_storage_root_that_is_a_symbolic_link_is_refused`); repeated `Range` fields and a case-insensitive unit; `HEAD` with a `Range` (`head_with_a_range_describes_the_get_for_that_range`, contract `206` and `416` on both `HEAD` operations); a short read is an error, not truncated content; restore paths are validated first; a listing prefix may end in `/`; the stored-name fallback goes through `safe_name`; write contention and incomplete heads are `409 conflict` and an unreadable reservation clock is `503 unavailable`; API-S-005 defines the one safe display-name transformation; and the tests assert the denial message, the mint answer's privacy headers, the journaled missing-credential denials, the `416` and `HEAD` answers through the desktop scheme, exactly one published version, and the whole file for an ignored range.

Round 3 on `0b03fb2`: 4 findings (P1 1, P2 1, P3 2), fixed in the next commit: the storage root is resolved first and then checked without following links, and on Unix the directory at the given path must be the same device and inode as the resolved one, so a swap for a link between the steps is refused; the ranged `HEAD` test asserts `Accept-Ranges` and the media type; an inventory that does not match its sidecar gets its own neutral `409` message (it is not repairable like a missing inventory); the README gives the playback time as about 320 s.

Round 4 on `21f7669`: 0 findings. Then `main` (PRs #84 and #86, PROV-O) was merged in; conflicts were resolved by keeping both sides (file and history routes and actions, contract schemas with one combined `NotFound` text, test targets, and the plan rows). In the next commit every successful file write's journal entry also carries the `object_ref` (`<object-id>@vN`) of ADR 033 Decision 7, and the contract moves to 0.9.0 because this task adds routes on top of `main`'s 0.8.0.

## Mocked Environment Variables

None. `FILESQUID_E2E_LIBRARY` is set by the e2e launcher to the media library it builds; no credential is mocked.

## Test output

TDD: the new tests failed to compile before the implementation:

```text
error[E0432]: unresolved import `filesquid_api::files`
error[E0432]: unresolved import `filesquid_api::content`
error[E0432]: unresolved import `filesquid_api::files`
error[E0407]: method `authenticate_digest` is not a member of trait `Authenticator`
error[E0432]: unresolved import `filesquid_api::files`
error[E0599]: no associated function or constant named `with_clock` found for struct `ApiState` in the current scope
error[E0599]: no method named `authenticate_digest` found for reference `&Switchable` in the current scope
error[E0599]: no variant, associated function, or constant named `ReadFiles` found for enum `Action` in the current scope
error[E0599]: no variant, associated function, or constant named `WriteFiles` found for enum `Action` in the current scope
error[E0599]: no method named `content_handles` found for struct `ApiState` in the current scope
error[E0599]: no method named `content_handles` found for struct `ApiState` in the current scope
error[E0599]: no method named `end_credential` found for struct `ApiState` in the current scope
error[E0599]: no method named `end_credential` found for struct `ApiState` in the current scope
error[E0599]: no method named `content_handles` found for struct `ApiState` in the current scope
error: could not compile `filesquid-api` (test "integration_content_handles") due to 13 previous errors
error: could not compile `filesquid-api` (test "integration_files") due to 7 previous errors
```

`cargo test -p filesquid-api --locked --test integration_files --test integration_content_handles --test unit_contract --test unit_auth --test unit_routes --lib`:

```text
     Running unittests src/lib.rs 
test auth::token::tests::equal_slices_compare_equal_and_every_difference_is_found ... ok
test admin::tests::an_export_that_hits_the_page_cap_returns_a_continuation_cursor ... ok
test admin::tests::a_complete_export_has_no_continuation_cursor_header ... ok
test auth::layer::tests::without_a_monitor_a_refusal_goes_to_stdout_unfiltered ... ok
test content::encoding::tests::only_43_base64url_characters_are_well_formed ... ok
test content::encoding::tests::thirty_two_bytes_become_43_characters_matching_rfc_4648 ... ok
test auth::token::tests::secrets_compare_by_digest_whatever_their_lengths ... ok
test content::store::tests::a_handle_no_longer_allowed_is_refused_and_removed ... ok
test content::store::tests::a_minted_handle_is_accepted_until_it_idles_out ... ok
test content::store::tests::ending_a_credential_ends_only_its_handles ... ok
test content::store::tests::the_store_keeps_digests_and_debug_shows_no_handle ... ok
test files::media::tests::extensions_map_without_case_and_unknown_ones_fall_back ... ok
test files::range::tests::plans_cap_every_answer_and_find_unsatisfiable_ranges ... ok
test files::range::tests::single_byte_ranges_parse ... ok
test journal_writer::tests::a_rejected_write_is_one_json_line_per_record_on_stdout ... ok
test routes::tests::the_liveness_probe_is_the_one_public_path ... ok
test routes::tests::a_route_other_than_the_liveness_probe_cannot_be_public - should panic ... ok
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
     Running tests/integration/content_handles.rs 
test a_handle_expires_after_300_idle_seconds ... ok
test a_handle_ends_with_the_credential_that_minted_it ... ok
test a_content_handle_cannot_mint_another_or_act_as_a_bearer_token ... ok
test a_handle_expires_3600_seconds_after_minting_even_in_use ... ok
test a_handle_serves_only_the_version_it_names ... ok
test a_mint_for_a_missing_file_or_version_is_not_found ... ok
test a_mint_answers_a_43_character_handle_its_url_and_an_rfc3339_expiry ... ok
test a_media_element_reads_a_range_through_the_handle ... ok
test a_mint_is_journaled_without_the_handle ... ok
test a_mint_must_name_one_explicit_version ... ok
test a_relaunch_ends_every_handle ... ok
test an_authorization_header_never_replaces_the_handle ... ok
test an_error_answer_never_echoes_the_handle ... ok
test an_unsatisfiable_range_through_the_handle_keeps_its_416 ... ok
test every_handle_is_new_random_bytes ... ok
test expired_handles_are_removed_when_minting ... ok
test minting_is_refused_exactly_like_the_ranged_read ... ok
test a_range_read_that_outlives_its_handle_is_not_cut_off ... ok
test every_unusable_handle_gets_one_identical_denial_and_no_file_is_read ... ok
test no_log_line_journal_entry_or_debug_output_carries_a_handle ... ok
test head_through_the_handle_describes_the_file_without_a_body ... ok
test the_credential_and_the_policy_are_checked_again_at_every_request ... ok
test playback_keeps_the_handle_alive_within_the_idle_window ... ok
test the_denial_is_never_401_or_404_even_without_any_handle ... ok
test the_1025th_handle_evicts_the_least_recently_used_one ... ok
test the_content_route_refuses_every_other_method ... ok
test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 17.50s
     Running tests/integration/files.rs 
test a_limit_outside_1_to_1000_or_a_malformed_cursor_is_refused ... ok
test a_file_length_is_read_once_then_cached ... ok
test a_file_added_beneath_a_pending_folder_persists_it ... ok
test a_move_of_a_missing_path_is_not_found_and_publishes_nothing ... ok
test a_malformed_or_multi_range_header_is_ignored ... ok
test a_prefix_lists_one_folder_level ... ok
test a_read_without_a_range_answers_the_whole_small_file ... ok
test a_read_pins_the_version_it_names ... ok
test a_move_renames_a_file_or_a_whole_folder_in_one_version ... ok
test a_request_body_with_an_unknown_field_is_invalid ... ok
test a_remove_and_a_restore_each_publish_one_version ... ok
test an_unknown_library_object_version_or_folder_is_not_found_and_journaled ... ok
test an_unsatisfiable_range_answers_416_with_the_length ... ok
test an_upload_above_the_cap_is_refused ... ok
test an_upload_commits_exactly_one_version_and_is_journaled ... ok
test an_upload_that_changes_nothing_is_refused_without_a_version ... ok
test details_answer_storage_facts_without_any_storage_path ... ok
test creating_a_folder_commits_nothing_and_stays_pending ... ok
test file_routes_need_the_token_and_refusals_are_journaled ... ok
test head_describes_the_file_without_reading_its_content ... ok
test history_lists_the_versions_that_changed_a_file ... ok
test no_answer_or_journal_entry_carries_a_filesystem_path ... ok
test pages_follow_the_opaque_cursor_without_a_repeat_or_gap ... ok
test single_ranges_answer_206_with_their_content_range ... ok
test the_root_lists_folders_first_then_files ... ok
test versions_list_every_version_and_the_head ... ok
test a_listing_holds_the_default_page_of_200 ... ok
test answers_are_capped_at_16_mib ... ok
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.13s
     Running tests/unit/auth.rs 
test a_host_route_cannot_be_made_public_through_the_guard ... ok
test a_non_bearer_authorization_header_is_refused_as_invalid_credentials ... ok
test a_present_but_unusable_authorization_header_is_not_a_missing_one ... ok
test a_websocket_upgrade_may_carry_the_token_as_a_subprotocol ... ok
test action_names_are_unique_dotted_identifiers ... ok
test a_not_permitted_denial_names_the_principal_and_the_action ... ok
test an_empty_bearer_token_is_presented_and_then_rejected ... ok
test an_error_without_a_reason_omits_the_member ... ok
test an_unusable_header_never_falls_back_to_the_subprotocol ... ok
test an_unusable_header_on_a_websocket_upgrade_is_refused_even_with_a_valid_subprotocol ... ok
test debug_output_never_shows_a_secret ... ok
test denials_map_reasons_to_codes_and_are_not_retryable ... ok
test an_authoriser_can_be_installed_only_once ... ok
test a_principal_the_policy_does_not_permit_is_forbidden ... ok
test every_denial_is_journaled_without_the_presented_credential ... ok
test a_denial_never_echoes_the_presented_credential ... ok
test every_route_denies_a_call_without_the_per_launch_token ... ok
test every_route_answers_the_per_launch_token_holder ... ok
test every_routed_path_declares_its_action ... ok
test launch_tokens_are_unique_256_bit_hex ... ok
test only_the_liveness_route_is_public ... ok
test host_routes_are_denied_by_default ... ok
test the_bearer_scheme_is_read_case_insensitively ... ok
test the_host_policy_grants_the_owner_and_the_administrator_every_action ... ok
test the_liveness_route_still_refuses_an_anonymous_wrong_method ... ok
test the_subprotocol_carrier_is_read_from_any_offered_entry ... ok
test the_subprotocol_token_counts_only_next_to_the_events_subprotocol ... ok
test the_token_authenticator_accepts_only_the_exact_token ... ok
test fallback_refusals_are_journaled_with_the_no_route_target ... ok
test the_token_issued_entry_has_no_field_that_could_carry_a_secret ... ok
test the_liveness_route_answers_without_a_token_and_says_only_that_the_process_is_up ... ok
test unknown_paths_and_wrong_methods_authenticate_before_routing ... ok
test without_an_installed_authoriser_every_route_is_denied ... ok
test every_route_denies_a_wrong_token ... ok
test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
     Running tests/unit/contract.rs 
test error_code_all_lists_every_variant ... ok
test command_envelope_carries_idempotency_key_expected_revision_and_scope ... ok
test contract_is_openapi_3_1_with_the_crate_api_version ... ok
test authentication_before_routing_is_documented_as_a_wire_convention ... ok
test error_codes_match_the_rust_enum_with_status_and_retryability ... ok
test every_component_schema_has_a_rust_sample ... ok
test errors_carry_code_and_retryability ... ok
test event_topics_in_the_contract_match_the_rust_topics ... ok
test every_documented_operation_has_a_unique_operation_id ... ok
test every_component_schema_matches_its_serialized_rust_type ... ok
test every_error_response_carries_the_api_error_schema ... ok
test every_limit_parameter_is_the_shared_page_size ... ok
test every_event_topic_documents_its_payload_schema ... ok
test every_operation_documents_both_denials ... ok
test every_operation_names_the_action_the_router_authorises ... ok
test every_operation_requires_the_bearer_scheme_and_none_makes_it_optional ... ok
test health_schema_matches_the_core_payload_and_requires_every_field ... ok
test host_routes_are_the_event_channel_only ... ok
test query_envelope_and_page_size_parameter_are_bounded_like_core ... ok
test routed_and_documented_paths_agree_both_ways ... ok
test router_fallbacks_are_documented_as_wire_conventions ... ok
test router_serves_its_own_contract ... ok
test the_checker_accepts_only_truly_unconstrained_objects ... ok
test the_contract_uses_only_checked_schema_keywords ... ok
test the_only_public_operation_is_the_liveness_probe ... ok
test every_documented_operation_answers_a_documented_status_and_body ... ok
test events_from_every_publisher_match_their_documented_payloads ... ok
test router_fallbacks_answer_as_documented ... ok
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.33s
     Running tests/unit/routes.rs 
test fallbacks_answer_the_same_through_every_host_uri_form ... ok
test the_content_denial_answers_the_same_through_every_host_uri_form ... ok
test the_health_route_answers_through_the_desktop_scheme ... ok
test every_routed_path_answers_the_same_through_every_host_uri_form ... ok
test denials_answer_the_same_through_every_host_uri_form ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

`cargo test -p filesquid-api --locked --test features` (content handles and file routes):

```text
Feature: Content handles for media elements
  Scenario: An authorised client mints a content handle for one file version
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  When the client mints a content handle for "holiday.mp4" at version v2
   ✔  Then the answer should carry a handle, its content URL and an absolute expiry in RFC 3339 UTC
   ✔  And the handle should be 43 base64url characters without padding
   ✔  And the content URL should contain neither a filesystem path nor the client's token
   ✔  And the activity journal should record one "api.content_handle.issued" entry without the handle
  Scenario: A media element reads a byte range through the content handle
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  Given the client has minted a content handle for "holiday.mp4" at version v2
   ✔  When a media element requests bytes 0 to 1048575 through the content URL without an Authorization header
   ✔  Then the answer should be 206 with "Content-Range" "bytes 0-1048575/104857600"
   ✔  And the answer should carry "Accept-Ranges" "bytes"
   ✔  And the answer should carry "Content-Type" "video/mp4"
  Scenario: A HEAD request through the content handle describes the file without a body
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  Given the client has minted a content handle for "holiday.mp4" at version v2
   ✔  When a media element sends HEAD through the content URL
   ✔  Then the answer should be 200 with an empty body
   ✔  And the answer should carry "Accept-Ranges" "bytes"
   ✔  And the answer should carry "Content-Type" "video/mp4"
  Scenario: An unsatisfiable range keeps its 416 answer
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  Given the client has minted a content handle for "holiday.mp4" at version v2
   ✔  When a media element requests bytes 104857600 to 104857700 through the content URL
   ✔  Then the answer should be 416 with "Content-Range" "bytes */104857600"
  Scenario Outline: Every content answer carries the privacy headers
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  Given the client has minted a content handle for "holiday.mp4" at version v2
   ✔  When a media element makes a ranged request through the content URL
   ✔  Then the answer should carry "Referrer-Policy" "no-referrer"
   ✔  And the answer should carry "Cache-Control" "private, no-store"
   ✔  And the answer should carry "X-Content-Type-Options" "nosniff"
   ✔  And the answer should carry "Content-Security-Policy" "default-src 'none'; sandbox"
  Scenario Outline: Every content answer carries the privacy headers
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  Given the client has minted a content handle for "holiday.mp4" at version v2
   ✔  When a media element makes a HEAD request through the content URL
   ✔  Then the answer should carry "Referrer-Policy" "no-referrer"
   ✔  And the answer should carry "Cache-Control" "private, no-store"
   ✔  And the answer should carry "X-Content-Type-Options" "nosniff"
   ✔  And the answer should carry "Content-Security-Policy" "default-src 'none'; sandbox"
  Scenario Outline: Every content answer carries the privacy headers
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  Given the client has minted a content handle for "holiday.mp4" at version v2
   ✔  When a media element makes a unsatisfiable request through the content URL
   ✔  Then the answer should carry "Referrer-Policy" "no-referrer"
   ✔  And the answer should carry "Cache-Control" "private, no-store"
   ✔  And the answer should carry "X-Content-Type-Options" "nosniff"
   ✔  And the answer should carry "Content-Security-Policy" "default-src 'none'; sandbox"
  Scenario Outline: Every content answer carries the privacy headers
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  Given the client has minted a content handle for "holiday.mp4" at version v2
   ✔  When a media element makes a refused request through the content URL
   ✔  Then the answer should carry "Referrer-Policy" "no-referrer"
   ✔  And the answer should carry "Cache-Control" "private, no-store"
   ✔  And the answer should carry "X-Content-Type-Options" "nosniff"
   ✔  And the answer should carry "Content-Security-Policy" "default-src 'none'; sandbox"
  Scenario: Playback keeps the handle alive while the idle window is not exceeded
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  Given the client has minted a content handle for "holiday.mp4" at version v2
   ✔  When a media element requests a range every 240 seconds for 50 minutes
   ✔  Then every range request should be answered with 206
  Scenario: A handle expires after 300 idle seconds
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  Given the client has minted a content handle for "holiday.mp4" at version v2
   ✔  When 301 seconds pass without a request through the content URL
   ✔  Then the next request through the content URL should be refused as forbidden with reason "invalid_credentials"
  Scenario: A handle expires 3,600 seconds after minting even while in use
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  Given the client has minted a content handle for "holiday.mp4" at version v2
   ✔  When a media element requests a range every 60 seconds for 3,601 seconds
   ✔  Then the first request after 3,600 seconds should be refused as forbidden with reason "invalid_credentials"
  Scenario: A range read that started before expiry is not cut off
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  Given the client has minted a content handle for "holiday.mp4" at version v2
   ✔  And a media element has started a 16 MiB range read 1 second before the handle expires
   ✔  When the handle expires while the answer is being sent
   ✔  Then the media element should receive all 16 MiB of the answer
  Scenario: The 1,025th handle evicts the least recently used one
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  Given the client has minted 1,024 content handles
   ✔  When the client mints one more content handle
   ✔  Then the least recently used handle should be refused as forbidden with reason "invalid_credentials"
   ✔  And the other 1,024 handles should still be answered
  Scenario Outline: Unusable handles get one identical denial that reveals nothing about the file
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  When a media element requests a range through an unknown handle
   ✔  Then the answer should be 403 forbidden with reason "invalid_credentials" and not retryable
   ✔  And the answer should never be 401 or 404
   ✔  And the answer should be byte for byte the same as for an unknown handle
   ✔  And no file should have been read
   ✔  And the activity journal should record one "api.auth.denied" entry with reason "invalid_credentials"
   ✔  And no log line or journal entry should contain the presented handle
  Scenario Outline: Unusable handles get one identical denial that reveals nothing about the file
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  When a media element requests a range through a malformed handle
   ✔  Then the answer should be 403 forbidden with reason "invalid_credentials" and not retryable
   ✔  And the answer should never be 401 or 404
   ✔  And the answer should be byte for byte the same as for an unknown handle
   ✔  And no file should have been read
   ✔  And the activity journal should record one "api.auth.denied" entry with reason "invalid_credentials"
   ✔  And no log line or journal entry should contain the presented handle
  Scenario Outline: Unusable handles get one identical denial that reveals nothing about the file
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  When a media element requests a range through a missing handle (an empty path segment)
   ✔  Then the answer should be 403 forbidden with reason "invalid_credentials" and not retryable
   ✔  And the answer should never be 401 or 404
   ✔  And the answer should be byte for byte the same as for an unknown handle
   ✔  And no file should have been read
   ✔  And the activity journal should record one "api.auth.denied" entry with reason "invalid_credentials"
   ✔  And no log line or journal entry should contain the presented handle
  Scenario Outline: Unusable handles get one identical denial that reveals nothing about the file
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  When a media element requests a range through an extra path segment after the handle
   ✔  Then the answer should be 403 forbidden with reason "invalid_credentials" and not retryable
   ✔  And the answer should never be 401 or 404
   ✔  And the answer should be byte for byte the same as for an unknown handle
   ✔  And no file should have been read
   ✔  And the activity journal should record one "api.auth.denied" entry with reason "invalid_credentials"
   ✔  And no log line or journal entry should contain the presented handle
  Scenario Outline: Unusable handles get one identical denial that reveals nothing about the file
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  When a media element requests a range through an expired handle
   ✔  Then the answer should be 403 forbidden with reason "invalid_credentials" and not retryable
   ✔  And the answer should never be 401 or 404
   ✔  And the answer should be byte for byte the same as for an unknown handle
   ✔  And no file should have been read
   ✔  And the activity journal should record one "api.auth.denied" entry with reason "invalid_credentials"
   ✔  And no log line or journal entry should contain the presented handle
  Scenario Outline: Unusable handles get one identical denial that reveals nothing about the file
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  When a media element requests a range through a handle for a file its minter may no longer read
   ✔  Then the answer should be 403 forbidden with reason "invalid_credentials" and not retryable
   ✔  And the answer should never be 401 or 404
   ✔  And the answer should be byte for byte the same as for an unknown handle
   ✔  And no file should have been read
   ✔  And the activity journal should record one "api.auth.denied" entry with reason "invalid_credentials"
   ✔  And no log line or journal entry should contain the presented handle
  Scenario Outline: Unusable handles get one identical denial that reveals nothing about the file
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  When a media element requests a range through a handle whose issuing credential has ended
   ✔  Then the answer should be 403 forbidden with reason "invalid_credentials" and not retryable
   ✔  And the answer should never be 401 or 404
   ✔  And the answer should be byte for byte the same as for an unknown handle
   ✔  And no file should have been read
   ✔  And the activity journal should record one "api.auth.denied" entry with reason "invalid_credentials"
   ✔  And no log line or journal entry should contain the presented handle
  Scenario: A handle grants nothing beyond its file version
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  Given the client has minted a content handle for "holiday.mp4" at version v2
   ✔  When a client presents the handle as a Bearer token on another API route
   ✔  Then the call should be refused as forbidden with reason "invalid_credentials"
   ✔  And the content URL should refuse a write method
  Scenario: Minting is authorised exactly like reading the file
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  Given a client whose credential may not read "holiday.mp4"
   ✔  When that client tries to mint a content handle for "holiday.mp4" at version v2
   ✔  Then the minting should get the same refusal as a ranged read of "holiday.mp4" by that client
  Scenario: Handles end with the desktop launch that minted them
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  Given the client has minted a content handle for "holiday.mp4" at version v2 on the desktop
   ✔  When the desktop app is relaunched
   ✔  Then a request through the old content URL should be refused as forbidden with reason "invalid_credentials"
  Scenario: A HEAD denial matches a GET denial without a body
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  When a media element sends HEAD through an unknown handle
   ✔  Then the answer should be 403 with the same headers as a GET denial and an empty body
  Scenario Outline: On filesquid-server a handle ends with its credential or permission
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  Given a filesquid-server where Ana has minted a content handle for "holiday.mp4" at version v2
   ✔  When Ana logs out
   ✔  Then Ana's next request through the content URL should be refused as forbidden with reason "invalid_credentials"
   ✔  And the activity journal should record one "api.auth.denied" entry with reason "invalid_credentials"
  Scenario Outline: On filesquid-server a handle ends with its credential or permission
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  Given a filesquid-server where Ana has minted a content handle for "holiday.mp4" at version v2
   ✔  When Ana's session ends
   ✔  Then Ana's next request through the content URL should be refused as forbidden with reason "invalid_credentials"
   ✔  And the activity journal should record one "api.auth.denied" entry with reason "invalid_credentials"
  Scenario Outline: On filesquid-server a handle ends with its credential or permission
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  Given a filesquid-server where Ana has minted a content handle for "holiday.mp4" at version v2
   ✔  When the server token that minted the handle is rotated
   ✔  Then Ana's next request through the content URL should be refused as forbidden with reason "invalid_credentials"
   ✔  And the activity journal should record one "api.auth.denied" entry with reason "invalid_credentials"
  Scenario Outline: On filesquid-server a handle ends with its credential or permission
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  Given a filesquid-server where Ana has minted a content handle for "holiday.mp4" at version v2
   ✔  When the server restarts
   ✔  Then Ana's next request through the content URL should be refused as forbidden with reason "invalid_credentials"
   ✔  And the activity journal should record one "api.auth.denied" entry with reason "invalid_credentials"
  Scenario Outline: On filesquid-server a handle ends with its credential or permission
   ✔> Given the owner's library holds a 100 MB video "holiday.mp4" at version v2
   ✔> And the owner's client holds a valid credential for the library
   ✔  Given a filesquid-server where Ana has minted a content handle for "holiday.mp4" at version v2
   ✔  When Ana's permission to read "holiday.mp4" is withdrawn
   ✔  Then Ana's next request through the content URL should be refused as forbidden with reason "invalid_credentials"
   ✔  And the activity journal should record one "api.auth.denied" entry with reason "invalid_credentials"
Feature: File operation routes
  Scenario: The owner lists a folder by its logical-path prefix
   ✔> Given the owner's library holds an object with "photos/a.jpg", "photos/b.jpg" and "notes.txt"
   ✔  When the owner lists the folder "photos"
   ✔  Then the listing should name "photos/a.jpg" and "photos/b.jpg"
   ✔  And no answer should carry a filesystem path or a storage key
  Scenario: An upload publishes exactly one OCFL version
   ✔> Given the owner's library holds an object with "photos/a.jpg", "photos/b.jpg" and "notes.txt"
   ✔  When the owner uploads "photos/c.jpg"
   ✔  Then the object should have exactly one new version
   ✔  And the activity journal should record one "files.upload" entry with opaque identifiers only
  Scenario: A move and a restore each publish one version
   ✔> Given the owner's library holds an object with "photos/a.jpg", "photos/b.jpg" and "notes.txt"
   ✔  When the owner moves "photos" to "pictures"
   ✔  And the owner restores "photos" from version v1
   ✔  Then the object should have exactly two new versions
   ✔  And "photos/a.jpg" and "pictures/a.jpg" should both be readable
  Scenario: A new empty folder stays pending until a file is added beneath it
   ✔> Given the owner's library holds an object with "photos/a.jpg", "photos/b.jpg" and "notes.txt"
   ✔  When the owner creates the folder "albums"
   ✔  Then the folder should be pending and no version should be published
   ✔  When the owner uploads "albums/first.jpg"
   ✔  Then the folder "albums" should be listed
  Scenario: Folder listings page with an opaque cursor
   ✔> Given the owner's library holds an object with "photos/a.jpg", "photos/b.jpg" and "notes.txt"
   ✔  When the owner lists the root one entry at a time
   ✔  Then every entry should be listed once, folders first
  Scenario: A ranged read is capped at 16 MiB
   ✔> Given the owner's library holds an object with "photos/a.jpg", "photos/b.jpg" and "notes.txt"
   ✔  Given the object holds a 20 MiB file "big.bin"
   ✔  When the owner reads "big.bin" without a range
   ✔  Then the answer should be 206 with the first 16 MiB
  Scenario: A write that changes nothing is refused without a version
   ✔> Given the owner's library holds an object with "photos/a.jpg", "photos/b.jpg" and "notes.txt"
   ✔  When the owner uploads "notes.txt" with its current content
   ✔  Then the write should be refused as "conflict"
   ✔  And no version should be published
   ✔  And the activity journal should record one refused "files.upload" entry
  Scenario: A client without the token cannot read or write files
   ✔> Given the owner's library holds an object with "photos/a.jpg", "photos/b.jpg" and "notes.txt"
   ✔  When a client without a token reads "notes.txt"
   ✔  And a client without a token removes "notes.txt"
   ✔  Then both calls should be refused as "unauthorized"
   ✔  And no version should be published
[Summary]
14 features
219 scenarios (142 passed, 77 skipped)
692 steps (615 passed, 77 skipped)
```

`cargo test -p filesquid-native --locked --test unit_files`:

```text
     Running tests/unit/files.rs 
test a_state_takes_one_file_service ... ok
test an_opened_library_is_served_and_keeps_its_state_outside_the_root ... ok
test a_write_through_the_host_service_publishes_one_version ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

`cargo test -p filesquid-server --locked --test integration_auth`:

```text
     Running tests/integration/auth.rs 
test a_blank_token_file_open_to_others_is_still_refused ... ok
test a_configured_token_must_have_the_generated_format ... ok
test a_group_or_world_readable_token_file_is_refused ... ok
test a_missing_admin_token_is_generated_into_the_data_dir_with_owner_only_access ... ok
test a_stored_token_that_lacks_the_generated_format_is_refused ... ok
test a_symlinked_token_file_is_refused ... ok
test a_browser_may_send_the_token_as_a_websocket_subprotocol ... ok
test an_owner_only_token_file_is_reused ... ok
test a_server_without_an_installed_authoriser_refuses_every_route ... ok
test every_server_route_refuses_a_wrong_token ... ok
test every_server_route_refuses_a_call_without_the_token ... ok
test other_methods_on_host_routes_are_refused_before_routing ... ok
test every_start_journals_the_administrator_token_issue ... ok
test the_event_channel_refuses_a_websocket_without_the_token ... ok
test the_liveness_probe_answers_without_a_token ... ok
test the_static_ui_stays_public ... ok
test the_server_answers_the_content_route_with_its_one_denial ... ok
test concurrent_first_starts_agree_on_one_token ... ok
test a_blank_owner_only_token_file_left_by_a_crash_is_replaced ... ok
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s
```

`cargo test -p filesquid-desktop --features test-webdriver --locked --test integration_api_protocol`:

```text
     Running tests/integration/api_protocol.rs 
test a_partial_response_passes_through_the_scheme_unchanged ... ok
test an_oversized_response_is_journaled_without_its_detail ... ok
test a_token_from_another_launch_is_refused ... ok
test a_response_over_the_buffer_cap_is_a_typed_internal_error ... ok
test app_origins_may_preflight_the_authorization_header ... ok
test app_origins_may_preflight_the_range_header ... ok
test app_origin_responses_expose_the_journal_cursor_header ... ok
test app_origin_responses_expose_the_range_headers ... ok
test dev_server_origins_are_trusted_only_in_debug_builds ... ok
test each_launch_installs_a_new_token_once ... ok
test app_origins_receive_cors_headers ... ok
test foreign_origins_get_no_preflight_grant ... ok
test foreign_origins_receive_no_cors_headers ... ok
test host_errors_carry_the_api_error_body ... ok
test release_builds_trust_only_the_apps_own_scheme_origins ... ok
test health_is_served_through_the_custom_scheme ... ok
test scheme_name_is_filesquid ... ok
test the_host_descriptor_is_served_through_the_custom_scheme ... ok
test the_desktop_owner_reaches_admin_routes_with_the_per_launch_token ... ok
test content_route_answers_pass_through_the_scheme_unchanged ... ok
test the_scheme_buffers_at_most_sixteen_mebibytes ... ok
test the_liveness_probe_needs_no_token ... ok
test the_init_script_hands_the_token_only_to_app_origins ... ok
test unknown_paths_are_not_found ... ok
test windows_style_scheme_urls_are_served_too ... ok
test windows_style_scheme_urls_allow_and_expose_the_range_headers ... ok
test a_call_without_the_per_launch_token_is_refused ... ok
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

Packaged windows, `xvfb-run -a npm run test:e2e` in `crates/desktop/tests/e2e` (WebKitGTK):

```text
[WebKitGTK 605.1.15 linux #0-0] » specs/capabilities.e2e.mjs
[WebKitGTK 605.1.15 linux #0-0]    ✓ lets the app page listen for and stop listening for FileSquid events
[WebKitGTK 605.1.15 linux #0-0]    ✓ refuses plugin:event|emit
[WebKitGTK 605.1.15 linux #0-0]    ✓ refuses plugin:path|resolve_directory
[WebKitGTK 605.1.15 linux #0-0]    ✓ refuses plugin:window|title
[WebKitGTK 605.1.15 linux #0-0]    ✓ refuses plugin:webview|get_all_webviews
[WebKitGTK 605.1.15 linux #0-0]    ✓ refuses plugin:app|version
[WebKitGTK 605.1.15 linux #0-0]    ✓ refuses plugin:fs|read_text_file
[WebKitGTK 605.1.15 linux #0-0]    ✓ refuses plugin:sql|load
[WebKitGTK 605.1.15 linux #0-0]    ✓ refuses plugin:shell|execute
[WebKitGTK 605.1.15 linux #0-0]    ✓ refuses plugin:dialog|open
[WebKitGTK 605.1.15 linux #0-0]    ✓ refuses plugin:http|fetch
[WebKitGTK 605.1.15 linux #0-0]    ✓ refuses health_ping
[WebKitGTK 605.1.15 linux #0-0]    ✓ never hands the app page a filesystem path
[WebKitGTK 605.1.15 linux #0-0] 13 passing (642ms)
[WebKitGTK 605.1.15 linux #0-1] » specs/content-handles.e2e.mjs
[WebKitGTK 605.1.15 linux #0-1]    ✓ mints a handle whose URL carries no token
[WebKitGTK 605.1.15 linux #0-1]    ✓ reads a range of the 100 MB video through the handle without a token
[WebKitGTK 605.1.15 linux #0-1]    ✓ refuses an unknown handle with the one identical denial
[WebKitGTK 605.1.15 linux #0-1]    ✓ loads an <img> through a content handle
[WebKitGTK 605.1.15 linux #0-1]    ✓ records that WebKitGTK cannot play a <video> from the custom scheme
[WebKitGTK 605.1.15 linux #0-1]    - plays the <video>, seeks, and keeps playing past the 300 s idle window
[WebKitGTK 605.1.15 linux #0-1] 5 passing (4s)
[WebKitGTK 605.1.15 linux #0-1] 1 skipped
[WebKitGTK 605.1.15 linux #0-2] » specs/refusals.e2e.mjs
[WebKitGTK 605.1.15 linux #0-2]    ✓ journals a command the host's IPC gate refuses, without its payload
[WebKitGTK 605.1.15 linux #0-2]    ✓ still lets the app page listen for FileSquid events
[WebKitGTK 605.1.15 linux #0-2]    ✓ journals a forged call on Tauri's postMessage path without either key
[WebKitGTK 605.1.15 linux #0-2]    ✓ refuses and journals a new window
[WebKitGTK 605.1.15 linux #0-2]    ✓ refuses and journals a navigation off the app's origins
[WebKitGTK 605.1.15 linux #0-2]    ✓ journals Tauri's own refusal on the postMessage fallback, with the real key
[WebKitGTK 605.1.15 linux #0-2] 6 passing (755ms)
[WebKitGTK 605.1.15 linux #0-3] » specs/transport.e2e.mjs
[WebKitGTK 605.1.15 linux #0-3]    ✓ gives the app page a 64-hex per-launch token
[WebKitGTK 605.1.15 linux #0-3]    ✓ answers API calls with the token and refuses them without it
[WebKitGTK 605.1.15 linux #0-3]    ✓ reads the last bytes of a 100 MB file as partial content
[WebKitGTK 605.1.15 linux #0-3]    ✓ reads the whole 100 MB file through capped ranges
[WebKitGTK 605.1.15 linux #0-3]    ✓ pushes events to the app window on the event channel
[WebKitGTK 605.1.15 linux #0-3]    ✓ keeps the app window on the app's own origin
[WebKitGTK 605.1.15 linux #0-3] 6 passing (5s)
Spec Files:	 4 passed, 4 total (100% completed) in 00:00:17
```

UI contract test, `npx vitest run tests/unit/contract.test.ts` in `apps/filesquid/ui` (after `npm run contract:types`):

```text
 Test Files  1 passed (1)
      Tests  1 passed (1)
```
