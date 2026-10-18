# Task API-F1.10 Walkthrough: Host descriptor route and change event

Module task [API-F1.10](../plan.md) of global [Task FS-2.1](../../../../../../resources/implementation-plan.md). Branch `feat/api-f1.10-host-descriptor`, from `main` at `ab52290` (PR #35). Dependencies API-F1.2 and API-F1.7 were done.

**Status: done.** `GET /api/v1/host` and the `host.change` event are in the router and the contract, both hosts describe themselves, and `filesquid-server` reads `FILESQUID_OPERATOR_NAME`.

## API-S-020: Host descriptor: host type, operator name and hosted backup and sync state, with a change event

Parents: [F-057 Desktop and server hosts](../../../../../../resources/prd.md#f-057-desktop-and-server-hosts), [SRS-F-057 Desktop and server hosts](../../../../../../resources/srs.md#srs-f-057-desktop-and-server-hosts), [F-059 Hosted encrypted backup and sync service](../../../../../../resources/prd.md#f-059-hosted-encrypted-backup-and-sync-service), [SRS-F-059 Hosted encrypted backup and sync service](../../../../../../resources/srs.md#srs-f-059-hosted-encrypted-backup-and-sync-service), [F-061 Activity journal and live health monitoring](../../../../../../resources/prd.md#f-061-activity-journal-and-live-health-monitoring), [ADR 032](../../../../../../resources/architecture/032-hosting-and-privacy-model.md) Decisions 4, 6 and 7. Gherkin: [`host_descriptor_API-S-020.feature`](../../tests/features/host_descriptor_API-S-020.feature).

| Constraint | Gate | State |
|---|---|---|
| Route answers every authenticated caller | `the_desktop_describes_itself_*`, `a_server_describes_itself_*`, desktop `the_host_descriptor_is_served_through_the_custom_scheme`, server `a_named_server_describes_itself_*` | Met |
| `host.read` granted to every accepted credential | `Action::granted_to_every_credential`; `every_accepted_credential_reads_it_even_one_granted_nothing_else`, `host_read_is_granted_to_every_authenticated_principal_whatever_the_policy` | Met |
| Not public; missing credential refused and journaled | `a_missing_credential_is_refused`, scenario "A caller without a credential is refused" | Met |
| Exactly four fields, `schema_version` 1; `hosted_service` exactly four fields | `the_descriptor_has_exactly_its_documented_fields`, contract `additionalProperties: false`, drift test | Met |
| `host_type` `desktop`, `server` or `browser` | `HostType`; contract enum checked against every variant | Met |
| Host type supplied by the host, never from a request | `ApiState::describe_host`; `the_host_type_never_follows_a_request_header_or_address` (origin, host, user agent, forwarded headers, absolute and custom-scheme URIs) | Met |
| `operator_name` null on desktop and browser | `HostIdentity::desktop()` and `browser()` cannot carry one; `only_a_server_identity_can_carry_an_operator` | Met |
| Operator trimmed, 1 to 256 characters, no control characters | `OperatorName::parse`; `an_operator_name_*` tests, server `the_operator_name_is_read_trimmed_from_the_environment` | Met |
| Unset or blank: starts, null, one `server.operator_unnamed` warning to stdout and the journal at every start, naming the variable | The line is written straight to stdout, outside the `RUST_LOG` filter, and the entry is queued on the journal directly. Gates: server harness outline (3 examples, with the log filter off), `an_unnamed_server_starts_with_one_warning_*` (log filter off, journal layer attached, still one entry), `the_binary_prints_the_warning_even_with_logging_off`, `the_warning_is_written_at_every_start`, `without_a_journal_layer_*` | Met |
| Too long or control character: refuses to start, names the variable, omits the value | server harness outline against the real binary (the 257-character value is exactly 257 characters), `an_unusable_operator_name_refuses_*`; a value that is not valid Unicode, in this or any other variable the server reads, is refused the same way instead of reading as unset (`a_non_unicode_variable_is_refused_without_echoing_it`, `the_binary_refuses_a_non_unicode_operator_name`) | Met |
| Hosted service from an injected enrolment port; no enrolment state reports not enrolled | `EnrolmentPort`, `NotEnrolled`; `the_default_port_reports_exactly_not_enrolled` | Met (API-F4.1 brings the real port) |
| Enrolled: lowercase bare DNS host name; backup and sync separate | `HostedService::enrolled_at`; `the_service_host_is_a_bare_dns_host_name`, `the_service_host_is_stored_lowercase`, `an_enrolled_service_reports_backup_and_sync_separately` | Met |
| Privacy of answer and event | `the_answer_and_the_event_carry_nothing_private`, scenario "The descriptor and its event carry nothing private" | Met |
| One `host.change` per change, whole descriptor, every subscriber, in order | `HostInfo::refresh_hosted_service`; `each_change_sends_one_host_change_*`, `a_refresh_without_a_change_sends_nothing`, scenario "A descriptor change reaches every subscriber" | Met. Trigger: whoever changes the enrolment state calls `refresh_hosted_service` right after the change (push, not poll). Host type and operator name never change in a running process, and both hosts use `NotEnrolled`, which never changes, so no host calls it yet; API-F4.1 calls it after every enrol, unenrol, backup switch and sync switch (stated in its plan row) |
| Reread after `events.resync` or reconnect | Resync: scenario "A client that missed events reads the descriptor again". Reconnect: `a_client_that_reconnects_reads_the_current_descriptor_from_the_route` shows that a new subscription replays nothing and the route answers the current state. The client's duty to reread is a contract statement (route description), checked by both tests; client behaviour itself is UI-F2.1b's to test | Met for the API side |
| Contract holds route, action, schemas and topic | `openapi.json` 0.6.0; `unit_contract` drift tests including a real `host.change` publisher; `schema.d.ts` regenerated | Met |
| Reading is not journaled | `reading_the_descriptor_is_not_journaled` | Met |
| Redaction of answer, event payload and `server.operator_unnamed` entry | the two privacy tests above, `the_operator_unnamed_entry_carries_nothing_private`, server scenario redaction step | Met |

### Secondary SRS mapping

- **API-S-003: One bounded event channel; a client rereads state after a resync hint.** `host.change` is a documented topic (`topic::ALL`, contract `EventTopic` and payload map); the resync scenario covers rereading.
- **API-S-004: Deny-by-default authorisation.** The route is guarded like every other; `unit_auth` covers it in its every-route loops.
- **API-S-015: The liveness probe is the only public route.** Unchanged; `host.read` still requires authentication.
- **API-S-017: Credential issues and refusals are journaled, successful calls are not.** Refusals of the route are journaled; reads are not.

## What changed

- `crates/api/src/host.rs` (new): `HostType`, `OperatorName`, `HostedService`, `HostIdentity`, `HostDescriptor`, `EnrolmentPort`, `NotEnrolled`, `HostInfo`.
- `crates/api`: `ApiState::describe_host` and `host()`, `Action::ReadHost` (`host.read`), `topic::HOST_CHANGE`, route `/host` (503 until described), contract 0.6.0, schema checker `maxLength`.
- `crates/server`: `ServerConfig.operator_name`, `OPERATOR_NAME_VARIABLE`, `OPERATOR_UNNAMED_OPERATION`, `operator_unnamed_entry`, `operator_unnamed_line`, `describe_server_host` called from `start_server` (and `start_server_writing_to`, which tests use to capture the unfiltered stdout line); new cucumber harness `tests/features.rs` for the `@server-start` scenarios; `integration_host`.
- `crates/desktop`: `describe_desktop_host` called from `builder()`.
- `ui/src/api/schema.d.ts` regenerated; no other UI change.
- Root `README.md` (server configuration, test commands) and `docker-compose.yml` (`FILESQUID_OPERATOR_NAME`).

Commands verified: the server started with and without `FILESQUID_OPERATOR_NAME` answered `GET /api/v1/host` as documented, logged one `server.operator_unnamed` line when unnamed, and refused a name with a tab; `docker compose config` resolves the variable set and unset.

## Mocked Environment Variables

None. Tests use generated or fixed test tokens in temporary data directories.

## Cubic round 1

Fixed: the unnamed-operator warning now reaches stdout whatever `RUST_LOG` says (with a test against the real binary); the refresh trigger is named in `host.rs`, this walkthrough and the API-F4.1 plan row; the README no longer says the UI shows the operator name; the forbidden-field lists in `host.rs` and the test doc comments match API-S-020; the `auth` module doc names the `host.read` exception; the stand-in member credential is explained where it is built; the duplicate binary-refusal test was removed (the feature harness keeps it); the Plan and ToDos were deleted.

## Cubic round 2

Fixed: the 257-character refusal value is now exactly 257 characters; `assert_plain_text` rejects any digit; every awaited event in the host steps is bounded by a 5-second timeout; a non-Unicode `FILESQUID_OPERATOR_NAME` is refused (`ServerConfig::from_env`, `require_unicode_operator_name`) instead of reading as unset; the README says "authenticated caller" and "non-blank, valid"; reconnect rereading has its own test.

## Cubic round 3

Fixed: `ServerConfig::from_env` reads every server variable (`ENV_VARIABLES`) through one helper, `unicode_value`, so a non-Unicode value in any of them is refused with a message that names the variable and not the value; the non-Unicode binary test sets `RUST_LOG=info` explicitly.

## Test output

Output from the first commit; the full hook was rerun on the final head.

```text
     Running tests/unit/host.rs (/home/user/.cargo-target-shared/debug/deps/unit_host-50d7895c602d641e)
test a_blank_operator_name_is_no_operator_name ... ok
test a_host_describes_itself_once_and_the_first_description_stays ... ok
test a_service_host_error_never_contains_the_value ... ok
test a_wrong_credential_is_refused ... ok
test an_enrolled_service_reports_backup_and_sync_separately ... ok
test an_operator_name_error_never_contains_the_value ... ok
test an_operator_name_holds_1_to_256_characters ... ok
test an_operator_name_is_trimmed ... ok
test an_operator_name_with_a_control_character_is_refused ... ok
test an_unnamed_server_reports_a_null_operator ... ok
test before_a_host_describes_itself_the_route_is_unavailable ... ok
test a_missing_credential_is_refused ... ok
test a_server_describes_itself_with_its_operator ... ok
test every_accepted_credential_reads_it_even_one_granted_nothing_else ... ok
test host_read_is_granted_to_every_authenticated_principal_whatever_the_policy ... ok
test only_a_server_identity_can_carry_an_operator ... ok
test host_change_is_a_documented_topic ... ok
test reading_the_descriptor_is_not_journaled ... ok
test the_browser_host_describes_itself_with_no_operator ... ok
test the_default_port_reports_exactly_not_enrolled ... ok
test the_descriptor_has_exactly_its_documented_fields ... ok
test a_refresh_without_a_change_sends_nothing ... ok
test the_desktop_describes_itself_with_no_operator_and_not_enrolled ... ok
test the_desktop_owner_reads_it_with_the_per_launch_token ... ok
test the_route_is_host_read ... ok
test the_service_host_is_a_bare_dns_host_name ... ok
test the_service_host_is_stored_lowercase ... ok
test the_host_type_never_follows_a_request_header_or_address ... ok
test the_answer_and_the_event_carry_nothing_private ... ok
test each_change_sends_one_host_change_with_the_whole_descriptor_to_every_subscriber ... ok
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
Feature: Host descriptor
  Scenario: The desktop host describes itself to the owner's webview
   ✔  Given a FileSquid desktop host that describes itself to the API
   ✔  When the owner's webview reads the host descriptor with the per-launch token
   ✔  Then the descriptor should say host type "desktop" with no operator name
   ✔  And the hosted service should be reported as not enrolled
  Scenario: A server describes itself with its operator's name to any signed-in caller
   ✔  Given a FileSquid server host whose operator name is set to "Garderen Family"
   ✔  When a signed-in member who is not an administrator reads the host descriptor
   ✔  Then the descriptor should say host type "server" with operator name "Garderen Family"
  Scenario: Backup and sync are reported separately once enrolled
   ✔  Given a FileSquid desktop host enrolled with the hosted service at "backup.filesquid.tech" with backup on and sync off
   ✔  When the owner's webview reads the host descriptor with the per-launch token
   ✔  Then the hosted service should be enrolled at "backup.filesquid.tech"
   ✔  And backup should be reported on and sync off, as two separate values
  Scenario: The host type never follows the request
   ✔  Given a FileSquid desktop host that describes itself to the API
   ✔  When a caller reads the host descriptor with headers that claim a server origin and a browser user agent
   ✔  Then the descriptor should still say host type "desktop"
  Scenario: The operator name is trimmed
   ✔  Given a FileSquid server host whose operator name is set to "  Garderen Family  "
   ✔  When a signed-in member who is not an administrator reads the host descriptor
   ✔  Then the descriptor should say host type "server" with operator name "Garderen Family"
  Scenario: A descriptor change reaches every subscriber
   ✔  Given a FileSquid desktop host that describes itself to the API
   ✔  And an owner's webview and a second authenticated client subscribed to the event channel
   ✔  When the host enrols with the hosted service
   ✔  And the host then turns sync on
   ✔  Then each subscriber should receive two "host.change" events in that order
   ✔  And the first should carry the enrolled descriptor with sync off and the second the descriptor with sync on
  Scenario: A client that missed events reads the descriptor again
   ✔  Given a subscriber that received an "events.resync" hint
   ✔  When it recovers its state
   ✔  Then it should read the host descriptor through the request route
  Scenario: A caller without a credential is refused
   ✔  Given a FileSquid desktop host that describes itself to the API
   ✔  When a client reads the host descriptor without a credential
   ✔  Then it should be refused as "unauthorized" because of "missing_credentials"
   ✔  And the activity journal should record one "api.auth.denied" entry
  Scenario: The descriptor and its event carry nothing private
   ✔  Given a FileSquid desktop host enrolled with the hosted service at "backup.filesquid.tech" with backup on and sync on
   ✔  When the descriptor and a "host.change" payload are inspected
   ✔  Then neither should contain a token, content handle, key or key identifier, account or device identifier, e-mail address, quota, object count, size, timestamp, path or file name
[Summary]
10 features
180 scenarios (74 passed, 106 skipped)
352 steps (246 passed, 106 skipped)
     Running tests/unit/auth.rs (/home/user/.cargo-target-shared/debug/deps/unit_auth-d3125f8fd2c3ca0b)
test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/unit/contract.rs (/home/user/.cargo-target-shared/debug/deps/unit_contract-1bbcccbcc9fc857d)
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
     Running tests/unit/events.rs (/home/user/.cargo-target-shared/debug/deps/unit_events-e78d30992c89d26a)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running tests/features.rs (/home/user/.cargo-target-shared/debug/deps/features-982c3befd7818a7f)
  Scenario Outline: A server without a usable operator name starts and says so
   ✔  Given a FileSquid server host started with FILESQUID_OPERATOR_NAME unset
   ✔  When a caller signed in to the server reads the host descriptor
   ✔  Then the descriptor should say host type "server" with no operator name
   ✔  And stdout should carry one "server.operator_unnamed" warning that names the variable
   ✔  And the activity journal should hold one "server.operator_unnamed" warning that names the variable
   ✔  And neither warning should contain a token, key, account or device identifier, path or file name
  Scenario Outline: A server without a usable operator name starts and says so
   ✔  Given a FileSquid server host started with FILESQUID_OPERATOR_NAME set to an empty value
   ✔  When a caller signed in to the server reads the host descriptor
   ✔  Then the descriptor should say host type "server" with no operator name
   ✔  And stdout should carry one "server.operator_unnamed" warning that names the variable
   ✔  And the activity journal should hold one "server.operator_unnamed" warning that names the variable
   ✔  And neither warning should contain a token, key, account or device identifier, path or file name
  Scenario Outline: A server without a usable operator name starts and says so
   ✔  Given a FileSquid server host started with FILESQUID_OPERATOR_NAME set to only spaces and tabs
   ✔  When a caller signed in to the server reads the host descriptor
   ✔  Then the descriptor should say host type "server" with no operator name
   ✔  And stdout should carry one "server.operator_unnamed" warning that names the variable
   ✔  And the activity journal should hold one "server.operator_unnamed" warning that names the variable
   ✔  And neither warning should contain a token, key, account or device identifier, path or file name
  Scenario Outline: An unusable operator name stops the server
   ✔  Given FILESQUID_OPERATOR_NAME is set to a name of 257 characters
   ✔  When filesquid-server starts
   ✔  Then it should refuse to start with an error that names FILESQUID_OPERATOR_NAME
   ✔  And the error should not contain the value
  Scenario Outline: An unusable operator name stops the server
   ✔  Given FILESQUID_OPERATOR_NAME is set to a name with a control character
   ✔  When filesquid-server starts
   ✔  Then it should refuse to start with an error that names FILESQUID_OPERATOR_NAME
   ✔  And the error should not contain the value
[Summary]
5 scenarios (5 passed)
26 steps (26 passed)
     Running tests/integration/host.rs (/home/user/.cargo-target-shared/debug/deps/integration_host-3deba5a4224193a7)
test an_unusable_operator_name_refuses_to_start_without_echoing_the_value ... ok
test the_operator_name_is_read_trimmed_from_the_environment ... ok
test the_operator_unnamed_entry_carries_nothing_private ... ok
test a_named_server_describes_itself_and_writes_no_warning ... ok
test an_unnamed_server_starts_with_one_warning_on_stdout_and_in_the_journal ... ok
test the_binary_refuses_an_unusable_operator_name_without_printing_it ... ok
test without_a_journal_layer_the_warning_is_journaled_directly_once ... ok
test the_warning_is_written_at_every_start ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/integration/api_protocol.rs (/home/user/.cargo-target-shared/debug/deps/integration_api_protocol-a424a860cb4e1001)
test the_host_descriptor_is_served_through_the_custom_scheme ... ok
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

`bash scripts/pre-commit.sh` passed on the first commit (`✅ All Pre-Commit Hooks Passed!`).
