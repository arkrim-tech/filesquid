# Task API-F1.1 Walkthrough: Versioned contract and generated TypeScript types

Module task [API-F1.1](../plan.md) of global Task FS-1.1 ([implementation-plan.md](../../../../../../resources/implementation-plan.md)). Branch `feat/api-f1.1-contract`.

## API-S-002: Versioned contract owned by `filesquid-api`, generated TypeScript types and a drift test

> The API contract MUST be versioned and owned by this crate (OpenAPI 3.1 in the repository). TypeScript types MUST be generated from it, and a test MUST fail when the contract and the generated types drift. Operations carry operation/idempotency key, expected revision, bounded page size, error code and retryability.

Parents: [F-056 Single FileSquid API](../../../../../../resources/prd.md#f-056-single-filesquid-api), [SRS-F-056 Single FileSquid API](../../../../../../resources/srs.md#srs-f-056-single-filesquid-api), [T-048](../../../../../../resources/product-vision.md#api-and-hosts), [CORE-S-005](../../../core/resources/srs.md), [ADR 030](../../../../../../resources/architecture/030-filesquid-api-and-transports.md) Decision 2.

## API-S-009: Module and API boundaries return errors the UI can show; a failed request does not crash a host

> Module and API boundaries MUST return errors the UI can show; a failed request MUST NOT crash a host.

Parents: [F-056 Single FileSquid API](../../../../../../resources/prd.md#f-056-single-filesquid-api), [F-037 Setup, updates and diagnostics](../../../../../../resources/prd.md#f-037-setup-updates-and-diagnostics), [ADR 030](../../../../../../resources/architecture/030-filesquid-api-and-transports.md) Decision 6.

## What changed

- **Contract `0.3.0`** ([`openapi.json`](../../openapi.json), `API_VERSION`):
  - New component schemas `CommandEnvelope` (schema version const 1, `operation_id` idempotency key, `expected_revision`, `scope`, each non-empty), `QueryEnvelope` (`page_size` 1–100), `CommandOutcome`, `CommandOutcomeKind`, `PortError` (`code`, `retryable`), `ApiError` (`code`, `message`, `retryable`) and `ErrorCode`.
  - Reusable parameters `PageSize` (`limit`, 1–100) and `Cursor`, and reusable `ApiError` responses. Every documented non-2xx response now carries `ApiError`.
  - The host-served event channel is marked `x-filesquid-host-route: true`.
  - `info.description` states the wire convention: commands send `envelope`, queries take `limit`/`cursor`, failures return `ApiError`.
- **Typed errors** ([`src/error.rs`](../../src/error.rs)): `ErrorCode` maps to one HTTP status each (400/401/403/404/409/500/503). Only `unavailable` is retryable. `ApiError::internal` logs the detail through `tracing::error!`, so it reaches stdout and the journal, and returns a generic message with no paths or SQL (API-S-005). The admin routes now answer every failure with `ApiError` JSON, including malformed query strings (the axum `QueryRejection` becomes `invalid_request`), and they check authorisation before validating parameters.
- **Operation schemas** ([`src/operation.rs`](../../src/operation.rs)): re-exports the `filesquid-core` envelopes and documents the wire convention.
- **Route table** ([`src/routes.rs`](../../src/routes.rs)): one table builds the router. `routed_paths()` lists exactly the paths the router serves.
- **Generated types**: `apps/filesquid/ui/src/api/schema.d.ts`, regenerated with `npm run contract:types` (`openapi-typescript` 7.13.0).
- **Generator decision**: `npm info openapi-typescript` shows `latest` = `7.13.0`, which is already pinned. `cargo search utoipa` shows `6.0.0`, but it was **not adopted**. The contract stays hand-authored and contract-first, and Rust is checked against it by the drift test, so no Rust dependency was added.

## Test → requirement mapping

| Requirement | Tests |
|---|---|
| API-S-002 versioned contract | `contract_is_openapi_3_1_with_the_crate_api_version`, `router_serves_its_own_contract`, BDD "The API serves its own versioned contract" |
| API-S-002 / ADR 030 drift: Rust routes ↔ contract | `routed_and_documented_paths_agree_both_ways`, `host_routes_are_the_event_channel_only`, `every_documented_operation_is_answered_by_the_router`, `every_documented_operation_has_a_unique_operation_id`, BDD "Every routed operation is documented in the contract" |
| API-S-002 / ADR 030 drift: Rust types ↔ contract | `every_component_schema_has_a_rust_sample`, `every_component_schema_matches_its_serialized_rust_type` (strict: no undocumented field, required present, enums equal Rust variants both ways), `the_contract_uses_only_checked_schema_keywords`, `health_schema_matches_the_core_payload_and_requires_every_field` |
| API-S-002 drift: contract ↔ generated TypeScript | Vitest `apps/filesquid/ui/tests/unit/contract.test.ts` |
| API-S-002 / CORE-S-005 operation schemas | `command_envelope_carries_idempotency_key_expected_revision_and_scope`, `query_envelope_and_page_size_parameter_are_bounded_like_core`, `errors_carry_code_and_retryability`, BDD "Operation schemas carry idempotency, revision, page size and retryability" |
| API-S-009 typed errors | `error_codes_match_the_rust_enum_with_status_and_retryability`, `error_code_all_lists_every_variant`, `every_error_response_carries_the_api_error_schema`, `unit_errors` (6 tests), BDD "A failed request returns an error the UI can show", "A request before the host is ready is a retryable error", "The host keeps answering after a failed request" |

[ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md) defines no Rust line-coverage tool, so no coverage percentage is reported. Every new public item (`ApiError`, `ErrorCode`, `routed_paths`, the `operation` re-exports) is exercised by the tests above.

The drift tests run in `scripts/pre-commit.sh`: `verify_tests.sh` runs `cargo test --workspace`, `cargo test -p filesquid-api` and `npm test` in `apps/filesquid/ui` (owner decision 2026-10-17: no hosted CI).

## Red: tests before the implementation

`cargo test -p filesquid-api --locked --test unit_contract --test unit_errors --test features` (excerpt; the tests referenced the API that did not exist yet):

```text
error[E0425]: cannot find function `routed_paths` in crate `filesquid_api`
error[E0432]: unresolved import `filesquid_api::operation`
error[E0432]: unresolved imports `filesquid_api::ApiError`, `filesquid_api::ErrorCode`
error[E0432]: unresolved imports `filesquid_api::routed_paths`, `filesquid_api::ApiError`, `filesquid_api::ErrorCode`
error[E0433]: cannot find `operation` in `filesquid_api`
error: could not compile `filesquid-api` (test "features") due to 3 previous errors
error: could not compile `filesquid-api` (test "unit_contract") due to 10 previous errors
error: could not compile `filesquid-api` (test "unit_errors") due to 1 previous error
```

`npx vitest run tests/unit/contract.test.ts` after the contract changed, before regenerating the types:

```text
   × src/api/schema.d.ts matches crates/api/openapi.json 796ms
 FAIL  tests/unit/contract.test.ts > src/api/schema.d.ts matches crates/api/openapi.json
AssertionError: expected 'export interface paths {\n    "/api/v…' to be 'export interface paths {\n    "/api/v…' // Object.is equality
      Tests  1 failed (1)
```

Mutation check: `ApiError.message` was removed from a copy of the contract, the tests were run, and the original was restored:

```text
test errors_carry_code_and_retryability ... FAILED
test every_component_schema_matches_its_serialized_rust_type ... FAILED
test result: FAILED. 14 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
```

## Green: raw output

`cargo test -p filesquid-api --locked --test unit_contract --test unit_errors --test features`:

```text
Feature: Versioned API contract
  Scenario: The API serves its own versioned contract
   ✔  Given a FileSquid API router
   ✔  When a client asks for the API contract
   ✔  Then the contract should be OpenAPI 3.1 at the crate's API version
  Scenario: Every routed operation is documented in the contract
   ✔  Given a FileSquid API router
   ✔  Then every path the router serves should be documented in the contract
   ✔  And every documented path should be served by the router or marked as a host route
  Scenario: Operation schemas carry idempotency, revision, page size and retryability
   ✔  Given the API contract
   ✔  Then a command envelope should require an operation id, an expected revision and a scope
   ✔  And a query page size should be bounded between 1 and 100
   ✔  And an API error should carry a code, a message and whether it is retryable
  Scenario: A failed request returns an error the UI can show
   ✔  Given a FileSquid server API that requires an administrator token
   ✔  When a client asks for the activity journal without a token
   ✔  Then the response should be an "unauthorized" error with a message
   ✔  And the error should not be retryable
  Scenario: A request before the host is ready is a retryable error
   ✔  Given a FileSquid API router whose monitor has not started
   ✔  When a client asks for the health report
   ✔  Then the response should be an "unavailable" error with a message
   ✔  And the error should be retryable
  Scenario: The host keeps answering after a failed request
   ✔  Given a FileSquid server API that requires an administrator token
   ✔  When an administrator asks for the activity journal with a malformed cursor
   ✔  Then the response should be an "invalid_request" error with a message
   ✔  When an administrator asks for the activity journal
   ✔  Then the response should be a journal page
[Summary]
4 features
70 scenarios (11 passed, 59 skipped)
100 steps (41 passed, 59 skipped)
running 16 tests
test error_code_all_lists_every_variant ... ok
test command_envelope_carries_idempotency_key_expected_revision_and_scope ... ok
test contract_is_openapi_3_1_with_the_crate_api_version ... ok
test error_codes_match_the_rust_enum_with_status_and_retryability ... ok
test errors_carry_code_and_retryability ... ok
test every_component_schema_has_a_rust_sample ... ok
test every_documented_operation_has_a_unique_operation_id ... ok
test every_error_response_carries_the_api_error_schema ... ok
test health_schema_matches_the_core_payload_and_requires_every_field ... ok
test host_routes_are_the_event_channel_only ... ok
test query_envelope_and_page_size_parameter_are_bounded_like_core ... ok
test every_documented_operation_is_answered_by_the_router ... ok
test router_serves_its_own_contract ... ok
test routed_and_documented_paths_agree_both_ways ... ok
test every_component_schema_matches_its_serialized_rust_type ... ok
test the_contract_uses_only_checked_schema_keywords ... ok
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
running 6 tests
test api_error_takes_retryability_from_its_code ... ok
test api_error_renders_as_json_with_the_code_status ... ok
test admin_routes_before_the_monitor_starts_are_unavailable_and_retryable ... ok
test invalid_and_malformed_query_parameters_are_invalid_requests ... ok
test missing_and_wrong_credentials_are_typed_errors ... ok
test the_router_keeps_answering_after_a_failed_request ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

The 59 skipped scenarios belong to the existing F-061, F-063 and F-064 features. Their domain operations do not exist yet, so they have no step definitions; this was already the case before this task.

`npx vitest run tests/unit/contract.test.ts` in `apps/filesquid/ui`:

```text
 ✓ tests/unit/contract.test.ts (1 test) 70ms
 Test Files  1 passed (1)
      Tests  1 passed (1)
```

Full gates: `cargo clippy --workspace --all-targets --locked -- -D warnings` (clean), `cargo test --workspace --locked` (exit 0, 54 test binaries ok, including `filesquid-server` and `filesquid-desktop` integration tests that mount the router), `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked` (clean). In `apps/filesquid/ui`: `npm run lint`, `npm run check` and `npm test` (7 files, 37 tests passed), plus `npx prettier --check .`.

## Mocked Environment Variables

None.
