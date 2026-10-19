# Task build-pin-rust-toolchain: Walkthrough

## Owner-approved infrastructure change (2026-10-18): pin the Rust toolchain to an exact stable release

No PRD or SRS requirement. `apps/filesquid/rust-toolchain.toml` named the moving `stable` channel, so every branch used whatever stable release rustup installed that day. Stable Rust 1.99.1 deprecated `AtomicUsize::fetch_update`, and `cargo clippy -- -D warnings` failed on `main` and every open branch overnight. The owner decided to pin an exact version, so a new stable release reaches a branch only through a pull request that bumps the pin and fixes the new lints. A nightly toolchain is still never required ([U-37](../product-vision.md#one-api-hosts-and-distributions)). No gate changes what it checks.

## What changed

| File | Change |
|---|---|
| [`apps/filesquid/rust-toolchain.toml`](../../apps/filesquid/rust-toolchain.toml) | `channel = "1.99.1"` instead of `"stable"`. Components `rustfmt` and `clippy` and the `wasm32-unknown-unknown` target are unchanged. |
| [README](../../README.md#native-prerequisites-platform-agnostic) | Native prerequisites name rustup 1.28.1 or newer (1.28.0 does not auto-install a toolchain) and the pinned 1.99.1, and say the toolchain file installs `rustfmt`, `clippy` and the wasm32 target itself (the old text asked for a manual `rustup target add`). Installing ahead of time runs `rustup toolchain install`, `rustup component add rustfmt clippy` and `rustup target add wasm32-unknown-unknown` from `apps/filesquid/`, as the setup script does, because some rustup versions skip the components the file declares. The C2PA wasm32 paragraph now gives `rustup target add` only as the fallback when rustup bypasses the toolchain file. New **Bumping the Rust toolchain** note: change the pin in its own `build/` pull request, fix the new lints there, run `FULL=1 bash scripts/pre-commit.sh`. |
| [ADR 013](../architecture/013-cargo-svelte-workspaces.md) | Dated amendment (status and the **Versions** paragraph): "current stable Rust" is now an exact stable release pinned in `rust-toolchain.toml`. The decision text is not rewritten. |
| [ADR 019](../architecture/019-coding-style-and-lint.md) | Dated amendment: "stable Rust" means the pinned release; lint commands unchanged. |
| [ADR 039](../architecture/039-cargo-hakari-workspace-hack.md) | Dated amendment: its Context records the toolchain as measured on 2026-10-18 (channel `stable`); the pin now replaces the channel. Decision unchanged. |
| [SRS](../srs.md#current-verified-stack-scaffold) | Current verified stack: Rust 1.99.1 pinned (was "Rust stable 1.98.1, channel `stable`"). |
| [C2PA README](../../apps/filesquid/crates/c2pa/README.md), [OCFL README](../../apps/filesquid/crates/ocfl/README.md) | Same: the pinned toolchain file installs the wasm32 target; `rustup target add` is the fallback when rustup bypasses the file. Documentation only. |
| [Rust briefing](../references/rust-language-dev-briefing.md) | Everyday commands run `rustup toolchain install` from `apps/filesquid/` (the pinned release) instead of `rustup toolchain install stable`, then add `rustfmt`, `clippy` and the wasm32 target explicitly. |
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

Full pre-commit hook on the head commit (`FULL=1 bash scripts/pre-commit.sh`, the same 151-second run the PR description cites). There is no Clippy finding on 1.99.1, because `main` already carries the `fetch_update` fix in `filesquid-api` `events.rs`. Below are the hook's own lines, verbatim and in order, with local paths shortened to `<repo>`. Left out are the 8,036 non-blank lines that cargo, cucumber, Vitest and ESLint print in between (compiler progress, per-test and per-scenario lines); each tool's verdict is the hook line that follows its output.

```text
$ FULL=1 bash scripts/pre-commit.sh
changed_scope: full (FULL=1 is set)
========================================
Running Pre-Commit Hooks... (test scope: full)
========================================
✅ Branch Validation passed.
Starting Port Isolation Validation...
🔍 Validating ./apps/filesquid/ui (Vite)...
✅ ./apps/filesquid/ui uniquely bound to 5173
✅ ./apps/filesquid/ui uniquely bound to 4173
🔍 Validating ./apps/imagesquid (Vite)...
✅ ./apps/imagesquid uniquely bound to 5174
✅ ./apps/imagesquid uniquely bound to 4174
🔍 Validating filesquid-server (default bind)...
✅ filesquid-server uniquely bound to 8083
🚀 VALIDATION PASSED: All modules maintain strict port isolation.
Starting Document Sync Validation...
✅ Document Sync Validation passed.
Starting PRD/SRS Traceability Validation...
✅ Traceability Validation passed: 95 PRD requirements, 82 resolved SRS references.
Starting Portable Documentation Validation...
✅ Portable Documentation validation passed.
Starting Workspace Boundary Validation...
✅ Workspace Boundary validation passed.
Starting logging rule validation...
✅ Logging rule validation passed (no relevant staged files).
Starting Format Validation...
🔍 Validating Rust formatting in ./apps/filesquid...
✅ ./apps/filesquid is properly formatted.
🔍 Validating Prettier formatting in ./apps/filesquid/ui...
✅ ./apps/filesquid/ui is properly formatted.
🔍 Validating Prettier formatting in ./apps/imagesquid...
✅ ./apps/imagesquid is properly formatted.
✅ Format validation passed.
Starting Documentation Validation...
🔍 ADR structure (ISO 42010)...
✅ ADR structure (ISO 42010) passed.
🔍 PRD allocation...
✅ PRD allocation passed.
🔍 Plan traceability...
✅ Plan traceability passed.
🔍 Relative links and anchors...
checked 212 Markdown files: 0 broken link(s)
✅ Relative links and anchors passed.
🔍 FileSquid and ImageSquid document boundary...
checked 165 documents: 0 link(s) cross the FileSquid/ImageSquid boundary
✅ FileSquid and ImageSquid document boundary passed.
🔍 MVP queue and merge driver tests...
✅ MVP queue and merge driver tests passed.
🔍 MVP progress bar...
✅ MVP progress bar passed.
🔍 Task cleanup...
✅ Task cleanup passed.
✅ Documentation validation passed.
Starting Change Scope Validation...
✅ Change scope validation passed (166 assertions).
Starting Lint and Test Validation...
ℹ️  Test scope: full
🔍 Clippy (apps/filesquid)...
✅ Clippy passed (0s).
🔍 Workspace tests (apps/filesquid)...
✅ Workspace tests passed (46s).
🔍 Module crate alone: filesquid-core (apps/filesquid)...
✅ Module crate alone: filesquid-core passed (1s).
🔍 Module crate alone: filesquid-ocfl (apps/filesquid)...
✅ Module crate alone: filesquid-ocfl passed (24s).
🔍 Module crate alone: filesquid-metadata (apps/filesquid)...
✅ Module crate alone: filesquid-metadata passed (0s).
🔍 Module crate alone: filesquid-c2pa (apps/filesquid)...
✅ Module crate alone: filesquid-c2pa passed (4s).
🔍 Module crate alone: filesquid-api (apps/filesquid)...
✅ Module crate alone: filesquid-api passed (3s).
🔍 Module crate alone: filesquid-native (apps/filesquid)...
✅ Module crate alone: filesquid-native passed (0s).
🔍 Clippy (filesquid-metadata, all features) (apps/filesquid)...
✅ Clippy (filesquid-metadata, all features) passed (1s).
🔍 Tests (filesquid-metadata, all features) (apps/filesquid)...
✅ Tests (filesquid-metadata, all features) passed (3s).
🔍 wasm32 build (filesquid-metadata, default features off) (apps/filesquid)...
✅ wasm32 build (filesquid-metadata, default features off) passed (0s).
🔍 wasm32 build (filesquid-metadata, ocfl feature) (apps/filesquid)...
✅ wasm32 build (filesquid-metadata, ocfl feature) passed (1s).
🔍 Rustdoc (apps/filesquid)...
✅ Rustdoc passed (0s).
🔍 Clippy (desktop, test-instrumented) (apps/filesquid)...
✅ Clippy (desktop, test-instrumented) passed (1s).
🔍 Desktop tests (test-instrumented) (apps/filesquid)...
✅ Desktop tests (test-instrumented) passed (18s).
🔍 ESLint (apps/filesquid/ui)...
✅ ESLint passed (3s).
🔍 TypeScript (apps/filesquid/ui)...
✅ TypeScript passed (4s).
🔍 Vitest (apps/filesquid/ui)...
✅ Vitest passed (13s).
ℹ️  Playwright e2e skipped for apps/filesquid/ui (set FILESQUID_E2E=1 to run).
🔍 ESLint (apps/imagesquid)...
✅ ESLint passed (2s).
🔍 TypeScript (apps/imagesquid)...
✅ TypeScript passed (2s).
🔍 Vitest (apps/imagesquid)...
✅ Vitest passed (3s).
ℹ️  Playwright e2e skipped for apps/imagesquid (set FILESQUID_E2E=1 to run).
ℹ️  Desktop packaged-window e2e skipped (set FILESQUID_DESKTOP_E2E=1 to run).
✅ Lint and test validation passed (test scope: full).
========================================
✅ All Pre-Commit Hooks Passed! Test scope: full (151s)
========================================
```
