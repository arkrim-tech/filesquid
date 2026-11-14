# Task API-F2.27 Walkthrough: Self-scoped journal route

Task row: [API-F2.27](../plan.md). Global task: Task FS-2.4 ([implementation plan](../../../../../../resources/implementation-plan.md)). Decisions: owner decision 2026-10-31, verbatim: "accept recommendations" (recommendation 3: the self-scoped journal route moves to Task FS-2.4); [ADR 033](../../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md).

## Summary of changes

- **Route.** `GET /api/v1/journal` (`src/journal/own.rs`) answers one page of the journal entries whose actor is the caller. It reads the same parameters as `GET /api/v1/admin/journal` and answers the same `JournalViewPage` through one shared page reader (`answer_page` in `src/admin/journal.rs`), so the filters, the cursor paging (`after`, `limit`, `order`, `next_cursor`), the redaction, the `provenance` link and `names=true` are the admin route's.
- **Action.** `Action::ReadOwnJournal` (`journal.read_own`). `Action::granted_to_every_credential` now names it beside `host.read`, so every authenticated caller may read their own entries whatever the policy says. Authentication is still required: no credential answers 401 `unauthorized`, a credential the host does not accept answers 403 `forbidden`.
- **The actor is always the caller.** The store query's actor is set to `Principal::actor` of the caller. An `actor` parameter answers 400 `invalid_request` ("Invalid query parameter: actor."), even when it names the caller, so a caller cannot ask for anyone else's entries and is never answered a page that silently ignores the filter.
- **No admin route.** The admin routes keep `monitor.read`. A caller the policy does not grant it is still refused there (`forbidden`, `not_permitted`), and the refusal is journaled as `api.auth.denied`.
- **Names.** Display names are resolved at read time through the host's `JournalNames` lookup for the caller, so a file the caller may not read carries no name. Names are never stored.
- **Nothing new is journaled.** A successful read writes no journal entry (API-S-017), so the route adds no journal event type.
- **Contract.** `openapi.json` 0.38.0 documents the path (`getOwnJournal`, `x-filesquid-action: journal.read_own`, the admin route's parameters without `actor`). `ui/src/api/schema.d.ts` and `ui/src/api/contract-version.ts` are regenerated with `npm run contract:types`; nothing else under `apps/filesquid/ui` changed.
- **Docs.** The root README "View logs" section shows the route and says what it answers.

A caller who is not an administrator: server accounts arrive with API-F1.9, so the tests build that caller from the authoriser's other principal. "ana" is the owner principal under a policy that grants that principal nothing, and "ben" is the administrator (`tests/support/own_journal_fixture.rs`). The journal names each by its principal's actor.

## SRS mapping

## API-S-031: Journal query and export (Self-scoped journal)

| Constraint | Test |
|---|---|
| Route `GET /api/v1/journal`, action `journal.read_own` | `the_route_performs_journal_read_own`; `unit_auth` `every_routed_path_declares_its_action`, `action_names_are_unique_dotted_identifiers`; `unit_contract` `routed_and_documented_paths_agree_both_ways`, `every_operation_names_the_action_the_router_authorises` |
| Granted to every accepted credential | `journal_read_own_is_granted_to_every_authenticated_principal_whatever_the_policy`, `a_caller_reads_only_their_own_entries` (the policy grants "ana" nothing) |
| Only the caller's entries | `a_caller_reads_only_their_own_entries`, `the_query_is_always_limited_to_the_caller`; scenario "A caller reads only their own entries" |
| Filters of the admin route; `actor` refused | `the_admin_filters_apply_within_the_callers_entries`, `an_actor_filter_is_refused`, `an_actor_parameter_is_refused_even_when_it_names_the_caller`, `invalid_filter_values_answer_invalid_request_as_on_the_admin_route`; scenario "The caller cannot choose whose entries to read" |
| Cursor paging | `cursor_paging_walks_the_callers_entries_without_repeat_or_gap`; scenario "The admin journal filters and paging apply" |
| Redaction | `the_answer_keeps_the_journals_redaction`; scenario "The self-scoped answer keeps the journal's redaction" |
| Names only for files the caller may read | `names_are_resolved_on_request_and_only_for_files_the_caller_may_read`; scenario "Names are resolved only for files the caller may read" |
| Grants none of the admin routes | `the_route_grants_no_admin_route` (every `monitor.read` route); scenario "The self-scoped route grants no admin route" |
| On the desktop the caller is the owner | `the_desktop_owner_reads_the_owners_entries_and_none_of_the_hosts`; scenario "The desktop owner reads the owner's entries" |

## API-S-004: Deny-by-default authorisation

`the_route_needs_an_accepted_credential`; scenario "A call without an accepted credential is refused"; the route is in `unit_auth`'s every-route denial tests through the route table.

## API-S-017: Journaled refusals, no entry for a successful call

`a_successful_read_journals_nothing`; `the_route_grants_no_admin_route` reads the `api.auth.denied` entries of the refused admin calls.

## Redaction

The route adds no journal event type, so there is no new entry to redact. The redaction test is on the answer: an entry written with a title, a path and a credential in its fields is answered with `redacted` in their place, carries only whitelisted members and opaque targets, and equals what the admin route answers for the same actor (`the_answer_keeps_the_journals_redaction`).

## Coverage

The repository configures no line-coverage tool for the Rust crates; coverage is shown by the constraint table above, in which every constraint of the row has a unit test and a scenario.

## Mocked Environment Variables

None. The README command was checked against a local `filesquid-server` started with a throwaway `FILESQUID_ADMIN_TOKEN` (64 hexadecimal characters) and a data directory under `TMPDIR`; nothing from that run is committed.

## Test output

### Failing first

`cargo test -p filesquid-api --test unit_journal_own --locked` before the route existed (the action variant alone was added so the tests compile; the two passing tests cover behaviour the authorisation layer already had):

```text
running 14 tests
test cursor_paging_walks_the_callers_entries_without_repeat_or_gap ... FAILED
test a_successful_read_journals_nothing ... FAILED
test a_caller_reads_only_their_own_entries ... FAILED
test journal_read_own_is_granted_to_every_authenticated_principal_whatever_the_policy ... FAILED
test an_actor_filter_is_refused ... FAILED
test the_admin_filters_apply_within_the_callers_entries ... FAILED
test the_answer_keeps_the_journals_redaction ... FAILED
test invalid_filter_values_answer_invalid_request_as_on_the_admin_route ... FAILED
test names_are_resolved_on_request_and_only_for_files_the_caller_may_read ... FAILED
test the_route_performs_journal_read_own ... FAILED
test the_desktop_owner_reads_the_owners_entries_and_none_of_the_hosts ... FAILED
test the_route_reports_unavailable_before_the_monitor_starts ... FAILED
test the_route_needs_an_accepted_credential ... ok
test the_route_grants_no_admin_route ... ok
test result: FAILED. 2 passed; 12 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
error: test failed, to rerun pass `-p filesquid-api --test unit_journal_own`
```

### filesquid-api unit tests

`cargo test -p filesquid-api --locked --lib --test unit_journal_own --test unit_auth --test unit_contract`:

```text
     Running unittests src/lib.rs
test result: ok. 81 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s
     Running tests/unit/auth.rs
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.58s
     Running tests/unit/contract.rs
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.73s
     Running tests/unit/journal_own.rs
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
```

The route's own tests in the library and the `unit_journal_own` binary:

```text
test journal::own::tests::the_query_is_always_limited_to_the_caller ... ok
test journal::own::tests::an_invalid_filter_names_its_parameter ... ok
test journal::own::tests::an_actor_parameter_is_refused_even_when_it_names_the_caller ... ok
     Running tests/unit/journal_own.rs 

running 14 tests
test a_successful_read_journals_nothing ... ok
test cursor_paging_walks_the_callers_entries_without_repeat_or_gap ... ok
test a_caller_reads_only_their_own_entries ... ok
test an_actor_filter_is_refused ... ok
test journal_read_own_is_granted_to_every_authenticated_principal_whatever_the_policy ... ok
test names_are_resolved_on_request_and_only_for_files_the_caller_may_read ... ok
test the_answer_keeps_the_journals_redaction ... ok
test the_desktop_owner_reads_the_owners_entries_and_none_of_the_hosts ... ok
test the_route_needs_an_accepted_credential ... ok
test the_route_performs_journal_read_own ... ok
test the_route_reports_unavailable_before_the_monitor_starts ... ok
test the_admin_filters_apply_within_the_callers_entries ... ok
test invalid_filter_values_answer_invalid_request_as_on_the_admin_route ... ok
test the_route_grants_no_admin_route ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
```

### filesquid-api BDD (`self_scoped_journal_API-S-031.feature`)

`cargo test -p filesquid-api --locked --test features`:

```text
Feature: Self-scoped journal route
  Scenario: A caller reads only their own entries
   ✔> Given own-journal entries by user "ana" and by user "ben"
   ✔  When own-journal caller "ana" reads the self-scoped journal
   ✔  Then own-journal answer should hold only entries whose actor is "ana"
  Scenario: The desktop owner reads the owner's entries
   ✔> Given own-journal entries by user "ana" and by user "ben"
   ✔  Given own-journal entries by the desktop owner and by the host itself
   ✔  When own-journal desktop owner reads the self-scoped journal
   ✔  Then own-journal answer should hold the owner's entries and no entry of the host itself
  Scenario: The admin journal filters and paging apply
   ✔> Given own-journal entries by user "ana" and by user "ben"
   ✔  When own-journal caller "ana" reads the self-scoped journal filtered by operation "file.upload" one page of 2 at a time
   ✔  Then own-journal pages should hold only "ana"'s file.upload entries, with no repeat or gap
  Scenario: The caller cannot choose whose entries to read
   ✔> Given own-journal entries by user "ana" and by user "ben"
   ✔  When own-journal caller "ana" reads the self-scoped journal filtered by actor "ben"
   ✔  Then own-journal answer should be refused as an invalid request naming the actor parameter
  Scenario: A call without an accepted credential is refused
   ✔> Given own-journal entries by user "ana" and by user "ben"
   ✔  When own-journal is read without a credential and with a credential the host does not accept
   ✔  Then own-journal answers should be unauthorized and forbidden, with no entry
  Scenario: The self-scoped route grants no admin route
   ✔> Given own-journal entries by user "ana" and by user "ben"
   ✔  When own-journal caller "ana" reads the admin journal
   ✔  Then own-journal answer should be refused as forbidden and journaled as api.auth.denied
  Scenario: The self-scoped answer keeps the journal's redaction
   ✔> Given own-journal entries by user "ana" and by user "ben"
   ✔  Given own-journal entry of "ana" about an upload of a file with a title and a path
   ✔  When own-journal caller "ana" reads the self-scoped journal
   ✔  Then own-journal entry should carry opaque IDs only, with no title, path, content or credential, exactly as the admin journal route would answer it
  Scenario: Names are resolved only for files the caller may read
   ✔> Given own-journal entries by user "ana" and by user "ben"
   ✔  Given own-journal entry of "ana" about a file "ana" may no longer read
   ✔  When own-journal caller "ana" reads the self-scoped journal with names
   ✔  Then own-journal entry should carry no name for that file
573 scenarios (379 passed, 194 skipped)
1990 steps (1796 passed, 194 skipped)
```

The skipped scenarios are those of other features whose steps later tasks add.

### README command against a running server

```text
$ curl -H "Authorization: Bearer $TOKEN" "http://127.0.0.1:8083/api/v1/journal?order=desc&limit=20"
{"entries":[{"id":6,"time":"2026-11-11T15:19:23.906Z","host":"filesquid-server","actor":"administrator","operation":"journal.export","outcome":"succeeded","level":"info","target_ids":["format:jsonl","entries:1"],"provenance":{"journal_only":true}}],"next_cursor":null}
$ curl -H "Authorization: Bearer $TOKEN" "http://127.0.0.1:8083/api/v1/journal?actor=system"
{"code":"invalid_request","message":"Invalid query parameter: actor.","retryable":false}
```

## Hook run

`FULL=1 bash scripts/pre-commit.sh` passed on the head commit; its `Test scope:` line is in the pull request.

## Sync Execution Report

No upstream requirement changed: API-S-031, SRS-F-061 and ADR 033 already state the route. Updated in this change: the crate `plan.md` row; API-F2.27 was the last open row of global Task FS-2.4, so the Task FS-2.4 box in `resources/implementation-plan.md` and its row and module box in `resources/implementation-plan-checklist.md` are ticked, with the MVP progress redrawn; the root `README.md` "View logs" section, the contract and the generated UI types.

## Routes the UI can now call

`GET /api/v1/journal` (`journal.read_own`): `from`, `to`, `operation`, `outcome`, `level`, `process`, `library`, `object` or `file`, `operation_id`, `after`, `limit`, `order`, `names`. It answers `JournalViewPage`.
