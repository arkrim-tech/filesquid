# Task build-pin-rust-toolchain: Walkthrough

## Owner-approved infrastructure change (2026-10-18): pin the Rust toolchain to an exact stable release

No PRD or SRS requirement. `apps/filesquid/rust-toolchain.toml` named the moving `stable` channel, so every branch used whatever stable release rustup installed that day. Stable Rust 1.99.1 deprecated `AtomicUsize::fetch_update`, and `cargo clippy -- -D warnings` failed on `main` and every open branch overnight. The owner decided to pin an exact version, so a new stable release reaches a branch only through a pull request that bumps the pin and fixes the new lints. A nightly toolchain is still never required ([U-37](../product-vision.md#one-api-hosts-and-distributions)). No gate changes what it checks.

## What changed

| File | Change |
|---|---|
| [`apps/filesquid/rust-toolchain.toml`](../../apps/filesquid/rust-toolchain.toml) | `channel = "1.99.1"` instead of `"stable"`. Components `rustfmt` and `clippy` and the `wasm32-unknown-unknown` target are unchanged. |
| [README](../../README.md#native-prerequisites-platform-agnostic) | Native prerequisites name rustup 1.28 or newer and the pinned 1.99.1, and say the toolchain file installs `rustfmt`, `clippy` and the wasm32 target itself (the old text asked for a manual `rustup target add`). New **Bumping the Rust toolchain** note: change the pin in its own `build/` pull request, fix the new lints there, run `FULL=1 bash scripts/pre-commit.sh`. |
| [ADR 013](../architecture/013-cargo-svelte-workspaces.md) | Dated amendment (status and the **Versions** paragraph): "current stable Rust" is now an exact stable release pinned in `rust-toolchain.toml`. The decision text is not rewritten. |
| [ADR 019](../architecture/019-coding-style-and-lint.md) | Dated amendment: "stable Rust" means the pinned release; lint commands unchanged. |
| [ADR 039](../architecture/039-cargo-hakari-workspace-hack.md) | Dated amendment: its Context records the toolchain as measured on 2026-10-18 (channel `stable`); the pin now replaces the channel. Decision unchanged. |
| [SRS](../srs.md#current-verified-stack-scaffold) | Current verified stack: Rust 1.99.1 pinned (was "Rust stable 1.98.1, channel `stable`"). |
| [init-rust-js skill](../../.agents/skills/init-rust-js/SKILL-init-rust-js.md) | New workspaces pin an exact stable release, not `channel = "stable"`. |

Searched and left unchanged:

- [`scripts/setup-dev-environment.sh`](../../scripts/setup-dev-environment.sh) already runs `rustup toolchain install` from `apps/filesquid/` without arguments, so it installs whatever the file pins; its messages already say "pinned".
- [`scripts/verify_tests.sh`](../../scripts/verify_tests.sh) queries targets from the workspace so the file picks the toolchain; [`scripts/changed_scope.sh`](../../scripts/changed_scope.sh) already classifies a `rust-toolchain` change as `full`.
- The `.claude/agents/` role files, `apps/README.md` and `apps/filesquid/` READMEs do not describe how the toolchain is chosen.
- [Product vision U-37](../product-vision.md#one-api-hosts-and-distributions) ("current stable Rust is preferred, nightly MUST NOT be required") and the PRD's no-nightly constraint still hold: 1.99.1 is a stable release.
- Earlier walkthroughs record what was true when they ran and stay as history.

## Verification

Installed and pinned toolchain, from `apps/filesquid/` (rustup downloaded and resolved the pinned name with no network trouble):

```text
$ rustc --version
rustc 1.99.1 (58cf8f9c3 2026-10-13)
$ cargo --version
cargo 1.99.1 (5f94df478 2026-08-27)
$ cargo clippy --version
clippy 0.1.99 (58cf8f9c35 2026-10-13)
$ rustup show active-toolchain
1.99.1-x86_64-unknown-linux-gnu (overridden by '<repo>/apps/filesquid/rust-toolchain.toml')
$ rustup target list --installed
wasm32-unknown-unknown
x86_64-unknown-linux-gnu
```

Change scope against `origin/main`:

```text
$ bash scripts/changed_scope.sh
changed_scope: full (apps/filesquid/rust-toolchain.toml changed)
full
```

Full pre-commit hook on the head commit (no Clippy finding on 1.99.1: `main` already carries the `fetch_update` fix in `filesquid-api` `events.rs`):

```text
$ FULL=1 bash scripts/pre-commit.sh   # step summary lines
changed_scope: full (FULL=1 is set)
✅ Branch Validation passed.
✅ ./apps/filesquid/ui uniquely bound to 5173
✅ ./apps/filesquid/ui uniquely bound to 4173
✅ ./apps/imagesquid uniquely bound to 5174
✅ ./apps/imagesquid uniquely bound to 4174
✅ filesquid-server uniquely bound to 8083
✅ Document Sync Validation passed.
✅ Traceability Validation passed: 95 PRD requirements, 82 resolved SRS references.
✅ Portable Documentation validation passed.
✅ Workspace Boundary validation passed.
✅ Logging rule validation passed (no relevant staged files).
✅ ./apps/filesquid is properly formatted.
✅ ./apps/filesquid/ui is properly formatted.
✅ ./apps/imagesquid is properly formatted.
✅ Format validation passed.
✅ ADR structure (ISO 42010) passed.
✅ PRD allocation passed.
✅ Plan traceability passed.
✅ Relative links and anchors passed.
✅ FileSquid and ImageSquid document boundary passed.
✅ MVP queue and merge driver tests passed.
✅ MVP progress bar passed.
✅ Task cleanup passed.
✅ Documentation validation passed.
✅ Change scope validation passed (166 assertions).
✅ Clippy passed (11s).
✅ Workspace tests passed (81s).
✅ Module crate alone: filesquid-core passed (7s).
✅ Module crate alone: filesquid-ocfl passed (35s).
✅ Module crate alone: filesquid-metadata passed (1s).
✅ Module crate alone: filesquid-c2pa passed (10s).
✅ Module crate alone: filesquid-api passed (17s).
✅ Module crate alone: filesquid-native passed (2s).
✅ Clippy (filesquid-metadata, all features) passed (2s).
✅ Tests (filesquid-metadata, all features) passed (8s).
✅ wasm32 build (filesquid-metadata, default features off) passed (0s).
✅ wasm32 build (filesquid-metadata, ocfl feature) passed (2s).
✅ Rustdoc passed (6s).
✅ Clippy (desktop, test-instrumented) passed (5s).
✅ Desktop tests (test-instrumented) passed (23s).
✅ ESLint passed (3s).
✅ TypeScript passed (4s).
✅ Vitest passed (12s).
✅ ESLint passed (2s).
✅ TypeScript passed (3s).
✅ Vitest passed (3s).
✅ Lint and test validation passed (test scope: full).
✅ All Pre-Commit Hooks Passed! Test scope: full (259s)
```
