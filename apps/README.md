# Apps Directory (`/apps`)

This directory holds the FileSquid product and its white-label UI distributions. Documentation validators live in the repository's `scripts/docs/`. The repository root has no `Cargo.toml` or `package.json`.

FileSquid is one backend product with one API. It has OCFL, metadata and C2PA modules. Hosts run that API, and plain HTML/CSS/TypeScript UIs call it. A distribution such as ImageSquid is a UI layer only: views, theme tokens, labels, brand assets and a distribution manifest. It has no Rust and no server code. Any backend capability a distribution needs is added to FileSquid as a module behind the same API ([product vision](../resources/product-vision.md)).

```text
apps/
├── README.md
├── filesquid/                 # FileSquid: the product (extractable to its own repository)
│   ├── Cargo.toml             # Product-local Cargo workspace (not at the repository root)
│   ├── rust-toolchain.toml
│   ├── crates/
│   │   ├── core/              # filesquid-core: portable contracts (no HTTP, no Tauri, no codecs)
│   │   ├── ocfl/              # filesquid-ocfl: OCFL 1.1 module
│   │   ├── metadata/          # filesquid-metadata: metadata module (Dublin Core, metadata.md, stores)
│   │   ├── c2pa/              # filesquid-c2pa: C2PA module (foundation: byte I/O, fetcher and crypto-backend traits)
│   │   ├── api/               # filesquid-api: the single FileSquid API (routes, contract, auth, events)
│   │   ├── server/            # filesquid-server: Axum host, multi-user
│   │   └── desktop/           # filesquid-desktop: Tauri 2 host
│   └── ui/                    # @filesquid/ui: vanilla TypeScript + Vite default UI and custom elements
└── imagesquid/                # ImageSquid: white-label UI distribution (vanilla TypeScript + Vite; no Rust)
```

## Hosts

| Host | How the UI reaches the API | Who sees plaintext | Status |
|---|---|---|---|
| Desktop (`filesquid-desktop`, Tauri 2) | In-process, through a Tauri custom URI scheme. No TCP port. | The owner's device only | Building now |
| Multi-user server (`filesquid-server`, Axum) | HTTP (and an event channel) on `FILESQUID_BIND`, default `127.0.0.1:8083`. It also serves the selected UI's static files. | The operator of that server (self-hosters, institutions) | Building now |
| Hosted backup and sync at `*.filesquid.tech` | The service stores ciphertext only. Clients encrypt before upload. | Nobody on the service side | Planned |
| E2EE browser host | The same API, run as WebAssembly in the browser; the server is a ciphertext mailbox | The owner's browser only | Roadmap |

## Rules for this directory

- Module crates (`core`, `ocfl`, `metadata` and `c2pa`) MUST NOT depend on Axum, Tokio-net or `tauri`. Each one builds and tests alone (`cargo test -p <crate>`).
- Hosts link `filesquid-api`. Domain logic does not go in a host crate.
- `apps/imagesquid` depends on `@filesquid/ui` by relative path during development and on a published version for release.
- Unique local ports: `apps/filesquid/ui` dev 5173 / preview 4173, `apps/imagesquid` dev 5174 / preview 4174, `filesquid-server` 8083.

## Docker

The root `docker-compose.yml` runs Postgres, the observability stack and, optionally, the `filesquid-server` container. The Tauri desktop host is not a Compose service; it runs natively.

## Logging

Every process writes unbuffered JSON to stdout and records 100% of its activity in FileSquid's activity journal; live health comes from the process registry and the Monitor dashboard in `@filesquid/ui` ([ADR 033](../resources/architecture/033-activity-journal-and-live-health-monitoring.md)). External sinks (VictoriaLogs, VictoriaMetrics) are optional through OTLP and the Compose `observability` profile. The desktop host never runs a telemetry server.

## Testing

Each app keeps its tests under its own `tests/` directory: `unit/`, `integration/`, `ui/`, `features/`, `e2e/`, `performance/` and `allure-results/`. Run Cargo from `apps/filesquid/` (the workspace root), for example `cargo test --workspace --locked` or `cargo test -p filesquid-ocfl`. Run npm inside `apps/filesquid/ui/` or `apps/imagesquid/`. The root [README](../README.md) has the full test and report commands.
