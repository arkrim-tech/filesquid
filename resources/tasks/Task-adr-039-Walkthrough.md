# Task ADR-039 Walkthrough: cargo-hakari workspace-hack for the pre-commit Rust steps

Documentation-only task on branch `docs/adr-039-cargo-hakari-workspace-hack`. TDD-ENFORCE: Not applicable - documentation update. No crate, Cargo file or script is added or changed.

## Owner request (2026-10-18)

Record the decision to unify Rust dependency feature sets across the `apps/filesquid` Cargo workspace with a cargo-hakari workspace-hack crate, to cut rebuilds in the pre-commit hook, with a measurement gate before the crate is kept.

## Changes

- [ADR 039](../architecture/039-cargo-hakari-workspace-hack.md) (new): the generated crate `filesquid-workspace-hack`, cargo-hakari pinned to 0.9.39, `.config/hakari.toml` in `apps/filesquid` with four platforms. The hack is a **dev-dependency only** (workspace-dotted line), so runtime, WASM and published graphs and every isolation guard stay unchanged. The hook gains a drift gate (`cargo hakari generate --diff` plus a dev-only placement check, instead of `manage-deps --dry-run`). The ADR 029 alone check becomes `cargo check -p <crate> --lib` in the full scope, plus a stand-alone test run and `cargo publish --dry-run` before publication. The crate is kept only if the full-scope Rust steps get at least 20% faster, cold and warm, and a `rust:filesquid-ocfl` run is not slower.
- [ADR index](../architecture/README.md): row 039.
- [SRS](../srs.md), "Current verified stack": one bullet saying the tooling is decided and not yet installed.

## Facts checked (2026-10-18, UTC)

| Fact | How it was checked |
|---|---|
| cargo-hakari 0.9.39 is the newest stable release, published 2026-09-19T02:24:08Z, needs Rust 1.91 | crates.io API `crates/cargo-hakari` and `crates/cargo-hakari/versions` |
| `resolver.feature-unification` is unstable on cargo 1.97.0 | a scratch crate with `[resolver] feature-unification = "workspace"` in `.cargo/config.toml`: ``warning: ignoring `resolver.feature-unification` without `-Zfeature-unification` `` |
| LLD is the default linker | `readelf -p .comment` of a scratch binary: `Linker: LLD 22.1.6` |
| 4 cores; `rustc 1.97.0`, `cargo 1.97.0` | `nproc`, `rustc --version`, `cargo --version` |
| Incremental and debuginfo off | [`scripts/lib/cargo_shared.sh`](../../scripts/lib/cargo_shared.sh) exports `CARGO_INCREMENTAL=0` and `CARGO_PROFILE_DEV_DEBUG=0` |
| `filesquid-api` resolves Tokio `net`, `mio` and `socket2` | `cargo tree -p filesquid-api -e features,normal -i tokio` |
| `cargo tree` without `dev` edges does not resolve dev-dependency features | `cargo tree -p filesquid-api -e normal,build,features` has no Tokio `rt-multi-thread` (0 lines); with `dev` added, 1 line |
| `manage-deps` treats a dev-only link as missing and adds a normal one; traversal- and final-excluded members get the link removed | source of `hakari` 0.18.0, `cli_ops/manage_deps.rs` and `Hakari::is_excluded` |
| hakari computes each member with no, default and all features, with and without dev-dependencies | source of `hakari` 0.18.0, `hakari.rs` |
| `cargo hakari publish -p <crate>` removes the hack before publishing; `cargo hakari disable` empties the hack | cargo-hakari 0.9.39 `command.rs` and `docs/publishing.rs` |

## Corrections to the brief

- The brief said each crate already has one test binary. It does not: test targets are explicit `[[test]]` entries, for example 16 in `filesquid-api` and 22 in `filesquid-c2pa`. The ADR states the real layout and lists fewer test binaries as a separate lever, not decided here.
- The brief proposed a hook gate with `cargo hakari manage-deps --dry-run`. With a dev-only hack that command would always ask for a normal dependency, so the ADR checks dev-only placement directly.
- The brief allowed a normal dependency with host crates excluded. `filesquid-api` already brings Tokio-net through Axum, so any normal hack shared with `filesquid-ocfl` or `filesquid-metadata` would give them Tokio-net. The `filesquid-core` manifest guard, the `filesquid-c2pa` graph guard (no other `filesquid-` package, no Tokio) and the `filesquid-ocfl` `path = "../` step would also fail. Dev-only placement avoids all of this without editing a guard.

## Open questions for the owner

1. The SRS "Current verified stack" says Rust stable 1.98.1; the containers run 1.97.0 (`rust-toolchain.toml` pins only the channel). Not changed here, following sync-logic.md.
2. [`scripts/docs/validate-adrs.mjs`](../../scripts/docs/validate-adrs.mjs) checks ADRs 001 to 033 only. ADRs 034 to 039 are not structure-checked. Not changed here, because this task changes no script.

## Sync Execution Report

ADR 039 → [ADR index](../architecture/README.md) → [SRS](../srs.md) "Current verified stack" (one bullet: decided, not installed). PRD: unchanged; its Technology Stack lists product stack ADRs, not build tooling (ADR 019 is not listed either). Rules ([local-verification.md](../../.agents/rules/local-verification.md), [src-isolation.md](../../.agents/rules/src-isolation.md)), the root README and `scripts/` change only with the implementation, if the measurement gate passes.

## Verification

Run with `CARGO_TARGET_DIR` on the shared target, `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0` and `TMPDIR` on tmpfs, after `npm ci` in `apps/filesquid/ui` and `apps/imagesquid`.

`bash scripts/pre-commit.sh` (docs scope) exit 0: `✅ All Pre-Commit Hooks Passed! Test scope: docs (14s)`.

`FULL=1 bash scripts/pre-commit.sh` on commit `e572fd3` (2026-10-18, UTC) exit 0. Excerpt of the step lines, copied from the output. The per-crate times are a warm-target baseline without the hack; they are not the measurement Decision 7 asks for.

```text
ℹ️  Test scope: full
✅ Clippy passed (14s).
✅ Workspace tests passed (72s).
✅ Module crate alone: filesquid-core passed (3s).
✅ Module crate alone: filesquid-ocfl passed (49s).
✅ Module crate alone: filesquid-metadata passed (14s).
✅ Module crate alone: filesquid-c2pa passed (34s).
✅ Module crate alone: filesquid-api passed (6s).
✅ Module crate alone: filesquid-native passed (21s).
✅ Rustdoc passed (13s).
✅ Clippy (desktop, test-instrumented) passed (13s).
✅ Desktop tests (test-instrumented) passed (32s).
✅ Lint and test validation passed (test scope: full).
✅ All Pre-Commit Hooks Passed! Test scope: full (325s)
```
