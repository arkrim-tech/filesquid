# Task API-F1.2 Walkthrough: Routes and the event channel

Module task [API-F1.2](../plan.md) of global Task FS-1.1 ([implementation-plan.md](../../../../../../resources/implementation-plan.md)). Branch `feat/api-f1.2-routes-events`. Builds on [API-F1.1](./Task-API-F1.1-Walkthrough.md).

## API-S-003: Transport-agnostic router mounted unchanged by every host, with one event channel for job progress and change notifications; clients do not depend on receiving every event

> The router MUST be transport-agnostic and mounted unchanged by the desktop host (custom URI scheme) and `filesquid-server` (HTTP). It MUST provide one event channel for job progress and change notifications; the transport is the outcome recorded in ADR 030. Clients MUST NOT depend on receiving every event: a subscriber that misses events MUST receive an `events.resync` hint instead of a silent gap and rereads durable state. The channel MUST be bounded: each subscriber lags at most 256 events, each event encodes to at most 65,536 bytes, and at most 64 client subscriptions are open at once (the next is refused with a retryable `unavailable` error). Every topic and its payload MUST be documented in the contract ([ADR 030](../../../../../../resources/architecture/030-filesquid-api-and-transports.md) Decision 4).

Parents: [F-056 Single FileSquid API](../../../../../../resources/prd.md#f-056-single-filesquid-api), [SRS-F-056 Single FileSquid API](../../../../../../resources/srs.md#srs-f-056-single-filesquid-api), [F-057 Desktop and server hosts](../../../../../../resources/prd.md#f-057-desktop-and-server-hosts), FileSquid [U-17](../../../../../../resources/product-vision.md#one-api-hosts-and-distributions), [T-049, T-050](../../../../../../resources/product-vision.md#api-and-hosts), [ADR 030](../../../../../../resources/architecture/030-filesquid-api-and-transports.md) Decisions 3 and 4.

This task also keeps [API-S-002](../srs.md) (versioned contract with a drift test) and [API-S-009](../srs.md) (errors the UI can show) for everything it adds.

## What changed

- **Event channel** ([`src/events.rs`](../../src/events.rs), now a public module). It holds no transport code.
  - `topic` constants: `process.update`, `journal.entry`, `health.change` and `events.resync`, with `topic::ALL`. The registry, monitor and journal writer publish through these constants.
  - `EventBus::subscription()` opens a client subscription, one per WebSocket session or webview channel. It returns an `EventSubscription`, or a retryable `unavailable` `ApiError` when `MAX_SUBSCRIBERS` (64) are open. The slot is claimed with a single atomic check-and-increment and released when the subscription is dropped.
  - `EventSubscription::next()` turns a lag (`RecvError::Lagged(n)`) into an `events.resync` event with `{"missed": n}`, so a client is never left with a silent gap. It returns `None` once the bus is gone, and it is cancel-safe.
  - `EVENT_BUFFER` (256): how many events each subscriber may fall behind. A broadcast ring of fixed size, so a slow subscriber cannot grow memory and publishing never blocks.
  - `MAX_EVENT_BYTES` (65,536): `publish` counts an event's JSON encoding and stops at the cap. A larger event is replaced by `events.resync` with `missed` 1 and counted in `oversized_events()`. That path logs at DEBUG only, because the journal writer thread publishes `journal.entry`. A journaled WARN line from that thread could wait on its own full queue.
  - `EventBus::subscribe()`, `publish()` and `subscriber_count()` keep their signatures, so `filesquid-server` and `filesquid-desktop` build unchanged. `subscribe()` is now documented as an uncounted raw receiver for trusted in-process consumers.
- **Contract `0.4.0`** ([`openapi.json`](../../openapi.json)):
  - New component schemas `EventTopic` (the enum of the four topics) and `EventResync` (`missed`, integer of at least 1). `ApiEvent.topic` now refers to `EventTopic`.
  - `GET /api/v1/events` gets a description of the hint semantics and the limits, plus the extension `x-filesquid-event-payloads`. The extension maps each topic to its payload schema: `ProcessView`, `JournalEntry`, `HealthReport` or `EventResync`.
- **Generated types**: `apps/filesquid/ui/src/api/schema.d.ts`, regenerated with `npm run contract:types` (`openapi-typescript` 7.13.0). `ApiEvent.topic` is now the `EventTopic` union. The UI's own code and tests compile unchanged.
- **Routes**: the request/response routes the router serves are `GET /api/v1/health`, `GET /api/v1/openapi.json` and the admin routes from API-F2.1. This task adds no route. It pins with tests that the same router answers each of them, and the 404/405 fallbacks, the same way through every host's address form. Those forms are a path only (`filesquid-server`), `filesquid://localhost/...` (desktop on macOS and Linux) and `http://filesquid.localhost/...` (desktop on Windows). Change-notification topics for file operations come with their routes in API-F1.3 and API-F1.6. Job progress travels as `process.update`, because every job registers in the process registry (ADR 033).
- **Docs**: [ADR 030](../../../../../../resources/architecture/030-filesquid-api-and-transports.md) records the event channel semantics and limits under Decision 4 and what the hosts must do. [API-S-003](../srs.md) now states the limits and the resync rule. [SRS-F-056](../../../../../../resources/srs.md#srs-f-056-single-filesquid-api) lists the two API features, and its test gap is narrowed to deny-by-default authorisation. The root README "Run tests" section gains the event-channel command.

## Test to requirement mapping

| Requirement / constraint | Tests |
|---|---|
| API-S-003 router mounted unchanged by every host | `unit_routes`: `every_routed_path_answers_the_same_through_every_host_uri_form`, `fallbacks_answer_the_same_through_every_host_uri_form`, `the_health_route_answers_through_the_desktop_scheme`; BDD "The same router answers through the desktop scheme and over HTTP" |
| API-S-003 one event channel: documented topics | `event_topics_in_the_contract_match_the_rust_topics`, `the_documented_limits_have_their_declared_values`, `every_component_schema_matches_its_serialized_rust_type` (`EventTopic`, `EventResync`, `ApiEvent`) |
| API-S-002 per-topic payloads in the contract | `every_event_topic_documents_its_payload_schema`, `events_from_every_publisher_match_their_documented_payloads` (real registry, monitor tick, journal writer and lag events checked against `x-filesquid-event-payloads`; every topic observed) |
| API-S-003 clients do not depend on every event: buffer boundary (256) | `a_subscriber_exactly_one_buffer_behind_loses_nothing`, `one_event_past_the_buffer_becomes_a_resync_hint_then_the_newest_events`; BDD "A subscriber exactly one buffer behind misses nothing", "A subscriber that falls further behind is told to reread" |
| Slow client cannot block publishers or grow memory | `a_slow_subscriber_cannot_block_publishers_or_grow_memory` (10,000 events, one hint with `missed` 9,744, then the newest 256) |
| Event size boundary (65,536 bytes) | `an_event_at_the_size_cap_is_delivered_unchanged`, `an_event_one_byte_over_the_size_cap_becomes_a_resync_hint`; BDD "An event larger than the size cap is replaced by a hint" |
| Subscriber cap (64), retryable `ApiError` (API-S-009), no leak after disconnect | `the_subscriber_cap_refuses_one_more_with_a_retryable_error`, `dropping_a_subscription_frees_its_slot_and_its_receiver`, `concurrent_subscribers_never_exceed_the_cap`; BDD "The channel refuses a subscriber over the cap with a retryable error" |
| Lag and close handling | `a_closed_bus_ends_the_subscription`, `a_resync_hint_carries_only_the_missed_count`, `subscribers_receive_published_events`, `publishing_without_subscribers_is_not_an_error`; BDD "A subscriber receives a published process update" |
| API-S-002 contract and generated types in lockstep | Vitest `apps/filesquid/ui/tests/unit/contract.test.ts`; `contract_is_openapi_3_1_with_the_crate_api_version` (0.4.0) |
| Hosts unchanged | `cargo test --workspace --locked` (the `filesquid-server` WebSocket and `filesquid-desktop` custom-scheme integration tests still pass) |

[ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md) defines no Rust line-coverage tool, so no coverage percentage is reported. Every new public item (`topic`, `EventResync`, `ApiEvent::resync`, `EventBus::subscription`, `open_subscriptions`, `oversized_events`, `EventSubscription::next` and the three limits) is exercised by the tests above.

## Red: tests before the implementation

`cargo test -p filesquid-api --locked --test unit_events --test unit_contract --test features` (unique errors; the tests referred to the API that did not exist yet):

```text
error: could not compile `filesquid-api` (test "features") due to 9 previous errors
error: could not compile `filesquid-api` (test "unit_contract") due to 7 previous errors
error: could not compile `filesquid-api` (test "unit_events") due to 20 previous errors
error[E0425]: cannot find type `EventSubscription` in `filesquid_api::events`
error[E0432]: unresolved import `filesquid_api::events::EventSubscription`
error[E0432]: unresolved imports `filesquid_api::events::topic`, `filesquid_api::events::EventResync`
error[E0432]: unresolved imports `filesquid_api::events::topic`, `filesquid_api::events::EventResync`, `filesquid_api::events::EventSubscription`, `filesquid_api::events::MAX_EVENT_BYTES`, `filesquid_api::events::MAX_SUBSCRIBERS`
error[E0432]: unresolved imports `filesquid_api::events::topic`, `filesquid_api::events::EventResync`, `filesquid_api::events::MAX_EVENT_BYTES`, `filesquid_api::events::MAX_SUBSCRIBERS`
error[E0599]: no associated function or constant named `resync` found for struct `ApiEvent` in the current scope
error[E0599]: no method named `open_subscriptions` found for reference `&EventBus` in the current scope
error[E0599]: no method named `open_subscriptions` found for struct `EventBus` in the current scope
error[E0599]: no method named `oversized_events` found for struct `EventBus` in the current scope
error[E0599]: no method named `subscription` found for reference `&EventBus` in the current scope
error[E0599]: no method named `subscription` found for struct `EventBus` in the current scope
error[E0603]: module `events` is private
```

`unit_routes` passed at red. It pins behaviour the router already had: axum routes on the URI path only. It is kept as a regression guard for API-S-003:

```text
test the_health_route_answers_through_the_desktop_scheme ... ok
test fallbacks_answer_the_same_through_every_host_uri_form ... ok
test every_routed_path_answers_the_same_through_every_host_uri_form ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

`npx vitest run tests/unit/contract.test.ts` in `apps/filesquid/ui` after the contract changed, before regenerating the types:

```text
   × src/api/schema.d.ts matches crates/api/openapi.json 63ms
      Tests  1 failed (1)
```

Mutation check 1: `events.resync` was removed from the `EventTopic` enum in a copy of the contract, the tests were run, and the original was restored:

```text
test event_topics_in_the_contract_match_the_rust_topics ... FAILED
test every_component_schema_matches_its_serialized_rust_type ... FAILED
test events_from_every_publisher_match_their_documented_payloads ... FAILED
test result: FAILED. 20 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
```

Mutation check 2: the size cap was changed from `>` to `>=` (off by one), the tests were run, and the change was reverted:

```text
test an_event_at_the_size_cap_is_delivered_unchanged ... FAILED
test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
```

## Green: raw output

`cargo test -p filesquid-api --locked --test unit_events --test unit_routes --test unit_contract --test features` (final run):

```text
Feature: Transport-agnostic routes and one event channel
  Scenario: The same router answers through the desktop scheme and over HTTP
   ✔  Given a FileSquid API router
   ✔  When a client asks for the health report through each host's address form
   ✔  Then every address form should get the same answer
  Scenario: A subscriber receives a published process update
   ✔  Given an event channel with one subscriber
   ✔  When a process update is published
   ✔  Then the subscriber should receive a "process.update" event
  Scenario: A subscriber exactly one buffer behind misses nothing
   ✔  Given an event channel with one subscriber that is not reading
   ✔  When as many events as the buffer holds are published
   ✔  Then the subscriber should receive every event in order
  Scenario: A subscriber that falls further behind is told to reread
   ✔  Given an event channel with one subscriber that is not reading
   ✔  When one more event than the buffer holds is published
   ✔  Then the subscriber should first receive an "events.resync" hint for 1 missed event
   ✔  And then the newest events the buffer holds
  Scenario: An event larger than the size cap is replaced by a hint
   ✔  Given an event channel with one subscriber
   ✔  When an event one byte over the size cap is published
   ✔  Then the subscriber should first receive an "events.resync" hint for 1 missed event
  Scenario: The channel refuses a subscriber over the cap with a retryable error
   ✔  Given an event channel with as many subscribers as the cap allows
   ✔  When one more client subscribes
   ✔  Then the subscription should be refused with a retryable "unavailable" error
   ✔  When one subscriber disconnects
   ✔  Then a new client should be able to subscribe
[Summary]
5 features
76 scenarios (17 passed, 59 skipped)
121 steps (62 passed, 59 skipped)
running 23 tests
test error_code_all_lists_every_variant ... ok
test command_envelope_carries_idempotency_key_expected_revision_and_scope ... ok
test contract_is_openapi_3_1_with_the_crate_api_version ... ok
test error_codes_match_the_rust_enum_with_status_and_retryability ... ok
test errors_carry_code_and_retryability ... ok
test event_topics_in_the_contract_match_the_rust_topics ... ok
test every_component_schema_has_a_rust_sample ... ok
test every_component_schema_matches_its_serialized_rust_type ... ok
test every_documented_operation_has_a_unique_operation_id ... ok
test every_error_response_carries_the_api_error_schema ... ok
test every_event_topic_documents_its_payload_schema ... ok
test every_limit_parameter_is_the_shared_page_size ... ok
test health_schema_matches_the_core_payload_and_requires_every_field ... ok
test query_envelope_and_page_size_parameter_are_bounded_like_core ... ok
test host_routes_are_the_event_channel_only ... ok
test routed_and_documented_paths_agree_both_ways ... ok
test router_fallbacks_are_documented_as_wire_conventions ... ok
test router_serves_its_own_contract ... ok
test the_checker_accepts_only_truly_unconstrained_objects ... ok
test every_documented_operation_answers_a_documented_status_and_body ... ok
test the_contract_uses_only_checked_schema_keywords ... ok
test router_fallbacks_answer_as_documented ... ok
test events_from_every_publisher_match_their_documented_payloads ... ok
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
running 13 tests
test a_resync_hint_carries_only_the_missed_count ... ok
test a_closed_bus_ends_the_subscription ... ok
test a_subscriber_exactly_one_buffer_behind_loses_nothing ... ok
test concurrent_subscribers_never_exceed_the_cap ... ok
test dropping_a_subscription_frees_its_slot_and_its_receiver ... ok
test one_event_past_the_buffer_becomes_a_resync_hint_then_the_newest_events ... ok
test publishing_without_subscribers_is_not_an_error ... ok
test an_event_one_byte_over_the_size_cap_becomes_a_resync_hint ... ok
test an_event_at_the_size_cap_is_delivered_unchanged ... ok
test the_documented_limits_have_their_declared_values ... ok
test subscribers_receive_published_events ... ok
test the_subscriber_cap_refuses_one_more_with_a_retryable_error ... ok
test a_slow_subscriber_cannot_block_publishers_or_grow_memory ... ok
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
running 3 tests
test the_health_route_answers_through_the_desktop_scheme ... ok
test fallbacks_answer_the_same_through_every_host_uri_form ... ok
test every_routed_path_answers_the_same_through_every_host_uri_form ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

The 59 skipped scenarios belong to the existing F-061, F-063 and F-064 features. Their domain operations do not exist yet, so they have no step definitions. That was already the case before this task.

`cargo test -p filesquid-api --locked`: 89 tests in 12 unit binaries passed, and the cucumber harness ran 17 scenarios with 0 failures.

`npx vitest run tests/unit/contract.test.ts` in `apps/filesquid/ui`, after `npm run contract:types`:

```text
 ✓ tests/unit/contract.test.ts (1 test) 63ms
      Tests  1 passed (1)
```

Full gates:

- `cargo fmt --all` and `cargo clippy --workspace --all-targets --locked -- -D warnings`: clean.
- `cargo test --workspace --locked`: exit 0. This includes the `filesquid-server` and `filesquid-desktop` integration tests, which mount the router and forward the bus with no code change.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked`: clean.
- In `apps/filesquid/ui`: `npx prettier --check .`, `npm run lint`, `npm run check` and `npm test` (7 files, 37 tests passed).
- In `apps/imagesquid`: `npm run check` and `npm test` (2 files, 5 tests passed).

## Host work left to DE-F1.1 and SV-F1.1

The router side is transport-agnostic and complete. The hosts still forward from the uncounted `EventBus::subscribe()` receiver, so until they change, their clients get neither the subscriber cap nor the resync hint. ADR 030 Decision 4 records what each host must do:

- **`filesquid-server` (SV-F1.1).** Call `state.events.subscription()` before upgrading `GET /api/v1/events`. When it returns an `ApiError`, answer with that error (503, retryable) instead of upgrading. Forward `EventSubscription::next()` to the socket until it returns `None` or the client leaves, `events.resync` included. Add the 503 response to the contract operation in the same change.
- **`filesquid-desktop` (DE-F1.1).** Forward from one `subscription()`, `events.resync` included, so the UI rereads after a gap.

## Mocked Environment Variables

None.
