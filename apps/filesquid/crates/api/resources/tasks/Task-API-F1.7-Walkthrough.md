# Task API-F1.7 Walkthrough: Authentication and authorisation

Module task [API-F1.7](../plan.md) of global Task FS-1.4 ([implementation-plan.md](../../../../../../resources/implementation-plan.md)). Branch `feat/api-f1.7-authorisation`. Builds on [API-F1.2](./Task-API-F1.2-Walkthrough.md).

## API-S-004: Every call is authenticated and authorised before files, SQL or keys are touched: per-launch token on desktop; accounts, groups and collection-level permissions on the server

> Every call MUST be authenticated and authorised before files, SQL or keys are touched: per-launch token on desktop; accounts, groups and collection-level permissions on the server. Access MUST be deny-by-default ([API-S-004](../srs.md), which now lists the granular constraints below).

Parents: [F-056 Single FileSquid API](../../../../../../resources/prd.md#f-056-single-filesquid-api), [SRS-F-056 Single FileSquid API](../../../../../../resources/srs.md#srs-f-056-single-filesquid-api) ("Every API call MUST be authorised before files, SQL or keys are touched. Access MUST be deny-by-default."), [F-057 Desktop and server hosts](../../../../../../resources/prd.md#f-057-desktop-and-server-hosts), [NFR-004 Security and privacy](../../../../../../resources/prd.md#nfr-004-security-and-privacy), FileSquid [U-06](../../../../../../resources/product-vision.md#platform-ownership-and-access), [U-18](../../../../../../resources/product-vision.md#one-api-hosts-and-distributions), [T-053](../../../../../../resources/product-vision.md#api-and-hosts), [ADR 030](../../../../../../resources/architecture/030-filesquid-api-and-transports.md) Decision 5.

This task also keeps [API-S-002](../srs.md) (versioned contract and drift tests), [API-S-006](../srs.md) (no storage I/O in the API crate) and [API-S-009](../srs.md) (errors the UI can show) for everything it adds.

## What changed

- **`auth` module** ([`src/auth/`](../../src/auth/mod.rs)):
  - `Principal` (`Owner`, `Administrator`), `Action` (`health.read`, `contract.read`, `events.subscribe`, `monitor.read`) and `Resource` (`Host`), all `#[non_exhaustive]`.
  - The API-F1.9 seams: the `Authenticator` trait (token to principal) and the `Policy` trait (principal, action and resource to a yes or no). `TokenAuthenticator` compares in constant time. `HostPolicy` grants the owner and the administrator every action on the host, and its exhaustive match makes a new principal a compile error until its permissions are decided. `DenyAll` grants nothing.
  - `Authoriser`: `deny_all()`, `desktop_owner(&LaunchToken)`, `server_administrator(token)` and `new(authenticator, policy)`. `authenticate` and `authorise` are pure.
  - `LaunchToken` and `generate_token`: 256 random bits as 64 hex characters, with redacted `Debug`.
  - `presented_token`: `Authorization: Bearer` (case-insensitive scheme), or on a WebSocket upgrade only, the `filesquid.bearer.<token>` subprotocol offered with `filesquid.events.v1`.
  - The guard: `require` middleware with a `Guard { state, action }`, plus `authorise_call` and `check_caller`. A refusal logs one stdout line (INFO, target `filesquid::auth`, never journaled twice) and journals `api.auth.denied` through the bounded queue on a blocking task. The record holds the actor (`anonymous` or the principal), the reason as error code and the action as target. `token_issued_entry` builds `api.auth.token_issued`.
- **Router** ([`src/routes.rs`](../../src/routes.rs)): every route-table entry now names its `Action`, and `router()` wraps every entry in `require`. A route cannot be added without a guard. The 404 and 405 fallbacks answer only an authenticated caller. `route_actions()` lists path and action. `ApiState::install_authoriser` installs the authoriser once; until it runs, every call is refused.
- **Typed denial** ([`src/error.rs`](../../src/error.rs)): `DenialReason` (`missing_credentials` gives 401 `unauthorized`; `invalid_credentials` and `not_permitted` give 403 `forbidden`) and `ApiError::denied`. `ApiError` gains an optional `reason` member, omitted for other errors. Messages never repeat the credential.
- **Admin routes and monitor**: the interim `AdminAuth` is removed. Admin routes no longer check credentials themselves; their guard does (`monitor.read`). `MonitorConfig::new` and `HostMonitor::open` lose the `auth` argument. The admin-token file I/O (`load_or_create_token`) moved from the API crate to `filesquid-server` ([API-S-006](../srs.md)). An unreadable token file is now an error instead of being silently replaced.
- **Contract `0.5.0`** ([`openapi.json`](../../openapi.json)):
  - A global `security: [{bearerAuth: []}]`. The per-operation optional security (`{}`) is removed.
  - 401 and 403 on every operation, the event channel included, and `x-filesquid-action` on every operation.
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
  - [API-S-004](../srs.md) lists the granular constraints.
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

## Work left to other tasks

- **UI client (`@filesquid/ui`, UI engineer):** `FileSquidClient` must send `Authorization: Bearer <token>` on every request, not only on `/api/v1/admin/*`. On the desktop it reads `window.__FILESQUID_LAUNCH_TOKEN__`. On the server it uses the administrator token until accounts exist. Its WebSocket must offer `["filesquid.events.v1", "filesquid.bearer.<token>"]`. It can show `ApiError.reason` (now in the generated types) as a sign-in or permission state. Until this lands, the UI's anonymous `health()` call and server WebSocket are refused. The Playwright end-to-end suites (`FILESQUID_E2E=1`) that call the real server need the same change.
- **DE-F1.1:** the desktop still forwards events to every webview with `app.emit`. A Tauri channel that delivers only to the app's own webviews belongs to that task, as do ranged reads.
- **API-F1.9:** accounts, sessions, groups and collection permissions as a new `Authenticator`, `Policy`, `Principal` and `Resource` variants. The guard does not change.

## Mocked Environment Variables

None. Tests use fixed in-test tokens (`s3cret`, `admin`, `auth-test-token` and similar) or tokens generated per test. The server run above used a token generated into a temporary `filesquid-data/`, which was deleted afterwards.
