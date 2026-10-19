# Architecture Decision Record - FileSquid

013-cargo-svelte-workspaces.md

## Title

Cargo workspaces plus a Svelte/Vite UI workspace under apps/; no BEAM

## Status

Accepted

Accepted: 2026-09-16T22:24:57Z. Revised: 2026-09-25T21:00:00Z. Revised: 2026-10-15T06:14:23Z. Amended by [ADR 028](./028-filesquid-ui-packages-and-panel-composition.md): each filesquid engine directory holds its own npm UI package under `ui/`.

Amended 2026-10-19 (owner decision of 2026-10-18): the Rust toolchain is pinned to an exact stable release in `apps/filesquid/rust-toolchain.toml` (`channel = "1.99.1"`), not the moving `stable` channel, because Rust 1.99.1 deprecated `AtomicUsize::fetch_update` and broke `clippy -D warnings` on every branch overnight. The version is bumped only in its own `build/` pull request that fixes the new lints; see the root [README](../../README.md#native-prerequisites-platform-agnostic). See the **Versions** note in the Decision.

Amended 2026-10-17 by [ADR 035](./035-macos-first-desktop-mvp.md): macOS is the first desktop target and the FileSquid MVP is desktop-only; Linux and Windows desktop follow the MVP.

Amended 2026-10-15 by [ADR 029](./029-filesquid-unified-product-and-distributions.md) and [ADR 031](./031-vanilla-typescript-ui-and-white-label-theming.md): one product-local Cargo workspace at `apps/filesquid/` with crates under `apps/filesquid/crates/`; UIs are Vite and TypeScript without Svelte (`apps/filesquid/ui/`, `apps/imagesquid/`). The per-engine `apps/filesquid-*/ui/` packages were never created and are superseded.

## Context / Requirement Reference

[ADR 001](./001-shared-rust-core-with-tauri-and-axum-hosts.md) forbids Phoenix/FastAPI as extra backends. Repository rules put applications in `apps/` with isolated manifests ([workspace boundaries](../../.agents/rules/workspace-boundaries.md)). The vision listed runtime versions and cloud frameworks as undecided. Inherited BEAM template files are not the product.

## Decision

Implementation lives under `apps/`: all Rust crates share **one Cargo workspace at `apps/filesquid/`** (crates are named `filesquid-*`, [ADR 029](./029-filesquid-unified-product-and-distributions.md)), and each UI package has its own npm manifest:

| Workspace | Contents |
|---|---|
| Rust library crate(s) | Portable contracts, private client, client engines, native processing ([ADR 001](./001-shared-rust-core-with-tauri-and-axum-hosts.md)); the client engines are **`filesquid-ocfl`** at `apps/filesquid/crates/ocfl/` ([ADR 016](./016-rust-ocfl-library.md)) and **`filesquid-metadata`** at `apps/filesquid/crates/metadata/` ([ADR 024](./024-filesquid-metadata-engine.md)), built for native and `wasm32-unknown-unknown` ([ADR 025](./025-client-side-filesquid-engines.md)) |
| Tauri desktop | Desktop host + bundled UI |
| Axum service | `filesquid-server` and the `*.filesquid.tech` service ([ADR 032](./032-hosting-and-privacy-model.md)) |
| Vite and TypeScript UI | `@filesquid/ui` at `apps/filesquid/ui/` and distribution UIs such as `apps/imagesquid/`, without Svelte ([ADR 031](./031-vanilla-typescript-ui-and-white-label-theming.md)) |
| ~~Engine UI packages~~ | Superseded by [ADR 031](./031-vanilla-typescript-ui-and-white-label-theming.md); the per-engine Svelte packages were never created |

No `mix.exs`, no Phoenix, no Python application backend, no language manifest at the repository root. Node.js exists **only to build and test the UI**, not as a production library backend. UI tests still run with Node; they do not make Node the application runtime.

**Versions:** current stable **Rust** and **TypeScript** at first implementation, pinned in lockfiles (`Cargo.lock`, UI lockfile). This ADR does not freeze patch numbers. Minimum: a stable Rust toolchain that builds Tauri 2.

> **Amendment 2026-10-19 (owner decision of 2026-10-18):** "current stable Rust" is now an exact stable release pinned in `apps/filesquid/rust-toolchain.toml` (`channel = "1.99.1"`, components `rustfmt` and `clippy`, target `wasm32-unknown-unknown`). A nightly toolchain is still never required ([U-37](../product-vision.md#one-api-hosts-and-distributions)). Moving to a newer stable release is its own `build/` pull request that fixes any new lints and passes the full pre-commit scope.

Linux and the independent web are CI build-and-test targets from the start ([FE-017](../product-vision.md#cross-platform-access), [U-34](../product-vision.md#one-api-hosts-and-distributions)). CI coverage does not set the desktop release order: amended by [ADR 035](./035-macos-first-desktop-mvp.md), macOS is the first desktop install target, and the Linux and Windows desktop installers follow the MVP.

## Rationale

Matches the accepted language split and keeps the template BEAM stack from becoming the product.

## Assumptions

- First scaffolding will create these workspaces; they do not exist yet.
- Observability compose remains optional hosted/dev infrastructure, not required to run the desktop app ([ADR 014](./014-stdout-observability.md)).

## Alternatives Considered

- **Umbrella Mix/Phoenix:** Rejected.
- **Monorepo root package.json / Cargo.toml:** Violates workspace isolation.
- **Pin exact Axum 0.8.9 in this ADR:** Lockfiles own patch versions.

## Consequences / Implications

Easier: exportable apps. Harder: first scaffold must not dump files at repo root. CI images need Rust + Node + OS webview/native deps.

## Related Decisions / Notes

- [ADR 001](./001-shared-rust-core-with-tauri-and-axum-hosts.md), [ADR 002](./002-tauri2-desktop-shell.md), [ADR 003](./003-svelte-client-rendered-ui.md), [ADR 004](./004-axum-hosted-ciphertext-mailbox.md), [ADR 016](./016-rust-ocfl-library.md)
