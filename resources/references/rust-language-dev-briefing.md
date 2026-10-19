# What Rust is

Revised 2026-10-15: names updated to the FileSquid-first model ([ADR 029](../architecture/029-filesquid-unified-product-and-distributions.md)).

Rust is a compiled, statically typed systems language. It aims at C/C++ territory — native binaries, predictable performance, direct control of memory and threads — without a garbage collector and without C’s class of memory bugs.

Graydon Hoare started it at Mozilla around 2006; 1.0 shipped in 2015. It is now used for browsers (parts of Firefox), operating-system components (Windows, Linux, Android), cloud infrastructure (AWS Firecracker, Cloudflare, Discord), CLIs, and, in FileSquid’s case, a local-first library (ImageSquid is its photo distribution) plus a hosted ciphertext service at `*.filesquid.tech`.

The design bet: **make illegal states unrepresentable at compile time**, then emit code that runs as close to the metal as C.

For an experienced developer, the unusual part is not syntax. It is **ownership**: every value has one owner; when the owner goes out of scope the value is dropped (destructors run, memory is freed). You may **borrow** a value (`&T` shared, `&mut T` exclusive) under rules the compiler (`rustc`) checks:

- Many shared borrows, **or** one mutable borrow, never both.
- A borrow cannot outlive the value it points at (**lifetimes**).
- You cannot use a moved value.

Those rules replace a GC *and* replace “don’t use after free.” They also encode thread safety: types that are safe to share across threads implement `Send`/`Sync`; types that are not, cannot be accidentally shared. Data races are a compile error, not a production incident.

Other core pieces you will live in daily:

| Concept | Rough analogue | Difference |
|---|---|---|
| `struct` / `enum` | classes / tagged unions | Enums are first-class and exhaustive; `match` must cover every variant |
| `Option<T>` / `Result<T, E>` | nullable / exceptions | Absence and failure are values. `?` unwraps or returns the error. There is no `null` |
| Traits | interfaces / protocols | Also used for operator overloading, async, and generic bounds (`T: Clone + Send`) |
| Generics + `impl` | templates / type classes | Monomorphized like C++ templates (fast, larger binaries) unless you use `dyn Trait` |
| `async` / `.await` | JS promises / C# async | Cooperative; FileSquid’s hosted path (`filesquid-server`) runs on **Tokio** (Axum) |
| `unsafe` | C | Allowed, but must be isolated; FFI to libvips/FFmpeg lives here |
| Cargo | npm + CMake + pip | Build, test, docs, lockfile, features, workspaces in one tool |

Edition **2024** is current (with the usual `edition = "2024"` in `Cargo.toml`). Editions are opt-in, so old crates keep compiling; the language does not break the ecosystem the way Python 2→3 did.

---

## Purpose — why this language, why here

Rust’s purpose in the industry is **safe systems software**: parsers, crypto, filesystems, network services, embeddable libraries, anything where a use-after-free, a data race, or a GC pause is unacceptable.

FileSquid’s purpose for it is narrower and explicit ([ADR 001](../architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)):

- One systems language, not Rust plus Python plus Elixir.
- Native work that *must* be native: OCFL on disk and S3, SHA-256, AES-GCM envelopes, SQLite, libvips, FFmpeg, LibRaw, PhotoKit-style OS adapters.
- The same contracts compiled into the desktop host, the hosted mailbox, and the browser (as WASM).
- Memory and CPU bounds that match a free local photo app on a 16 GB laptop (U-17 / NFR-002/003), not a GC-heavy Electron heap.

The hosted service at `*.filesquid.tech` is **not** a photo backend. Axum is a ciphertext mailbox: accounts, opaque IDs, ciphertext parts, static SPA files. Private meaning (albums, captions, search, pixels) is computed in the owner’s process — Tauri Rust or browser WASM — never on the server ([ADR 004](../architecture/004-axum-hosted-ciphertext-mailbox.md)).

---

## Strengths

**Memory safety without a GC.** No tracing pauses while decoding a 60 MP RAW or hashing a 4K video. Drop is deterministic. For a photo library that holds decoded pixels, this is the point.

**Fearless concurrency (the marketing name is accurate).** `Send`/`Sync` plus exclusive `&mut` make “two workers mutating the same inventory” a type error unless you wrap it in `Mutex`/`RwLock` or a channel. FileSquid’s durable jobs, per-object OCFL leases, and bounded worker pools ([ADR 007](../architecture/007-durable-jobs-and-interactive-performance.md)) map onto that model.

