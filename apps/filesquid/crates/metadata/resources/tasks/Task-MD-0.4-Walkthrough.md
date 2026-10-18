# Task MD-0.4 Walkthrough: WASM build and OCFL-sourced extraction

Global [Task 0.15](../../../../../../resources/implementation-plan.md#task-0.15). Branch `feat/md-0.4-wasm-ocfl-extraction`.

## MD-S-003: Persist through a pluggable `MetadataStore`

`sidecar` default, optional `ocfl` through `filesquid-ocfl`, optional native `sqlite` (later task). `filesquid-ocfl` is now an optional dependency behind the `ocfl` feature and absent from the default features.

## MD-S-009: The crate MUST build for `wasm32-unknown-unknown`

Extraction takes caller-supplied bytes or a reader plus a `FileRef`, independent of any `MetadataStore`, never a filesystem path; the CLI and path helpers are native-only.

## What changed

- `file_ref::FileRef`: opaque reference (object id, optional version, validated logical path).
- `store::MetadataStore` (async, runtime-agnostic) with `StoreError`; `store::sidecar::SidecarStore` (`<file>.metadata.md`, native only, Git-guarded, write-then-rename); `store::ocfl::OcflStore` plus the former `store.rs` functions (`metadata_key`, `load_from`, `save_to`, and native `load`/`save`).
- `extract::extract_from_bytes` and `extract_from_reader` replace `extract_into(path)`; they return an `Extracted` report (EXIF found, ID3 found, statements the register refused). Genuine reader I/O errors are returned; absent or truncated tag blocks are not errors.
- Cargo features: `sidecar` (default), `ocfl`, `ocfl-fs`, `cli`. `cli` module compiled out for `wasm32`. Tests that need a feature declare `required-features`.
- `kamadak-exif` 0.5.5 and `id3` 1.17.2 both compile for `wasm32-unknown-unknown`; no adapter needed gating.
- Hook: `scripts/verify_tests.sh` runs clippy and tests with `--all-features` for `filesquid-metadata` and `cargo check --lib --target wasm32-unknown-unknown` with default features off and with `ocfl`, whenever the crate is in scope (and always in the full suite). `apps/filesquid/rust-toolchain.toml` now lists the target. The F-052 BDD scenarios run the same wasm32 checks (skippable only with `FILESQUID_SKIP_WASM32=1`).
- `tests/unit/isolation.rs` and the F-052 feature updated: `filesquid-ocfl` must be optional and outside the default features.
- `filesquid-cli` now enables the `cli` feature of `filesquid-metadata` (manifest change only); `filesquid-desktop` keeps the default features.
- Docs: module README, root README, ADR 029 note, module plan and global plan boxes, checklist.

Notes for later tasks: the `ocfl` store still writes `metadata.md` whole through the backend (it does not yet create a new OCFL version per edit; that belongs to the OCFL write path in `filesquid-ocfl`). The sidecar naming (`<file>.metadata.md`) is this task's reading of ADR 027's "`metadata.md` beside a plain file"; change it here if the owner prefers one `metadata.md` per directory.

## Coverage

| Requirement | Tests |
|---|---|
| MD-S-003 optional OCFL store | `unit_isolation`, `unit_store` (`OcflStore`), `unit_sidecar`, F-052 "Persistence is pluggable and filesquid-ocfl is optional", "Sidecar store keeps a record beside a plain file" |
| MD-S-009 wasm32 build | F-052 "Engine builds for the browser" (default features off, and with `ocfl`), hook step |
| MD-S-009 bytes/reader plus `FileRef` | `unit_extract`, `unit_file_ref`, `unit_cli` extract, F-052 "Extraction reads bytes from filesquid-ocfl", "Extraction takes a reader and a file reference, independent of any store", "CLI and path helpers are native-only" |

## Mocked Environment Variables

None.

## Raw test output

```text
$ cargo test -p filesquid-metadata --locked
     Running unittests src/lib.rs (<target>/debug/deps/filesquid_metadata-891d4d9141a10c65)
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/csv.rs (<target>/debug/deps/unit_csv-dc57a5734821db6f)
running 10 tests
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/extract.rs (<target>/debug/deps/unit_extract-2e4333f990fc812c)
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/file_ref.rs (<target>/debug/deps/unit_file_ref-e6896331984eee5f)
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/isolation.rs (<target>/debug/deps/unit_isolation-f2b7d357cf6e9521)
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/json.rs (<target>/debug/deps/unit_json-8a8d7a37fee19882)
running 19 tests
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/markdown.rs (<target>/debug/deps/unit_markdown-91b5bece22962ede)
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/register.rs (<target>/debug/deps/unit_register-fccc8374e3b068da)
running 10 tests
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/sidecar.rs (<target>/debug/deps/unit_sidecar-80046b3ca9c46da4)
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

$ cargo test -p filesquid-metadata --all-features --locked
     Running unittests src/lib.rs (<target>/debug/deps/filesquid_metadata-d15df7d01d4f34b2)
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/features.rs (<target>/debug/deps/features-07a7ab5b359570c0)
[Summary]
1 feature
22 scenarios (21 passed, 1 skipped)
58 steps (57 passed, 1 skipped)
     Running tests/unit/cli.rs (<target>/debug/deps/unit_cli-aa0e6f44367c766a)
running 17 tests
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/csv.rs (<target>/debug/deps/unit_csv-914124dba3065950)
running 10 tests
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/extract.rs (<target>/debug/deps/unit_extract-b0a307480a3fe1c9)
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/file_ref.rs (<target>/debug/deps/unit_file_ref-b9629f940ee45048)
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/isolation.rs (<target>/debug/deps/unit_isolation-e1579170a44422ff)
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/json.rs (<target>/debug/deps/unit_json-e7a4b27fc9baf530)
running 19 tests
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/markdown.rs (<target>/debug/deps/unit_markdown-7e2b61bf9fcc878c)
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/register.rs (<target>/debug/deps/unit_register-3edccd342da90018)
running 10 tests
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/sidecar.rs (<target>/debug/deps/unit_sidecar-ea6bb586574858e8)
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/store.rs (<target>/debug/deps/unit_store-123c6fdb18e01892)
running 9 tests
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.88s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

$ cargo check -p filesquid-metadata --lib --no-default-features --target wasm32-unknown-unknown --locked
    Checking filesquid-metadata v0.1.0 (/home/user/filesquid/apps/filesquid/crates/metadata)
    Finished `dev` profile [unoptimized] target(s) in 0.40s
$ ... --no-default-features --features ocfl --target wasm32-unknown-unknown
    Checking filesquid-metadata v0.1.0 (/home/user/filesquid/apps/filesquid/crates/metadata)
    Finished `dev` profile [unoptimized] target(s) in 0.45s
```

## Pre-commit gate

`FULL=1 bash scripts/pre-commit.sh` summary line (scripts/ and the toolchain file changed, so every commit of this branch runs the full scope): `✅ All Pre-Commit Hooks Passed! Test scope: full (175s)`.
