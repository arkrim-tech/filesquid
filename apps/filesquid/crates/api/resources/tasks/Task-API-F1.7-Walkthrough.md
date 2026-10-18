# Task API-F1.7 Walkthrough: Authentication and authorisation

Module task [API-F1.7](../plan.md) of global Task FS-1.4 ([implementation-plan.md](../../../../../../resources/implementation-plan.md)). Branch `feat/api-f1.7-authorisation`. Builds on [API-F1.2](./Task-API-F1.2-Walkthrough.md).

## API-S-004: Every call is authenticated and authorised before files, SQL or keys are touched: per-launch token on desktop; accounts, groups and collection-level permissions on the server

> Every call MUST be authenticated and authorised before files, SQL or keys are touched: per-launch token on desktop; accounts, groups and collection-level permissions on the server. Access MUST be deny-by-default ([API-S-004](../srs.md), which now lists the granular constraints below).

Parents: [F-056 Single FileSquid API](../../../../../../resources/prd.md#f-056-single-filesquid-api), [SRS-F-056 Single FileSquid API](../../../../../../resources/srs.md#srs-f-056-single-filesquid-api) ("Every API call MUST be authorised before files, SQL or keys are touched. Access MUST be deny-by-default."), [F-057 Desktop and server hosts](../../../../../../resources/prd.md#f-057-desktop-and-server-hosts), [NFR-004 Security and privacy](../../../../../../resources/prd.md#nfr-004-security-and-privacy), FileSquid [U-06](../../../../../../resources/product-vision.md#platform-ownership-and-access), [U-18](../../../../../../resources/product-vision.md#one-api-hosts-and-distributions), [T-053](../../../../../../resources/product-vision.md#api-and-hosts), [ADR 030](../../../../../../resources/architecture/030-filesquid-api-and-transports.md) Decision 5.

This task also keeps [API-S-002](../srs.md) (versioned contract and drift tests), [API-S-006](../srs.md) (no storage I/O in the API crate) and [API-S-009](../srs.md) (errors the UI can show) for everything it adds.

### Secondary SRS mapping

This task also keeps three module requirements for everything it adds. Their full text:

- **API-S-002: The API contract MUST be versioned and owned by this crate (OpenAPI 3.1 in the repository). TypeScript types MUST be generated from it, and a test MUST fail when the contract and the generated types drift. Operations carry operation/idempotency key, expected revision, bounded page size, error code and retryability.** Covered here by contract 0.5.0 (global `bearerAuth`, 401 and 403 on every protected operation, `x-filesquid-action`, the public liveness probe with `security: []`, the `DenialReason` and `Liveness` schemas, `ApiError.reason`), the regenerated `schema.d.ts`, `unit_contract` and the Vitest `contract.test.ts`.
- **API-S-006: Application meaning (catalog, search, replica, the FileSquid OCFL application profile) MUST be implemented here once for every host. The crate MUST NOT link Tauri host APIs, libvips, FFmpeg or a SQLite driver and MUST NOT perform storage I/O itself; hosts inject storage backends. Cargo features MUST NOT be the only boundary.** Covered here by moving the administrator token file (`load_or_create_token`) out of `filesquid-api` into `filesquid-server`. The authorisation layer itself is pure apart from the journal queue and the random source.
- **API-S-009: Module and API boundaries MUST return errors the UI can show; a failed request MUST NOT crash a host.** Covered here by the typed denials (`ApiError` with `reason`, a message for people, never retryable, never repeating the credential) and the typed `method_not_allowed` answer on host routes.

## What changed

- **`auth` module** ([`src/auth/`](../../src/auth/mod.rs)):
  - `Principal` (`Owner`, `Administrator`), `Action` (`health.read`, `contract.read`, `events.subscribe`, `monitor.read`, and `public` for the liveness probe added in review round 1) and `Resource` (`Host`), all `#[non_exhaustive]`.
  - The API-F1.9 seams: the `Authenticator` trait (token to principal) and the `Policy` trait (principal, action and resource to a yes or no). `TokenAuthenticator` compares in constant time. `HostPolicy` grants the owner and the administrator every action on the host, and its exhaustive match makes a new principal a compile error until its permissions are decided. `DenyAll` grants nothing.
  - `Authoriser`: `deny_all()`, `desktop_owner(&LaunchToken)`, `server_administrator(token)` and `new(authenticator, policy)`. `authenticate` and `authorise` are pure.
  - `LaunchToken` and `generate_token`: 256 random bits as 64 hex characters, with redacted `Debug`.
  - `presented_credential` (named `presented_token` before review round 6): `Authorization: Bearer` (case-insensitive scheme) on any request, WebSocket upgrades included. Browsers cannot set headers on a WebSocket, so on an upgrade they may instead offer the `filesquid.bearer.<token>` subprotocol next to `filesquid.events.v1`. A present but unusable `Authorization` header is `invalid_credentials` and never falls back to the subprotocol.
  - The guard: `require` middleware with a `Guard { state, action }`, plus `authorise_call` and `check_caller`. A refusal logs one stdout line (INFO, target `filesquid::auth`, never journaled twice) and journals `api.auth.denied` through the bounded queue on a blocking task. The record holds the actor (`anonymous` or the principal), the reason as error code and the action as target. `token_issued_entry` builds `api.auth.token_issued`.
- **Router** ([`src/routes.rs`](../../src/routes.rs)): every route-table entry now names its `Action`, and `router()` wraps every protected entry in `require`. The sole unguarded exception is the public liveness probe `GET /api/v1/health/live` (added in review round 1); building the router panics if any other path declares `public`. The 404 and 405 fallbacks answer only an authenticated caller. `route_actions()` lists path and action. `ApiState::install_authoriser` installs the authoriser once; until it runs, every guarded call is refused.
- **Typed denial** ([`src/error.rs`](../../src/error.rs)): `DenialReason` (`missing_credentials` gives 401 `unauthorized`; `invalid_credentials` and `not_permitted` give 403 `forbidden`) and `ApiError::denied`. `ApiError` gains an optional `reason` member, omitted for other errors. Messages never repeat the credential.
- **Admin routes and monitor**: the interim `AdminAuth` is removed. Admin routes no longer check credentials themselves; their guard does (`monitor.read`). `MonitorConfig::new` and `HostMonitor::open` lose the `auth` argument. The admin-token file I/O (`load_or_create_token`) moved from the API crate to `filesquid-server` ([API-S-006](../srs.md)). An unreadable token file is now an error instead of being silently replaced.
- **Contract `0.5.0`** ([`openapi.json`](../../openapi.json)):
  - A global `security: [{bearerAuth: []}]`. The per-operation optional security (`{}`) is removed.
  - 401 and 403 on every protected operation, the event channel included, and `x-filesquid-action` on every operation. The public liveness probe (review round 1) has `security: []` and no denial responses, as the contract test checks.
  - The `DenialReason` schema and `ApiError.reason`.
  - The wire convention (authentication before 404 and 405) in `info.description`, and the WebSocket subprotocol carrier on `GET /api/v1/events`.
- **Generated types**: `apps/filesquid/ui/src/api/schema.d.ts`, regenerated with `npm run contract:types` (openapi-typescript 7.13.0). The Vitest drift test passes.
- **`filesquid-server`**:
  - `install_admin_authoriser` installs the administrator authoriser. `main` calls it before the monitor starts and exits with code 2 if it fails.
  - `GET /api/v1/events` and `/metrics` are guarded (`events.subscribe`, `monitor.read`). The event upgrade selects `filesquid.events.v1`.
  - The static UI stays public.
- **`filesquid-desktop`**:
  - `install_launch_token` generates the per-launch token and installs it. `builder()` now returns `Result` and fails closed.
  - A command-free plugin registers `launch_token_script`. It defines a read-only `window.__FILESQUID_LAUNCH_TOKEN__` only on the app's own origins (`APP_ORIGINS`) and only in the main frame.
  - The scheme handler answers the CORS preflight of app origins, which a cross-origin request with `Authorization` needs.
  - `api.auth.token_issued` is journaled at launch.
- **Docs**:
  - [ADR 030](../../../../../../resources/architecture/030-filesquid-api-and-transports.md) Decision 5 records the layer.
  - [API-S-004](../srs.md) lists the granular constraints. After review round 2 the liveness exception, the server token file and the journaling rule are their own requirements: [API-S-015, API-S-016 and API-S-017](../srs.md).
  - The [SRS-F-056](../../../../../../resources/srs.md#srs-f-056-single-filesquid-api) test gap is replaced by the new tests.
  - [DE-F1.1](../../../desktop/resources/plan.md) notes what landed here.
  - The root README covers the token in the server commands (run first, see below), `/metrics` with the token, and the authorisation test commands.

## Test to requirement mapping

| Requirement / constraint | Tests |
|---|---|
| Deny-by-default: no installed authoriser refuses every call | `without_an_installed_authoriser_every_route_is_denied`, `a_server_without_an_installed_authoriser_refuses_every_route`; BDD "A host that installed no authoriser refuses every call" |
| Every route refuses a call without the token (401 `missing_credentials`), health and contract included | `every_route_denies_a_call_without_the_per_launch_token`, `every_server_route_refuses_a_call_without_the_token` (all API routes, `/api/v1/events`, `/metrics`), desktop `a_call_without_the_per_launch_token_is_refused`; BDD "Every route refuses a call without the per-launch token", "The health route and the event channel are not public" |
| Wrong token (403 `invalid_credentials`): another launch, near miss, empty | `every_route_denies_a_wrong_token`, `every_server_route_refuses_a_wrong_token`, desktop `a_token_from_another_launch_is_refused`; BDD "Every route refuses a call with a wrong token" |
| Holder of the token reaches every route | `every_route_answers_the_per_launch_token_holder`, `every_documented_operation_answers_a_documented_status_and_body`; BDD "The owner's own webview reaches every route" |
| Authenticate before 404/405 | `unknown_paths_and_wrong_methods_authenticate_before_routing`; BDD "An unknown path does not reveal the route table" |
| Policy seam: known principal not permitted (403 `not_permitted`) | `a_principal_the_policy_does_not_permit_is_forbidden`; BDD "A signed-in principal the policy does not permit is refused" |
| Host routes use the same layer; WebSocket subprotocol carrier only on upgrades | `host_routes_are_denied_by_default`, `a_websocket_upgrade_may_carry_the_token_as_a_subprotocol`, server `the_event_channel_refuses_a_websocket_without_the_token`, `a_browser_may_send_the_token_as_a_websocket_subprotocol` (real TCP; `filesquid.events.v1` selected) |
| Credential parsing | `the_bearer_scheme_is_read_case_insensitively`, `an_empty_bearer_token_is_presented_and_then_rejected`, `the_subprotocol_carrier_is_read_from_any_offered_entry` |
| 256-bit hex tokens, constant-time compare, redacted `Debug`, install once | `launch_tokens_are_unique_256_bit_hex`, `the_token_authenticator_accepts_only_the_exact_token`, `equal_slices_compare_equal_and_every_difference_is_found`, `debug_output_never_shows_a_secret`, `an_authoriser_can_be_installed_only_once`, desktop `each_launch_installs_a_new_token_once` |
| Typed denial (API-S-009): codes, reasons, not retryable, no credential echo | `denials_map_reasons_to_codes_and_are_not_retryable`, `an_error_without_a_reason_omits_the_member`, `a_denial_never_echoes_the_presented_credential` |
| Journal: `api.auth.denied` and `api.auth.token_issued`, redaction tests | `every_denial_is_journaled_without_the_presented_credential`, `a_not_permitted_denial_names_the_principal_and_the_action`, `the_token_issued_entry_names_the_principal_and_never_a_secret`; BDD "Every refusal is journaled without the presented credential" |
| Contract (API-S-002): global bearer, 401/403 everywhere, actions, `DenialReason` | `every_operation_requires_the_bearer_scheme_and_none_makes_it_optional`, `every_operation_documents_both_denials`, `every_operation_names_the_action_the_router_authorises`, `authentication_before_routing_is_documented_as_a_wire_convention`, `every_routed_path_declares_its_action`, `every_component_schema_matches_its_serialized_rust_type` (`DenialReason`, `ApiError.reason`); Vitest `contract.test.ts` |
| Desktop: token only to app origins, CORS preflight | `the_init_script_hands_the_token_only_to_app_origins`, `app_origins_may_preflight_the_authorization_header`, `foreign_origins_get_no_preflight_grant` |
| Server admin token file (owner-only, reused on restart); static UI public | `a_missing_admin_token_is_generated_into_the_data_dir_with_owner_only_access`, `the_static_ui_stays_public` |

[ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md) defines no Rust line-coverage tool, so no coverage percentage is reported. Every new public item in `auth`, `DenialReason`, `ApiError::denied`, `route_actions`, `install_admin_authoriser`, `install_launch_token` and `launch_token_script` is exercised by the tests above.

## Red: tests before the implementation

`cargo test -p filesquid-api --locked --no-run`, then the same for `-p filesquid-server` and `-p filesquid-desktop` (unique errors, counted; the tests referred to an API that did not exist yet):

```text

4 error[E0599]: no method named `install_authoriser` found for struct `ApiState` in the current scope
4 error[E0061]: this function takes 3 arguments but 2 arguments were supplied
3 error[E0432]: unresolved import `filesquid_api::auth`
1 error: could not compile `filesquid-api` (test "unit_health") due to 2 previous errors
1 error: could not compile `filesquid-api` (test "unit_errors") due to 4 previous errors
1 error: could not compile `filesquid-api` (test "unit_admin") due to 5 previous errors
2 error[E0432]: unresolved import `filesquid_server::install_admin_authoriser`
1 error[E0599]: no method named `install_authoriser` found for struct `ApiState` in the current scope
1 error[E0432]: unresolved import `filesquid_api::routed_paths`
1 error[E0432]: unresolved import `filesquid_api::auth`
1 error: could not compile `filesquid-server` (test "integration_static_ui") due to 2 previous errors
1 error: could not compile `filesquid-server` (test "integration_monitor") due to 1 previous error
1 error: could not compile `filesquid-server` (test "integration_auth") due to 2 previous errors
1 error[E0603]: constant `APP_ORIGINS` is private
1 error[E0432]: unresolved import `filesquid_desktop_lib::install_launch_token`
1 error[E0432]: unresolved import `filesquid_desktop_lib::api_protocol::launch_token_script`
1 error[E0432]: unresolved import `filesquid_api::auth`
1 error[E0061]: this function takes 3 arguments but 2 arguments were supplied
1 error: could not compile `filesquid-desktop` (test "integration_api_protocol") due to 5 previous errors
```

## Green: raw output

From `apps/filesquid/` with `CARGO_TARGET_DIR` on the shared target, `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0` and `TMPDIR` on a tmpfs with more than 5 GiB free:

```text
$ cargo fmt --all --check            # exit 0
$ cargo clippy --workspace --all-targets --locked -- -D warnings
Finished `dev` profile [unoptimized] target(s) in 25.88s
$ cargo doc --workspace --no-deps     # exit 0, no warnings
```

`cargo test -p filesquid-api --locked` (every target):

```text
     Running unittests src/lib.rs (target/debug/deps/filesquid_api-e692301c8f1ac3fe)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/features.rs (target/debug/deps/features-ea430bbe3e9bab64)
85 scenarios (26 passed, 59 skipped)
154 steps (95 passed, 59 skipped)
     Running tests/unit/admin.rs (target/debug/deps/unit_admin-81e7ee21ba4f723c)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/auth.rs (target/debug/deps/unit_auth-0eab147cebfe3bc2)
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/unit/contract.rs (target/debug/deps/unit_contract-0451289c9c685355)
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
     Running tests/unit/errors.rs (target/debug/deps/unit_errors-974d82f14842e04a)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/events.rs (target/debug/deps/unit_events-e86309e482d907d9)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests/unit/health.rs (target/debug/deps/unit_health-6b2889fc225dd40a)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/health_rules.rs (target/debug/deps/unit_health_rules-6e932182b190a254)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/journal.rs (target/debug/deps/unit_journal-c2b24783ef33de5f)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/metrics.rs (target/debug/deps/unit_metrics-51cf8ccfb91e5de9)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/registry.rs (target/debug/deps/unit_registry-864a1c1d906051c0)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/unit/routes.rs (target/debug/deps/unit_routes-31db950b4fe399ae)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/writer.rs (target/debug/deps/unit_writer-a83b9cc5bea7b1be)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`unit_auth`:

```text
     Running tests/unit/auth.rs (target/debug/deps/unit_auth-0eab147cebfe3bc2)

running 24 tests
test a_websocket_upgrade_may_carry_the_token_as_a_subprotocol ... ok
test a_principal_the_policy_does_not_permit_is_forbidden ... ok
test a_denial_never_echoes_the_presented_credential ... ok
test a_not_permitted_denial_names_the_principal_and_the_action ... ok
test action_names_are_unique_dotted_identifiers ... ok
test an_empty_bearer_token_is_presented_and_then_rejected ... ok
test debug_output_never_shows_a_secret ... ok
test denials_map_reasons_to_codes_and_are_not_retryable ... ok
test an_authoriser_can_be_installed_only_once ... ok
test an_error_without_a_reason_omits_the_member ... ok
test every_denial_is_journaled_without_the_presented_credential ... ok
test every_route_denies_a_call_without_the_per_launch_token ... ok
test host_routes_are_denied_by_default ... ok
test launch_tokens_are_unique_256_bit_hex ... ok
test the_bearer_scheme_is_read_case_insensitively ... ok
test every_routed_path_declares_its_action ... ok
test the_subprotocol_carrier_is_read_from_any_offered_entry ... ok
test the_token_authenticator_accepts_only_the_exact_token ... ok
test the_host_policy_grants_the_owner_and_the_administrator_every_action ... ok
test the_token_issued_entry_names_the_principal_and_never_a_secret ... ok
test every_route_answers_the_per_launch_token_holder ... ok
test unknown_paths_and_wrong_methods_authenticate_before_routing ... ok
test every_route_denies_a_wrong_token ... ok
test without_an_installed_authoriser_every_route_is_denied ... ok

test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
```

`unit_contract`:

```text
     Running tests/unit/contract.rs (target/debug/deps/unit_contract-0451289c9c685355)

running 27 tests
test authentication_before_routing_is_documented_as_a_wire_convention ... ok
test contract_is_openapi_3_1_with_the_crate_api_version ... ok
test command_envelope_carries_idempotency_key_expected_revision_and_scope ... ok
test error_code_all_lists_every_variant ... ok
test event_topics_in_the_contract_match_the_rust_topics ... ok
test errors_carry_code_and_retryability ... ok
test error_codes_match_the_rust_enum_with_status_and_retryability ... ok
test every_component_schema_has_a_rust_sample ... ok
test every_documented_operation_has_a_unique_operation_id ... ok
test every_component_schema_matches_its_serialized_rust_type ... ok
test every_error_response_carries_the_api_error_schema ... ok
test every_event_topic_documents_its_payload_schema ... ok
test every_limit_parameter_is_the_shared_page_size ... ok
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
test every_documented_operation_answers_a_documented_status_and_body ... ok
test router_fallbacks_answer_as_documented ... ok
test events_from_every_publisher_match_their_documented_payloads ... ok

test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
```

BDD `deny_by_default_authorisation_F-056.feature`, from the `features` target. The whole run has 6 features and 85 scenarios: 26 passed and 59 skipped. The skipped scenarios belong to features whose domain operations are not built yet (F-061 OTLP export, F-063 search, F-064 activity); they were skipped before this task too.

```text
Feature: Deny-by-default authorisation
  Scenario: The owner's own webview reaches every route with the per-launch token
   ✔> Given a desktop FileSquid API with a per-launch token
   ✔  When the owner's webview calls every API route with the per-launch token
   ✔  Then every call should be answered
  Scenario: Every route refuses a call without the per-launch token
   ✔> Given a desktop FileSquid API with a per-launch token
   ✔  When a client calls every API route without a token
   ✔  Then every call should be refused as "unauthorized" because of "missing_credentials"
   ✔  And no refusal should be retryable
  Scenario: Every route refuses a call with a wrong token
   ✔> Given a desktop FileSquid API with a per-launch token
   ✔  When a client calls every API route with a token from another launch
   ✔  Then every call should be refused as "forbidden" because of "invalid_credentials"
   ✔  And no refusal should be retryable
  Scenario: The health route and the event channel are not public
   ✔> Given a desktop FileSquid API with a per-launch token
   ✔  When a client asks for the health status without a token
   ✔  And a client opens the event channel without a token
   ✔  Then every call should be refused as "unauthorized" because of "missing_credentials"
  Scenario: An unknown path does not reveal the route table to an anonymous caller
   ✔> Given a desktop FileSquid API with a per-launch token
   ✔  When a client asks for an API path that does not exist without a token
   ✔  Then every call should be refused as "unauthorized" because of "missing_credentials"
  Scenario: A host that installed no authoriser refuses every call
   ✔> Given a desktop FileSquid API with a per-launch token
   ✔  Given a FileSquid API router with no authoriser installed
   ✔  When the owner's webview calls every API route with the per-launch token
   ✔  Then every call should be refused as "forbidden" because of "invalid_credentials"
  Scenario: A browser opens the event channel with the token as a WebSocket subprotocol
   ✔> Given a desktop FileSquid API with a per-launch token
   ✔  When the owner's webview opens the event channel offering the token as a subprotocol
   ✔  Then every call should be answered
  Scenario: A signed-in principal the policy does not permit is refused
   ✔> Given a desktop FileSquid API with a per-launch token
   ✔  Given a desktop FileSquid API whose policy does not permit reading the monitor
   ✔  When the owner's webview asks for the monitor health with the per-launch token
   ✔  Then every call should be refused as "forbidden" because of "not_permitted"
  Scenario: Every refusal is journaled without the presented credential
   ✔> Given a desktop FileSquid API with a per-launch token
   ✔  When a client calls every API route with a token from another launch
   ✔  Then the activity journal should record one "api.auth.denied" entry per refused call
   ✔  And no journal entry should contain the presented token
```

`cargo test -p filesquid-server --locked`:

```text
     Running unittests src/lib.rs (target/debug/deps/filesquid_server-fc8dee05b3b755bb)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (target/debug/deps/filesquid_server-0976992133571182)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/auth.rs (target/debug/deps/integration_auth-36720fe2c3bda221)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/events.rs (target/debug/deps/integration_events-122f5a81df14c043)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/integration/health.rs (target/debug/deps/integration_health-5deec2ed9bd7b5a1)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/monitor.rs (target/debug/deps/integration_monitor-0fb5504cca00d7e1)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/static_ui.rs (target/debug/deps/integration_static_ui-5f103cbfe84b1b81)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/health.rs (target/debug/deps/unit_health-073032d0aeb84476)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/integration/auth.rs (target/debug/deps/integration_auth-36720fe2c3bda221)

running 7 tests
test a_missing_admin_token_is_generated_into_the_data_dir_with_owner_only_access ... ok
test a_browser_may_send_the_token_as_a_websocket_subprotocol ... ok
test a_server_without_an_installed_authoriser_refuses_every_route ... ok
test the_event_channel_refuses_a_websocket_without_the_token ... ok
test the_static_ui_stays_public ... ok
test every_server_route_refuses_a_call_without_the_token ... ok
test every_server_route_refuses_a_wrong_token ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

`cargo test -p filesquid-desktop --locked`:

```text
     Running unittests src/lib.rs (target/debug/deps/filesquid_desktop_lib-317172dd40e1a316)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (target/debug/deps/filesquid_desktop-d2788ae594f56e29)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/api_protocol.rs (target/debug/deps/integration_api_protocol-0595c3f395bad985)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/integration/api_protocol.rs (target/debug/deps/integration_api_protocol-0595c3f395bad985)

running 13 tests
test a_token_from_another_launch_is_refused ... ok
test app_origins_may_preflight_the_authorization_header ... ok
test app_origins_receive_cors_headers ... ok
test foreign_origins_get_no_preflight_grant ... ok
test each_launch_installs_a_new_token_once ... ok
test foreign_origins_receive_no_cors_headers ... ok
test a_call_without_the_per_launch_token_is_refused ... ok
test scheme_name_is_filesquid ... ok
test health_is_served_through_the_custom_scheme ... ok
test the_init_script_hands_the_token_only_to_app_origins ... ok
test windows_style_scheme_urls_are_served_too ... ok
test unknown_paths_are_not_found ... ok
test the_desktop_owner_reaches_admin_routes_with_the_per_launch_token ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

Generated types and drift test, from `apps/filesquid/ui`:

```text
$ npm run contract:types
✨ openapi-typescript 7.13.0
🚀 ../crates/api/openapi.json → src/api/schema.d.ts [159.3ms]
$ npx vitest run tests/unit/contract.test.ts
 ✓ tests/unit/contract.test.ts (1 test) 105ms
 Test Files  1 passed (1)
      Tests  1 passed (1)
```

README commands, run against a real `filesquid-server` (`FILESQUID_UI_DIR=ui/dist cargo run -p filesquid-server` in `apps/filesquid`, token generated at first start):

```text
$ curl -s -w ' %{http_code}\n' http://127.0.0.1:8083/api/v1/health
{"code":"unauthorized","message":"FileSquid needs credentials for this request. Sign in or reopen the app.","retryable":false,"reason":"missing_credentials"} 401
$ TOKEN=$(cat filesquid-data/admin-token)
$ curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8083/api/v1/health
{"schema_version":1,"status":"ok","product":"filesquid"}
$ curl -H "Authorization: Bearer $TOKEN" "http://127.0.0.1:8083/api/v1/admin/journal?order=desc&limit=20"
{"entries":[{"id":4,"time":"2026-10-17T23:42:50.749Z","host":"filesquid-server","actor":"anonymous","operation":"api.auth.denied","outcome":"failed","level":"warn","target_ids":["health.read"],"error_ ...
$ curl -s -w ' %{http_code}\n' http://127.0.0.1:8083/metrics
{"code":"unauthorized", ... "reason":"missing_credentials"} 401
$ curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8083/metrics
# HELP filesquid_up 1 while the FileSquid host is running
# TYPE filesquid_up gauge
filesquid_up 1
$ curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8083/
200
```

## Review round 1: owner decisions and Cubic findings

### Owner decisions

- **Public liveness probe.** `GET /api/v1/health/live` answers `{"status":"up"}` without a credential: no version, component detail or journal. Its route-table entry carries the action `public` (`Action::Public`), the one action the guard lets through, so the exception is explicit rather than a gap. The contract gives it `security: []` and `x-filesquid-action: public`, and adds the `Liveness` schema. Full health stays behind the token. Tests: `only_the_liveness_route_is_public`, `the_liveness_route_answers_without_a_token_and_says_only_that_the_process_is_up`, `the_liveness_route_still_refuses_an_anonymous_wrong_method`, `a_public_host_route_runs_without_a_token`, `the_only_public_operation_is_the_liveness_probe`, server `the_liveness_probe_answers_without_a_token`, desktop `the_liveness_probe_needs_no_token`, BDD "A liveness probe needs no token and learns only that the process is up". The deny-by-default tests now loop over every protected route, which is every route except the probe.
- **Journaling.** The journal records refusals plus credential issue (no rotation event exists, so none is claimed). Successful calls are not journaled one by one; what they change is journaled as the operations they cause. The rule text now says exactly that in [API-S-017](../srs.md), the [PRD](../../../../../../resources/prd.md) 100% logging bullet, the [product vision](../../../../../../resources/product-vision.md) FE-081, [ADR 033](../../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md) and `.agents/rules/logging-enforce.md`.
- **UI client.** Wiring `FileSquidClient` to send the token ships as a separate small `@filesquid/ui` pull request right after this one merges. [DE-F1.1](../../../desktop/resources/plan.md) says so.

### Cubic findings

| Finding | Fix | Test (written first) |
|---|---|---|
| P1: a group- or world-readable token file was accepted (`admin_token.rs`) | The token file now fails closed on Unix. A stored file is used only when it is a regular file (not a symbolic link), owned by the server user (`geteuid`) and has no group or other permission bits. Anything else stops the server with an error that names `chmod 600` and never contains the token. Checked by hand: a 0644 file stops the server with exit code 2. | `a_group_or_world_readable_token_file_is_refused` (0644, 0640, 0604), `a_symlinked_token_file_is_refused`, `an_owner_only_token_file_is_reused` |
| P2: two first starts could write different tokens | The file is created with `create_new`. A loser of the race rereads the winner's token, waiting at most 50 x 20 ms. | `concurrent_first_starts_agree_on_one_token` (20 rounds of 8 threads) |
| P2: the token compare leaked the token's length | `TokenAuthenticator` keeps only the SHA-256 digest and compares 32-byte digests in constant time (`sha2` 0.11, already in the workspace; ADR 009 suite) | `secrets_compare_by_digest_whatever_their_lengths` |
| P2: Axum's unguarded 405 on `/api/v1/events` and `/metrics` | Host routes are guarded with `layer` (which also covers the method fallback), and the fallback answers a typed `method_not_allowed` | `other_methods_on_host_routes_are_refused_before_routing` |
| P2: the bearer subprotocol was accepted without `filesquid.events.v1` | The token entry counts only when `filesquid.events.v1` is offered too | `the_subprotocol_token_counts_only_next_to_the_events_subprotocol` |
| P2: fallback refusals were journaled without a target | They are journaled with the target `no_route` (`NO_ROUTE_TARGET`) | `fallback_refusals_are_journaled_with_the_no_route_target` |
| P2: with no monitor, an INFO line hidden by `RUST_LOG=warn` was the only record of a refusal | Without a monitor the redacted entry is printed straight to stdout as one JSON line (`journal_unavailable: true`), bypassing the log filter | `without_a_monitor_a_refusal_goes_to_stdout_unfiltered` |
| P2: `X-Journal-Next-Cursor` was not exposed to app origins | `Access-Control-Expose-Headers: x-journal-next-cursor` on app-origin responses | `app_origin_responses_expose_the_journal_cursor_header` |
| P2: client wiring untracked (`desktop/plan.md`) | DE-F1.1 names the follow-up `@filesquid/ui` pull request | Documentation |
| P3: vacuous `evil.example` assertion | The test asserts the secret appears exactly once, after the origin guard | `the_init_script_hands_the_token_only_to_app_origins` |
| P3: vacuous journal-redaction assertion | The test asserts the exact serialized field set of `api.auth.token_issued` | `the_token_issued_entry_has_no_field_that_could_carry_a_secret` |
| P3: secondary SRS mapping without full titles | The full text of API-S-002, API-S-006 and API-S-009 is above | Documentation |

### Red: round 1 tests before the fixes

Liveness, before `Action::Public` existed (`cargo test -p filesquid-api --locked --no-fail-fast`):

```text
error: could not compile `filesquid-api` (test "features") due to 1 previous error
error: could not compile `filesquid-api` (test "unit_auth") due to 4 previous errors
error[E0599]: no variant, associated function, or constant named `Public` found for enum `Action` in the current scope
```

Cubic findings, before the fixes (`cargo test -p filesquid-api -p filesquid-server -p filesquid-desktop --locked --no-fail-fast`; the API lib tests and `unit_auth` did not compile, and five host tests failed):

```text
error[E0425]: cannot find function `digest` in this scope
error[E0425]: cannot find function `secret_eq` in this scope
error[E0433]: cannot find type `Recorded` in this scope
error[E0425]: cannot find value `NO_ROUTE_TARGET` in module `auth`
---- a_group_or_world_readable_token_file_is_refused stdout ----
---- a_symlinked_token_file_is_refused stdout ----
---- concurrent_first_starts_agree_on_one_token stdout ----
---- other_methods_on_host_routes_are_refused_before_routing stdout ----
---- app_origin_responses_expose_the_journal_cursor_header stdout ----
test result: FAILED. 9 passed; 4 failed (integration_auth)
test result: FAILED. 14 passed; 1 failed (integration_api_protocol)
```

### Green: round 1 raw output

`cargo test -p filesquid-api -p filesquid-server -p filesquid-desktop --locked` (exit code 0):

```text
     Running unittests src/lib.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/features.rs
86 scenarios (27 passed, 59 skipped)
     Running tests/unit/admin.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/auth.rs
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/contract.rs
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
     Running tests/unit/errors.rs
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/events.rs
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/unit/health.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/health_rules.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/journal.rs
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/metrics.rs
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/registry.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/routes.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/writer.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/api_protocol.rs
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running unittests src/lib.rs
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/auth.rs
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/integration/events.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/integration/health.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/monitor.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/static_ui.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/health.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Clippy (`-D warnings`), `cargo fmt --check`, the Vitest contract drift test (after `npm run contract:types`) and `cargo doc --workspace --no-deps` pass.

README commands, run against a real `filesquid-server`:

```text
$ curl http://127.0.0.1:8083/api/v1/health/live
{"status":"up"}
$ curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8083/api/v1/health
{"schema_version":1,"status":"ok","product":"filesquid"}
$ curl -s -X POST -w ' %{http_code}\n' http://127.0.0.1:8083/metrics
{"code":"unauthorized","message":"FileSquid needs credentials for this request. Sign in or reopen the app.","retryable":false,"reason":"missing_credentials"} 401
$ chmod 644 filesquid-data/admin-token && cargo run -q -p filesquid-server
{"level":"ERROR","fields":{"message":"cannot set up API authorisation","message":"filesquid-data/admin-token is readable by other users; run chmod 600 on it and restart"}, ...}
exit=2
```

## Review round 2: Cubic findings on 779ca42

| Finding | Fix | Test (written first) |
|---|---|---|
| P1: the token file was checked with `symlink_metadata`, then reopened by path (TOCTOU) | On Unix the file is opened once with `O_NOFOLLOW`. The owner, mode and regular-file checks run on that handle's metadata (`fstat`), and the token is read from the same handle. | `the_token_file_is_opened_once_without_following_a_link` (lib), `a_symlinked_token_file_is_refused` |
| P2: a blank `admin-token` was a permanent startup failure | `create_with` removes the file when the write or sync fails. A blank, owner-only, regular file is treated as left by a crashed first start: after a bounded wait (50 x 20 ms) for a concurrent writer, it is replaced atomically (temporary file, then rename). The permission checks still apply to blank files. | `a_failed_write_removes_the_partly_written_file` (lib), `a_blank_owner_only_token_file_left_by_a_crash_is_replaced`, `a_blank_token_file_open_to_others_is_still_refused` |
| P2: `Action::Public` bypassed the guard on any route | `auth::require` never bypasses authentication. Only the router leaves a `public` entry unguarded, and building the router panics if any path other than `/health/live` declares `public`. | `a_host_route_cannot_be_made_public_through_the_guard`, `a_route_other_than_the_liveness_probe_cannot_be_public` (lib, `should_panic`), `the_liveness_probe_is_the_one_public_path` (lib) |
| P2: the server never journaled its administrator token issue | `start_server` installs the authoriser, opens the monitor, then journals `api.auth.token_issued` (target `administrator`) at every start, whether the token was generated or loaded. `main` uses it. | `every_start_journals_the_administrator_token_issue` |
| P2: ADR 033 Status and Rationale contradicted the narrowed journaling rule | Status now defines "activity" with the owner decision (successful API calls are not journaled one by one); Decision 2 refers to it; Rationale explains the exclusion and states that stdout is not a durable record. The "visible as API calls" claim is gone everywhere. | Documentation |
| P2: README remedy was `chmod` only, with a hard-coded path | Separate remedies for permissions, ownership and symbolic links, using `$FILESQUID_DATA_DIR`. The server's error messages name the same remedies. | Run against the real binary (below) |
| P3: API-S-004 packed too many rules | Split into [API-S-004](../srs.md) (deny-by-default layer), API-S-015 (liveness exception), API-S-016 (server token file) and API-S-017 (journaling). The plan row and the feature file cite them. | Documentation |
| P3: feature steps said "every API route" | They say "every protected API route", matching `protected_paths()` | BDD run |
| P3: vacuous digest-length check | The test checks the FIPS 180-4 vector SHA-256("abc") = `ba7816bf...f20015ad`. It passed on first run because the digest was already correct; it now proves the algorithm, not the array length. | `secrets_compare_by_digest_whatever_their_lengths` |
| P3: "rotation" claimed without a rotation event | Removed from the walkthrough, API-S-017, ADR 030, ADR 033, the PRD, FE-081 and `logging-enforce.md`; the rule now says "credential issue" only | Documentation |
| P3: README said every API call needs a token | It now says every route except the liveness probe | Documentation |
| P3: layer.rs comment said "instead", and ADR 030 said "one JSON line" | Both now say the entry is printed in addition to the INFO line | Documentation |
| P3: DE-F1.1 still listed "guard every request with a per-launch token" | Phrase removed | Documentation |

### Red: round 2 tests before the fixes

```text
error[E0425]: cannot find function `create_with` in this scope
error[E0425]: cannot find function `read_token_file` in this scope
error: could not compile `filesquid-server` (lib test) due to 2 previous errors
error[E0432]: unresolved import `filesquid_server::start_server`
error: could not compile `filesquid-server` (test "integration_auth") due to 1 previous error
---- a_host_route_cannot_be_made_public_through_the_guard stdout ----
test result: FAILED. 29 passed; 1 failed (unit_auth)
error[E0061]: this function takes 3 arguments but 4 arguments were supplied
error[E0425]: cannot find value `LIVENESS_PATH` in this scope
error: could not compile `filesquid-api` (lib test) due to 4 previous errors
```

### Green: round 2 raw output

`cargo test -p filesquid-api -p filesquid-server -p filesquid-desktop --locked` (exit code 0):

```text
     Running unittests src/lib.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
     Running tests/features.rs
86 scenarios (27 passed, 59 skipped)
     Running tests/unit/admin.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/auth.rs
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/contract.rs
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
     Running tests/unit/errors.rs
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/events.rs
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/unit/health.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/health_rules.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/journal.rs
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/metrics.rs
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/registry.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/routes.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/writer.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running unittests src/lib.rs
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/api_protocol.rs
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running unittests src/lib.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/auth.rs
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s
     Running tests/integration/events.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/integration/health.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/monitor.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/static_ui.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/health.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

The README token-file remedies, run against the real `filesquid-server` binary with `FILESQUID_DATA_DIR` set (the ownership case needs a second user and was not run here; its message is pinned by `check_private`):

```text
--- first start
"$FILESQUID_DATA_DIR/admin-token"
"filesquid-server listening"
--- chmod 644
"cannot start API authorisation or the activity journal"
".../admin-token is readable by other users; run chmod 600 on it and restart"
--- chmod 600 "$FILESQUID_DATA_DIR/admin-token"
"filesquid-server listening"
--- symbolic link
"cannot start API authorisation or the activity journal"
".../admin-token is a symbolic link; delete it and restart to generate a new token, or replace it with a regular file holding the token (then chmod 600 it)"
--- delete the link and restart
"$FILESQUID_DATA_DIR/admin-token"
"filesquid-server listening"
```

## Review round 3: Cubic findings on a1bdb55

Three rounds of edge cases landed in the same token-file code, so this round shrinks what that code handles instead of patching it again: the token file now exists only on Unix.

| Finding | Fix | Test (written first) |
|---|---|---|
| P2: on Windows a symlink swap after `symlink_metadata` bypassed the refusal | Not patched. On non-Unix targets the server never reads or writes a token file: it requires `FILESQUID_ADMIN_TOKEN` and refuses to start without it, with an error that names the variable. The whole token-file implementation is now a `#[cfg(unix)]` module. API-S-016, ADR 030 and the README say so. | `without_a_configured_token_the_server_refuses_to_start` (compiled and run on non-Unix targets only; this Linux run cannot execute it) |
| P2: a FIFO at `admin-token` hung the open | The open uses `O_NOFOLLOW \| O_NONBLOCK`, so it returns, and the regular-file check on the handle rejects the FIFO with a remedy. | `a_fifo_at_the_token_path_is_refused_without_hanging` (skips when `mkfifo` is unavailable; it ran here). Red: without `O_NONBLOCK` it failed after its 5 s timeout. |
| P2: a PID-named temporary file orphaned by a crash could block recovery | Temporary files get a random name (`admin-token.<16 hex>.tmp`) created with `create_new`, retried at most 5 times on `AlreadyExists`. At startup, stale `admin-token.*.tmp` files are removed, but only regular files (never links) owned by the server user. | `an_orphaned_temporary_file_does_not_block_recovery`, `stale_cleanup_never_touches_a_link_or_another_name`, `temporary_names_are_random_not_the_process_id` |
| P3: README applied `Authorization` to the WebSocket | HTTP requests and `/metrics` use the Bearer header; browsers use the subprotocol pair on the WebSocket | Documentation |
| P3: README missed the not-a-regular-file case | New bullet: remove it (for example a directory or a FIFO) and restart | Run against the real binary (below) |
| P3: API-S-016 had no number for the wait | It now states at most 50 rereads 20 ms apart (1 s), and the random temporary name and the stale cleanup | Documentation |
| P3: router docs said every route is wrapped | The module, `router`, `auth` and crate docs name the liveness probe as the one route outside the guard | `cargo doc` |
| P3: API-S-017 tag without a `token_issued` scenario | New scenario "The desktop journals the issue of its per-launch token at launch". It uses the new `auth::record_token_issue`, which both hosts now call. | BDD scenario (28 passed, up from 27) |

### Red: round 3 tests before the fixes

```text
error[E0425]: cannot find function `remove_stale_temporaries` in this scope
error[E0599]: no method named `expect` found for struct `PathBuf` in the current scope
error: could not compile `filesquid-server` (lib test) due to 3 previous errors
error[E0425]: cannot find function `record_token_issue` in module `auth`
error: could not compile `filesquid-api` (test "features") due to 1 previous error
```

With the fixes in and `O_NONBLOCK` removed again, to show the FIFO test catches the hang:

```text
thread 'admin_token::tests::a_fifo_at_the_token_path_is_refused_without_hanging' panicked at crates/server/src/admin_token.rs:354:14:
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 5 filtered out; finished in 5.08s
```

### Green: round 3 raw output

`cargo test -p filesquid-api -p filesquid-server -p filesquid-desktop --locked` (exit code 0):

```text
     Running unittests src/lib.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/features.rs
87 scenarios (28 passed, 59 skipped)
     Running tests/unit/admin.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/auth.rs
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/unit/contract.rs
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
     Running tests/unit/errors.rs
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/events.rs
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/unit/health.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/health_rules.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/journal.rs
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/metrics.rs
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/registry.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/routes.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/writer.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/api_protocol.rs
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running unittests src/lib.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s
     Running unittests src/main.rs
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/auth.rs
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s
     Running tests/integration/events.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/integration/health.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/monitor.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/static_ui.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/health.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

The README "not a regular file" remedy, run against the real `filesquid-server` binary:

```text
--- a directory at the token path
"cannot start API authorisation or the activity journal"
"$FILESQUID_DATA_DIR/admin-token is not a regular file; remove it (for example a directory or a FIFO) and restart to generate a new token"
--- a FIFO at the token path
"cannot start API authorisation or the activity journal"
"$FILESQUID_DATA_DIR/admin-token is not a regular file; remove it (for example a directory or a FIFO) and restart to generate a new token"
--- remove it and restart
"$FILESQUID_DATA_DIR/admin-token"
"filesquid-server listening"
```

## Review round 4: Cubic findings on feab6ac

This round keeps shrinking the token-file code instead of adding machinery.

| Finding | Fix | Test |
|---|---|---|
| P2: the startup cleanup of `admin-token.*.tmp` could delete a concurrent server's recovery file (`admin_token.rs:93`) | The cleanup is **removed**, not made concurrency-safe. Startup never deletes temporary files. Random names and `create_new` already mean an orphan left by a crash never blocks recovery, so an orphan is a harmless leftover. API-S-016, ADR 030 and the module docs say so; the round-3 cleanup test is gone. | `an_orphaned_temporary_file_does_not_block_recovery` now also asserts that the orphan is left untouched (red with the cleanup, green without) |
| P2: "stale" was undefined in API-S-016 (`srs.md:27`) | Resolved by the removal: API-S-016 now says startup MUST NOT delete temporary token files, and an orphan MUST NOT block a later recovery | Same test |
| P3: the cleanup requirement was broader than the implementation (`srs.md:27`) | Resolved by the removal: there is no cleanup requirement left to scope | Documentation |
| P2: the README's Windows flow could not start or authenticate (`README.md:72`) | A PowerShell 7 example generates a token, sets `FILESQUID_ADMIN_TOKEN` for the server launch, and reuses `$TOKEN` with `curl.exe`. The logs section's old `Get-Content filesquid-data/admin-token` line now points to that flow. | The same `FILESQUID_ADMIN_TOKEN` flow was run on Linux (below). PowerShell (`pwsh`) is not installed in this environment, so the PowerShell lines themselves were **not run**. |
| P3: lib.rs claimed the fallbacks are authorised | The crate docs now say routed operations are authenticated and authorised, and the 404 and 405 fallbacks are authentication-only. The code matches: the fallbacks call `auth::check_caller`, which only authenticates. | Existing `unknown_paths_and_wrong_methods_authenticate_before_routing` |
| P3: ADR 030 said "where the server runs" and wrote `<data dir>/admin-token` | The phrase is dropped, and the remedies use `$FILESQUID_DATA_DIR/admin-token` | Documentation |

### Red

```text
thread 'admin_token::tests::an_orphaned_temporary_file_does_not_block_recovery' panicked at crates/server/src/admin_token.rs:376:46:
orphan kept: Os { code: 2, kind: NotFound, message: "No such file or directory" }
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 4 filtered out; finished in 1.09s
```

### Green: round 4 raw output

`cargo test -p filesquid-api -p filesquid-server -p filesquid-desktop --locked` (exit code 0):

```text
     Running unittests src/lib.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
     Running tests/features.rs
87 scenarios (28 passed, 59 skipped)
     Running tests/unit/admin.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/auth.rs
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/contract.rs
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
     Running tests/unit/errors.rs
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/events.rs
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/unit/health.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/health_rules.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/journal.rs
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/metrics.rs
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/registry.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/routes.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/writer.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/api_protocol.rs
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running unittests src/lib.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s
     Running unittests src/main.rs
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/auth.rs
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s
     Running tests/integration/events.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/integration/health.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/monitor.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/static_ui.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/health.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

The `FILESQUID_ADMIN_TOKEN` flow from the Windows example, run on Linux against the real `filesquid-server` binary (on port 8094 to avoid clashing with other sessions, so these commands use 8094 rather than the README's 8083; no token file is written. Re-run in review round 5 so the commands and the output agree):

```text
$ curl http://127.0.0.1:8094/api/v1/health/live
{"status":"up"}
$ curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8094/api/v1/health
{"schema_version":1,"status":"ok","product":"filesquid"}
token file written: no
```

## Review round 6: Cubic full re-review of 31e8c0f (20 findings)

| Finding | Fix | Test |
|---|---|---|
| P2: the launch token was injected for the Vite dev origins in every build (`api_protocol.rs:109`) | `RELEASE_ORIGINS` (the app's own scheme origins) are trusted in every build; `DEV_ORIGINS` (127.0.0.1:5173 and :5174) only when `debug_assertions` is on. `app_origins()` feeds both the init script and CORS. | `release_builds_trust_only_the_apps_own_scheme_origins`, `dev_server_origins_are_trusted_only_in_debug_builds`; the init-script test checks the dev origins appear only in debug builds |
| P2: `FILESQUID_ADMIN_TOKEN` was accepted at any strength (`srs.md:27`) | A configured or stored token must be exactly 64 hexadecimal characters (256 bits), like a generated one. Otherwise the server refuses to start with an error that names the variable (or file) and the format, never the value. API-S-016, ADR 030 and the README say so; the README gives `openssl rand -hex 32` for Linux and macOS, and its PowerShell example already makes 32 random bytes as 64 hex characters. | `a_configured_token_must_have_the_generated_format`, `a_stored_token_that_lacks_the_generated_format_is_refused`; real binary below |
| P2: a non-Bearer `Authorization` header counted as missing and fell back to the subprotocol (`credentials.rs:28`) | `presented_credential` returns `Absent`, `Unusable` or `Bearer`. A present but unusable header is `invalid_credentials` (403) and never falls back to the subprotocol; only a request with neither carrier is `missing_credentials`. | `a_present_but_unusable_authorization_header_is_not_a_missing_one`, `an_unusable_header_never_falls_back_to_the_subprotocol`, `a_non_bearer_authorization_header_is_refused_as_invalid_credentials`, `an_unusable_header_on_a_websocket_upgrade_is_refused_even_with_a_valid_subprotocol` |
| P2: no denial was checked through the desktop URI forms (`routes.rs:32`) | Unauthenticated variant `answer_as(.., None)` | `denials_answer_the_same_through_every_host_uri_form` (every protected route and both fallbacks, same 401 through all three forms) |
| P2/P3: docs said "every route/operation" (walkthrough lines 24, 30, 35; `resources/srs.md:1012`; server `lib.rs:48`; api `lib.rs:76`) | All name `GET /api/v1/health/live` as the only public exception and say "every guarded call". SRS-F-056's deny-by-default bullet names it too. The server config doc says the token file is Unix-only. | Documentation |
| P3: ADR 030 Status had no API-F1.7 amendment | "Amended" entries for API-F1.7 (2026-10-17) and the server admin-token change (2026-10-18) | Documentation |
| P3: `auth/mod.rs` ADR link one level short | Six `../` | Link check in pre-commit |
| P3: `logging-enforce.md` missed host start and stop | Added | Documentation |
| P3: `LaunchToken` derived `Clone` | Removed; nothing cloned it | Compiles |
| P3: `Action::ALL` said "contract order" | "declaration order" | Documentation |
| P3: server `lib.rs:14` cited ADR 031 | Cites ADR 032 (hosting and privacy model) and states the reason | Documentation |
| P3: the SRS and walkthrough did not say WebSocket upgrades accept the Bearer header | API-S-004 and the walkthrough say non-browser clients can send the header on the upgrade, and browsers use the subprotocol because they cannot set headers | Documentation |
| P3: the server token-issue test did not check for the token | It now asserts the stored token never appears in the journaled `api.auth.token_issued` entries | `every_start_journals_the_administrator_token_issue` |
| P3: the walkthrough action list omitted `public` | Added | Documentation |

### Red: round 6 tests before the fixes

```text
error[E0432]: unresolved imports `filesquid_api::auth::Credential`, `filesquid_api::auth::presented_credential`
error: could not compile `filesquid-api` (test "unit_auth") due to 1 previous error
error[E0432]: unresolved imports `filesquid_desktop_lib::api_protocol::app_origins`, `filesquid_desktop_lib::api_protocol::DEV_ORIGINS`, `filesquid_desktop_lib::api_protocol::RELEASE_ORIGINS`
error: could not compile `filesquid-desktop` (test "integration_api_protocol") due to 1 previous error
---- a_configured_token_must_have_the_generated_format stdout ----
---- a_stored_token_that_lacks_the_generated_format_is_refused stdout ----
test result: FAILED. 16 passed; 2 failed (integration_auth)
```

### Green: round 6 raw output

`cargo test -p filesquid-api -p filesquid-server -p filesquid-desktop --locked` (exit code 0):

```text
     Running unittests src/lib.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
     Running tests/features.rs
87 scenarios (28 passed, 59 skipped)
     Running tests/unit/admin.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/auth.rs
test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/unit/contract.rs
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
     Running tests/unit/errors.rs
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/events.rs
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/unit/health.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/health_rules.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/journal.rs
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/metrics.rs
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/registry.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/routes.rs
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/writer.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/api_protocol.rs
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running unittests src/lib.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s
     Running unittests src/main.rs
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/auth.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s
     Running tests/integration/events.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/integration/health.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/monitor.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/static_ui.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/health.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

The README token-generation command and the format check, run against the real `filesquid-server` binary (port 8095):

```text
$ export FILESQUID_ADMIN_TOKEN=$(openssl rand -hex 32)
length: 64
"filesquid-server listening"
$ export FILESQUID_ADMIN_TOKEN=weak-secret-value
"cannot start API authorisation or the activity journal"
"FILESQUID_ADMIN_TOKEN must be 64 hexadecimal characters (256 bits), like a generated token; generate one with `openssl rand -hex 32`"
```

## Work left to other tasks

- **UI client (`@filesquid/ui`):** by owner decision a separate small pull request right after this one merges. `FileSquidClient` must send `Authorization: Bearer <token>` on every request (desktop: `window.__FILESQUID_LAUNCH_TOKEN__`; server: the administrator token until accounts exist) and offer `["filesquid.events.v1", "filesquid.bearer.<token>"]` on its WebSocket. It can probe `GET /api/v1/health/live` without a token and show `ApiError.reason` as a sign-in or permission state. The Playwright end-to-end suites (`FILESQUID_E2E=1`) need the same change.
- **DE-F1.1:** the desktop still forwards events to every webview with `app.emit`. A Tauri channel that delivers only to the app's own webviews belongs to that task, as do ranged reads.
- **API-F1.9:** accounts, sessions, groups and collection permissions as a new `Authenticator`, `Policy`, `Principal` and `Resource` variants. The guard does not change.

## Mocked Environment Variables

None. Tests use fixed in-test tokens (`s3cret`, `admin`, `auth-test-token` and similar) or tokens generated per test. The server run above used a token generated into a temporary `filesquid-data/`, which was deleted afterwards.