**Zero-cost abstractions.** Iterators, generics, and `async` compile down to the same kind of code you would write by hand in C. You pay for what you use.

**One toolchain that actually ships.** `rustc` + Cargo + `rustup` + clippy + rustfmt + rust-analyzer is the default, not an optional IDE plugin. Cross-compilation, `wasm32-unknown-unknown`, and target triples (`x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, …) are first-class.

**Excellent FFI to C.** libvips, LibRaw, libheif, FFmpeg are C libraries. Rust is designed to call them (`bindgen`, `cc`, `pkg-config`) and to be called *from* them. That is how desktop media work happens ([ADR 006](../architecture/006-libvips-native-stills-not-imagemagick.md), [ADR 011](../architecture/011-native-media-adapters-and-ffmpeg-license.md)).

**Same source, three runtimes.** Native binary (Tauri, Axum), and a WASM subset for envelopes, hashes, replica queries ([ADR 008](../architecture/008-browser-wasm-scope.md)). Crypto test vectors can be shared across Rust / WASM / WebCrypto ([ADR 009](../architecture/009-cryptography-webcrypto-compatible-suite.md)).

**Error handling that matches preservation work.** `Result` forces you to decide what happens when an inventory sidecar digest mismatches. That is the right default for OCFL, not “throw and hope a middleware logs it.”

**Ecosystem fit.** Axum (HTTP), Tokio (async runtime), SQLx (Postgres/SQLite), serde (JSON), the official `c2pa` crate, `ring`/`rustcrypto`, wasm-bindgen — these are the libraries the ADRs already name.

---

## Weaknesses

**The learning curve is real.** Ownership, lifetimes, and “why won’t this compile?” last weeks, not an afternoon. Experienced C#/Java/Python/JS developers typically write slow, clone-heavy, or over-`Arc<Mutex<_>>` code until the model clicks. Budget pair-programming and clippy, not heroics.

**Compile times.** Debug builds are acceptable; full release + LTO on a workspace that also links libvips/FFmpeg is not instant. Incremental compilation and `sccache` help; they do not make it TypeScript.

**Expressiveness vs. iteration speed.** Prototyping UI, gallery layout, and “does this album query feel right?” belongs in the vanilla TypeScript UI ([ADR 031](../architecture/031-vanilla-typescript-ui-and-white-label-theming.md), superseding Svelte). Rust is the wrong place to sketch product UX.

**WASM is a subset, not a desktop.** No filesystem OCFL, no libvips, no FFmpeg, no PhotoKit in the browser. Compiling a crate to WASM does not magically give it those APIs ([ADR 008](../architecture/008-browser-wasm-scope.md)). Native processing **must not** be a dependency of the WASM graph.

**Cargo features are a footgun.** Features are additive and unify across the whole dependency tree. FileSquid forbids using features as the *only* privacy boundary: Axum must not be able to turn on “search private albums” by depending on a crate that has that feature ([ADR 001](../architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)). Separate crates, explicit executable dependencies, CI that fails if the hosted binary’s graph contains native processing.

**FFI and `unsafe` still exist.** Binding FFmpeg incorrectly can still segfault. The safety story is “unsafe is localized and reviewed,” not “unsafe is impossible.”

**Async is two worlds.** Tokio (hosted Axum) vs. maybe blocking workers on desktop vs. WASM’s single-threaded-plus-workers model. Mixing blocking libvips calls onto the Tokio runtime will stall the mailbox. Isolation of expensive work is a product requirement, not an implementation detail.

**Binary size and legal surface.** A release binary plus bundled LGPL FFmpeg, libvips, libheif, LibRaw is a real installer, with license and patent review before a public desktop binary ([ADR 011](../architecture/011-native-media-adapters-and-ffmpeg-license.md)). FileSquid is MIT-licensed; the *linked inventory* is not covered by that license.

**Hiring and onboarding.** Fewer Rust developers than TypeScript. The team accepted that: two languages (Rust + TypeScript), not three.

**Language choice is not a performance pass.** The vision is explicit: stack ADRs are not U-17 evidence. Unbounded galleries and unbounded job queues are slow in any language ([ADR 007](../architecture/007-durable-jobs-and-interactive-performance.md)).

---

## How Rust projects are organized

Think **crates**, not “a repo with a `src/`.”

A **package** is a `Cargo.toml`. It can contain:

- a **library crate** (`src/lib.rs`) — what others `use`
- zero or more **binary crates** (`src/main.rs` or `src/bin/*.rs`)
- integration tests (`tests/`), examples, benches

A **workspace** is several packages that share a `Cargo.lock` and can depend on each other with `path = ...`. FileSquid’s rule is **one product-local workspace at `apps/filesquid/`**, not a root `Cargo.toml` ([ADR 013](../architecture/013-cargo-svelte-workspaces.md), amended by [ADR 029](../architecture/029-filesquid-unified-product-and-distributions.md)). No language manifest at the repository root.

Current layout (crates are `filesquid-*`; core, server and desktop were formerly `imagesquid_*`; see [ADR 029](../architecture/029-filesquid-unified-product-and-distributions.md) and the [apps README](../../apps/README.md)):

```text
apps/
  filesquid/                # FileSquid: the product
    Cargo.toml              # product-local Cargo workspace
    rust-toolchain.toml
    crates/
      core/                 # filesquid-core: portable contracts (opaque IDs, jobs, crypto envelopes)
      ocfl/                 # filesquid-ocfl: OCFL 1.1 module
      metadata/             # filesquid-metadata: metadata module (Dublin Core, stores)
      c2pa/                 # filesquid-c2pa: C2PA module (planned; docs and features only, not a workspace member yet)
      api/                  # filesquid-api: the single FileSquid API (routes, contract, auth, events)
      server/               # filesquid-server: Axum host
      desktop/              # filesquid-desktop: Tauri 2 host
    ui/                     # @filesquid/ui: vanilla TypeScript + Vite (not Cargo; ADR 031)
  imagesquid/               # ImageSquid distribution: views, tokens, labels, brand (no Rust)
```

Module crates (`core`, `ocfl`, `metadata`, `c2pa`) do not depend on Axum, Tauri or `filesquid-api`; hosts link `filesquid-api`.

`filesquid-ocfl` is already named and frozen ([ADR 016](../architecture/016-rust-ocfl-library.md), [ADR 023](../architecture/023-filesquid-ocfl-engine-and-parquet.md)): Cargo name `filesquid-ocfl`, Rust import `filesquid_ocfl`, path `apps/filesquid/crates/ocfl/`. It is a FileSquid OCFL 1.1 module crate that builds, tests and publishes on its own ([ADR 029](../architecture/029-filesquid-unified-product-and-distributions.md)). Default public API is a generic engine. FileSquid collection snapshots and dual-profile encryption are callers. A later optional `parquet` Cargo feature may project logical state; it is off for the FileSquid MVP. A later `fsquid` links the library as an installable tool.

**Library families and who may link them** ([ADR 001](../architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)):

```text
Portable contracts  →  Tauri, Axum, WASM
Private client      →  Tauri, WASM          (never Axum)
Native processing   →  Tauri only           (never Axum, never WASM)
```

That split is a **privacy and capability architecture**, not a style preference. CI must fail the hosted build if libvips/FFmpeg/OCFL disk appear in Axum’s dependency graph.

Everyday commands:

```sh
cd apps/filesquid          # the Cargo workspace root
rustup toolchain install   # the exact release pinned in rust-toolchain.toml
cargo test -p filesquid-ocfl
cargo clippy --all-targets
cargo fmt
cargo doc --open
```

Dependencies come from [crates.io](https://crates.io) (or git/path). `Cargo.lock` pins versions for binaries; libraries often keep lockfiles too in this kind of app workspace. Features in `Cargo.toml` gate optional backends (e.g. filesystem vs S3 on `filesquid-ocfl`) — additive capabilities, not secret “enable private search on the server” switches.

Tests: unit tests live next to the code (`#[cfg(test)] mod tests`); integration tests in `tests/`. Property tests (`proptest`/`quickcheck`) and golden OCFL fixtures (community validators as oracles, not as the engine) are the natural style for a preservation library.

---

## Packaging and deployment

Rust compiles to a **target triple**. You get a native executable, a static/dynamic library, or a `.wasm` module. There is no JVM, no required runtime, no `node_modules` in production for the Rust parts.

### Desktop (primary FileSquid delivery)

**Tauri 2** wraps the vanilla TypeScript UI ([ADR 031](../architecture/031-vanilla-typescript-ui-and-white-label-theming.md)) in the OS webview (WebKitGTK / WKWebView / WebView2) and talks to a Rust host over IPC — not a local HTTP server ([ADR 002](../architecture/002-tauri2-desktop-shell.md)).

What ships:

- The Rust host binary (and any helper worker processes for FFmpeg isolation).
- Bundled native libraries: libvips, LGPL FFmpeg (no `--enable-gpl` / `--enable-nonfree` in the default installer), LibRaw, libheif where legal.
- The compiled TypeScript UI assets inside the Tauri bundle.
- Per-OS installers: `.deb`/AppImage/Flatpak-class on Linux first; later `.dmg`/`.msi`. **A single binary for all OSes is not required.**

Tauri’s updater, code signing, and capability files (deny-by-default IPC) are part of packaging, not an afterthought. The webview never gets raw filesystem paths or SQL.

Compared with Electron: no bundled Chromium, much smaller idle RAM — which is why Electron was rejected against U-17.

### Hosted (server)

`filesquid-server` (Axum on Tokio) is a normal Linux service: one (or a few) statically-ish linked binaries, SQLx → PostgreSQL, S3-compatible object storage for ciphertext. Deploy like any other compiled daemon (container, systemd, whatever the later region ADR says). It may serve the already-built SPA as static files. It must not SSR private HTML.

Scale-out is ordinary HTTP + Postgres + object storage. The mailbox never decodes photos, so “more users” does not mean “more libvips on the server.”

### Web (independent browser app)

Browser-safe crates compile to **WebAssembly** (`wasm32-unknown-unknown` + `wasm-bindgen`, typically `wasm-pack` or Vite’s WASM plugin). Day-one WASM: envelopes, hashes, replica queries — **not codecs**. Expensive WASM runs on workers, incrementally, not on the UI thread.

The SPA is ordinary static files. Unlock happens in the browser (WASM and/or WebCrypto with shared test vectors). Gallery pixels are decrypted replicas or prepared derivatives, never a plaintext query to Axum.

WASM download size, browser storage quotas, and eviction are product states, not “Rust will handle it.”

### Mobile

FileSquid’s accepted scope is a **focused mobile-web companion**, not a native App Store/Play binary in the stack ADRs. The same TypeScript client + WASM path is the mobile delivery.

Tauri 2 *can* target iOS and Android (Rust + webview). That is available later if a product ADR says so; it is not the current architecture. Native mobile would still exclude the full desktop media stack unless you ship a huge binary and fight App Store codec/legal rules.

| Surface | Rust form | UI | Native media / OCFL disk |
|---|---|---|---|
| Linux/macOS/Windows desktop | Tauri host + workers | TypeScript UI in OS webview | Yes |
| Hosted mailbox | Axum binary | Serves static SPA only | No private media |
| Independent browser / mobile web | WASM subset | Same TypeScript SPA | No; prepared derivatives + replica |
| Native iOS/Android | Not in accepted ADRs | — | — |

---

## Scalability and performance

**Single-machine (the photo library).** This is where Rust earns its keep.

- Decode, hash, encrypt, and OCFL write are CPU- and IO-bound C-equivalent work with no GC jitter.
- Rayon-style data parallelism or explicit worker pools for hashing/thumbnails; Tokio for network (S3 multipart, mailbox).
- Bounded queues: default highlights from ADR 007 include ≤4 active transfers, ≤2 CPU-heavy workers, staging cap 10 GiB, migration process memory ≤2 GiB excluding ML packs, warm interaction p95 ≤100 ms on a declared 4-core/16 GB/SSD desktop.
- Interactive path stays in the client (indexed, paged gallery; nearby prefetch; cache with a budget). Background jobs persist identity and resume from **canonical OCFL receipts**, not from “the UI was still open.”

Rust makes this *feasible*. Unbounded work still fails the NFRs. Language reputation is not a measurement.

**Multi-machine (hosted).** Axum is a standard async Rust HTTP server: cheap tasks (green threads on Tokio), high connection counts, low RAM per connection — the Discord/Cloudflare class of story. FileSquid’s hosted path should stay *easy* to scale because it does the wrong thing for a photo CDN on purpose: it stores ciphertext blobs and operational rows. Thumbnail storms, face-search fan-out, and libvips clusters are **not** the hosted architecture.

**Correctness under crash and concurrency** is the other kind of scale. OCFL writes reserve `vN` *before* object-root bytes; root inventory sidecar last; S3 heads use compare-and-swap. Readers treat incomplete `vN` as unpublished. That protocol is implemented in Rust because the type system and `Result` help you not skip a step; the protocol still has to be written ([ADR 016](../architecture/016-rust-ocfl-library.md)).

**WASM performance.** Fine for hashes, envelope open, replica queries. Wrong for RAW develop and 4K transcode. Do not “just compile native processing to WASM.”

**What will actually bite you:** decoded-image RAM, cache keys that forget edit versions, prefetch that never cancels, blocking FFI on the async runtime, and debug-vs-release surprises (Rust debug is slow; always measure `--release` / packaged artifacts).

---

## Other things a developer will find interesting

**The borrow checker is a design tool.** When the compiler refuses two mutable aliases to an OCFL inventory, it is telling you the same thing the write protocol says: one lease, then publish. Fighting the checker with `clone()` everywhere works and is sometimes right (owned snapshots); `Arc<T>` is how you share immutable data across workers; `Arc<Mutex<T>>` is a last resort, not a default.

**Algebraic types for domain rules.** An OCFL object that is `Incomplete { expected: vN, head: vNMinus1 }` vs `Published { head: vN }` is an enum, not a boolean and a prayer. Pattern matching makes “don’t treat unpublished as committed” local and testable.

**`Drop` and RAII.** File locks, temp workspaces assembled *outside* the object root, then published — destructors can clean up incomplete work if you structure it that way (and if you don’t `mem::forget` the guard).

**No null, few exceptions.** Panic is for “this is a bug” (assert, unwrap in tests). Library and IPC boundaries return `Result`. Tauri commands should serialize those errors, not crash the host.

**Macros are used, not worshipped.** `serde` derives, `tokio::main`, `#[tauri::command]`, SQLx `query!` (compile-time checked SQL). You will read more macros than you write.

**Editions and stability.** Code from 2018 still builds. That is why crates.io works. Prefer current stable; nightly only if a crate truly requires it (FileSquid should not).

**Documentation is a first-class artifact.** `///` doc comments compile into `cargo doc`. Public crate APIs (`filesquid-ocfl` especially) should be documented as if another team — or a debug CLI — is the caller, because they will be.

**Clippy is opinionated and usually right.** Treat warnings as part of review. `unsafe` blocks should have a one-line safety comment: what invariant you are upholding.

**Supply chain.** `cargo audit` / `cargo deny` for licenses and advisories. That matters twice: crates.io deps, and the C libraries you bundle. The default desktop installer must stay LGPL-FFmpeg-clean.

**Interoperability story that is unusual.** Same SHA-256 / AES-256-GCM / HKDF / Ed25519 / X25519 / Argon2id suite on native Rust, WASM, and WebCrypto, with matching test vectors. If those drift, the independent browser cannot open what the desktop sealed.

**What not to reach for from other ecosystems**

- Do not add Phoenix, FastAPI, Celery, Oban, Electron, Flutter, ImageMagick, or a second OCFL engine (`rocfl` is a CI oracle, not the library).
- Do not put a `.git` directory in an OCFL storage root. Git is this repo’s VCS, not the photo store.
- Do not treat Axum as “the backend for the gallery.”
- Do not keep heavy decode/upload running after the user quit unless a product setting says so; default is checkpoint and resume on next launch.

---

## How to ramp up on this team

1. Official book: [The Rust Programming Language](https://doc.rust-lang.org/book/) — especially ownership, enums, `Result`, modules, and Cargo.
2. [Rust by Example](https://doc.rust-lang.org/rust-by-example/) and [Too Many Lists](https://rust-unofficial.github.io/too-many-lists/) when ownership feels abstract.
3. Async: Tokio tutorial *after* you can write synchronous library code. Axum will make more sense then.
4. FFI: skim how `bindgen` wraps a C header before touching libvips.
5. Project: read ADRs **001, 002, 004, 007, 008, 013, 016, 029, 031** in `resources/architecture/` before writing a crate. They are the architecture; the crates live under `apps/filesquid/crates/`.

Mental model to keep: **TypeScript paints; Rust decides, persists, and proves.** If a rule matters on desktop *and* in the browser *and* must never run on the server, it belongs in a portable or private-client crate — not in TypeScript UI code, and not in an Axum handler.