# Task fix-subscriber-slot-fetch-update Walkthrough: Claim subscriber slots without the deprecated `fetch_update`

Fix branch `fix/api-subscriber-slot-deprecated-fetch-update`. Not a plan task: it unblocks every pull request whose change scope includes Rust, because the workspace Clippy gate failed on the current stable toolchain.

## API-S-003: The router MUST be transport-agnostic and mounted unchanged by the desktop host (custom URI scheme) and `filesquid-server` (HTTP), with one bounded event channel for job progress and change notifications

The constraint this fix touches, quoted from [API-S-003](../srs.md): "at most 64 client subscriptions are open at once (the next is refused with a retryable `unavailable` error)". It is enforced by `SubscriberSlot::claim` in [`src/events.rs`](../../src/events.rs) under `MAX_SUBSCRIBERS` (64). The requirement text does not change.

## Cause: toolchain drift

[`rust-toolchain.toml`](../../../../rust-toolchain.toml) pins only the `stable` channel. The installed stable, rustc 1.99.1 (2026-10-13), deprecates `AtomicUsize::fetch_update` ("renamed to `try_update` for consistency"), and `cargo clippy --workspace --all-targets --locked -- -D warnings` turns the deprecation into an error:

```text
error: use of deprecated method `std::sync::atomic::Atomic::<usize>::fetch_update`: renamed to `try_update` for consistency
   --> crates/api/src/events.rs:217:14
    |
217 |         open.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
    |              ^^^^^^^^^^^^
    |
    = note: `-D deprecated` implied by `-D warnings`
error: could not compile `filesquid-api` (lib) due to 1 previous error
```

It was the only `fetch_update` in the workspace. Renaming the call to `try_update` would not build on an older stable, where that method does not exist, and `#[allow(deprecated)]` would hide the warning instead of fixing it.

## What changed

- **`SubscriberSlot::claim`** ([`src/events.rs`](../../src/events.rs)) now reserves, then checks, with no loop: `fetch_add(1)` returns the previous count; a caller that saw `MAX_SUBSCRIBERS` or more gives the place back with `fetch_sub(1)` and is refused. Both methods have been stable since Rust 1.0, so the code builds on older and current stable alike.
- **Guarantee:** each caller sees a distinct previous count, so only callers that saw fewer than `MAX_SUBSCRIBERS` are granted, and granted slots never exceed the cap.
- **Trade-off, documented in the rustdoc:** between its `fetch_add` and `fetch_sub`, a refused caller holds the counter above the granted number. A concurrent caller that would just have fit can then be refused as well, with the same retryable `unavailable` answer as at the cap. `EventBus::open_subscriptions` can briefly read above the cap for the same reason, and its rustdoc now says so. The old rustdoc claimed that check and increment were one atomic step; that text is replaced.
- **New unit test** `concurrent_claims_and_releases_never_exceed_the_cap_and_free_every_slot` ([`tests/unit/events.rs`](../../tests/unit/events.rs)): 8 threads, each running 200 rounds of claiming a batch of 12 subscriptions, holding them, then dropping them. The test records the most subscriptions ever held at once and asserts that it never exceeds `MAX_SUBSCRIBERS`. It then checks that every slot and receiver is released, that all 64 slots can be claimed again and that the 65th is refused. The existing tests (`concurrent_subscribers_never_exceed_the_cap`, `the_subscriber_cap_refuses_one_more_with_a_retryable_error`, `dropping_a_subscription_frees_its_slot_and_its_receiver`) still pass unchanged.

## Documentation sync

`scripts/verify_doc_sync.sh` checks only `.ex` and `.exs` sources against Markdown changes, so a Rust-only change does not trigger it. This walkthrough is the only documentation change needed: the SRS, PRD, ADRs and BDD features describe the cap, not how it is counted, and none of them change.

## Test results

Clippy for the crate after the fix:

```text
$ cargo clippy -p filesquid-api --all-targets --locked -- -D warnings
    Finished `dev` profile [unoptimized] target(s) in 0.24s
```

The event unit tests:

```text
$ cargo test -p filesquid-api --test unit_events --locked
     Running tests/unit/events.rs (unit_events)

running 14 tests
test a_resync_hint_carries_only_the_missed_count ... ok
test a_closed_bus_ends_the_subscription ... ok
test a_subscriber_exactly_one_buffer_behind_loses_nothing ... ok
test an_event_one_byte_over_the_size_cap_becomes_a_resync_hint ... ok
test concurrent_subscribers_never_exceed_the_cap ... ok
test dropping_a_subscription_frees_its_slot_and_its_receiver ... ok
test one_event_past_the_buffer_becomes_a_resync_hint_then_the_newest_events ... ok
test publishing_without_subscribers_is_not_an_error ... ok
test subscribers_receive_published_events ... ok
test the_documented_limits_have_their_declared_values ... ok
test the_subscriber_cap_refuses_one_more_with_a_retryable_error ... ok
test concurrent_claims_and_releases_never_exceed_the_cap_and_free_every_slot ... ok
test an_event_at_the_size_cap_is_delivered_unchanged ... ok
test a_slow_subscriber_cannot_block_publishers_or_grow_memory ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

The whole crate (`cargo test -p filesquid-api --locked`): every test binary passed (unit tests in `src/lib.rs` 7, `unit_admin` 6, `unit_auth` 34, `unit_contract` 28, `unit_errors` 11, `unit_events` 14, `unit_health` 2, `unit_health_rules` 7, `unit_host` 31, `unit_isolation` 13, `unit_library` 30, `unit_library_queries` 12, `unit_metrics` 1, `unit_registry` 5, `unit_replica` 48, `unit_routes` 4, `unit_writer` 5, and the cucumber `features` runner).

The full hook on the head commit, `FULL=1 bash scripts/pre-commit.sh`, is recorded in the pull request with its `Test scope: full` summary line.

## Mocked Environment Variables

None.
