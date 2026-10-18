# Architecture Decision Record - FileSquid

017-test-runners-and-reporting.md

## Title

cargo test, Vitest, Playwright, and Allure under each app

## Status

Accepted

Accepted: 2026-09-22T07:00:00Z. Amended: 2026-10-18T05:36:00Z, in Task DE-F1.1, to record the first desktop native-window tests and their test-only dependencies.

## Context / Requirement Reference

AGENTS.md requires unit, UI, integration, performance, and BDD tests with structured reports. Scaffolding must not dump reports at the repository root ([artifact-isolation.md](../../.agents/rules/artifact-isolation.md)).

## Decision

- Rust crates use `cargo test` (optionally nextest later) with `[[test]]` paths under `tests/unit/` and `tests/integration/`.
- Frontend uses Vitest (jsdom) for unit/UI and Playwright against `npm run build` + preview for e2e.
- Desktop native e2e is WebdriverIO embedded (`@wdio/tauri-service`) under `apps/filesquid/crates/desktop/tests/e2e/` when first window tests ship; not required to compile the scaffold.
  - Recorded 2026-10-18 (Task DE-F1.1): the first window tests shipped. `tests/e2e/` is its own small npm package (test tooling only) pinning `@wdio/tauri-service` 1.5.0 and WebdriverIO 9.32.0, with the `embedded` driver provider. The desktop crate's `test-webdriver` Cargo feature adds `tauri-plugin-wdio-webdriver` 1.5.0 (the embedded WebDriver server, listening on `127.0.0.1`) and a 100 MB range fixture; shipping builds never enable it, so they open no TCP listener ([ADR 002](./002-tauri2-desktop-shell.md), [ADR 035](./035-macos-first-desktop-mvp.md)). On Linux without a display the run uses Xvfb (`xvfb-run`). The JUnit report goes to `apps/filesquid/crates/desktop/tests/allure-results/wdio/`, cleared before each run. The pre-commit hook runs it only with `FILESQUID_DESKTOP_E2E=1`.
- Gherkin lives under each app `tests/features/`. The first runner is `cargo test` / Playwright; the `cucumber` crate is added when the first Rust Gherkin scenario is implemented.
- Allure/JUnit output stays in each app’s `tests/allure-results/`.
- Criterion benches and k6 against Axum are later performance work ([ADR 007](./007-durable-jobs-and-interactive-performance.md)).

## Rationale

Matches the test-folder taxonomy of the TDD rule ([tdd-enforce.md](../../.agents/rules/tdd-enforce.md)) without claiming FileSquid U-11 measurements.

## Assumptions

- No application features beyond health/ping exist yet.
- Playwright browsers are installed per developer machine (`npx playwright install`).

## Alternatives Considered

- **Jest + Locust:** Rejected; Vitest matches Vite; k6 is the HTTP load candidate.
- **Root-level Allure:** Violates artifact isolation.

## Consequences / Implications

Easier: each app owns its reports. Harder: CI must `cd` into each crate.

## Related Decisions / Notes

- [ADR 013](./013-cargo-svelte-workspaces.md), AGENTS.md Test-driven development.
- [ADR 030](./030-filesquid-api-and-transports.md) S1 results: the packaged-window `Range` read is the first native-window test.
