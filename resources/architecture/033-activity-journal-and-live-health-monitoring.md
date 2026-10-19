# Architecture Decision Record - FileSquid

033-activity-journal-and-live-health-monitoring.md

## Title

FileSquid's own activity journal (complete logging) and live health monitoring with a FileSquid dashboard; external telemetry stacks are optional OTLP sinks

## Status

Accepted

Accepted: 2026-10-15. Owner requirements: (1) **100% logging of all activity**, where activity means domain operations, job and process state changes, authorisation refusals, credential issue, warnings, errors and host start and stop (owner decision 2026-10-17, Task API-F1.7: successful API calls are not journaled one by one); (2) **live health monitoring of the system and its processes**. The owner prefers FileSquid's own tooling to retro-fitting the inherited 2026 "best practice" telemetry stack (collector, Redpanda, Loki, Prometheus, Grafana), and directed: adopt the OpenTelemetry + VictoriaMetrics option only as an optional export, drop Redpanda, build FileSquid's own dashboard, and revise AGENTS.md and `logging-enforce.md` accordingly. Supersedes the collector and stack parts of [ADR 014](./014-stdout-observability.md) (stdout JSON is kept). An earlier draft of this ADR in the same branch kept a Grafana stack beside the server; this version replaces it.

Amended 2026-10-18T06:52:39Z (owner decision on the [CL-0.1 Conflict Artifact](../../apps/filesquid/crates/api/resources/tasks/Task-CL-0.1-Conflict.md), option A): the journal store and the host checks are host-injected. `filesquid-api` keeps the `JournalStore` and `HostChecks` traits, the in-memory store, the monitor, the health rules and the admin routes, and links no SQLite driver ([API-S-006](../../apps/filesquid/crates/api/resources/srs.md)). The SQLite journal store, the data-directory checks (write probe, free space) and `HostMonitor` live in the native-processing library `filesquid-native` (`apps/filesquid/crates/native`). `filesquid-desktop` links it with its `desktop` Cargo feature, which will hold the desktop-only native processing (OCFL coordinator, job and media workers; DE-0.8). `filesquid-server` MAY link `filesquid-native` with its default features only (the SQLite journal store, the data-directory checks and `HostMonitor`), keeping its journal in `<data dir>/filesquid.sqlite` until its PostgreSQL operational store lands (SV-F1.2, which then starts `HostMonitor` with its PostgreSQL store); the server's `unit_native_features` test (`cargo tree -p filesquid-server -e features`) fails if it enables `desktop`. Decision 2's PostgreSQL target for servers is unchanged.

## Context / Requirement Reference

- Requirements: [FileSquid U-87, U-88](../product-vision.md#administration-and-monitoring), [FE-080, FE-081](../product-vision.md#platform-api-and-hosts), [T-055, T-056, T-057](../product-vision.md#api-and-hosts), [PRD F-061](../prd.md#f-061-activity-journal-and-live-health-monitoring).
- The inherited stack was a template default, not a FileSquid requirement. It does not meet requirement (1) by itself: stdout is lost when nobody collects it, which is always the case on the desktop host. Redpanda Community Edition is under the Business Source License, which is not an OSI open-source licence; Loki and Grafana are AGPL-3.0; Promtail reached end of life on 2026-03-02.
- FileSquid already has the pieces a domain-aware monitor needs: an operational store (SQLite on desktop, PostgreSQL on servers), durable jobs with checkpoints ([ADR 007](./007-durable-jobs-and-interactive-performance.md)), an API with an event channel ([ADR 030](./030-filesquid-api-and-transports.md)) and a UI package ([ADR 031](./031-vanilla-typescript-ui-and-white-label-theming.md)).
- The desktop host is a Tauri process, not a container; nothing collects its stdout.

## Decision

**1. Instrumentation.** Every FileSquid process instruments with Rust `tracing`. Every event still goes unbuffered to **stdout as JSON** (ADR 014). No process writes ad-hoc text log files.

**2. Activity journal: 100% logging of activity as defined under Status.** Each host that runs `filesquid-api` (the desktop app and `filesquid-server`; the browser host when it exists) records FileSquid's activity in an append-only **activity journal** in its own operational store (SQLite on desktop, PostgreSQL on `filesquid-server`). The journal is structured product data, queried through the API; it is not a log file.
- **Local-first:** the journal, registry and health checks use the host's operational store, and Monitor uses that host's API without an external telemetry service. The desktop runs in-process; server users access the hosted UI and API over the server's configured transport. Each host and each device keeps its own journal; journals are not synchronised between devices (whether to sync them is an open decision). The browser host's journal location is decided with that host.
- **Ciphertext-only service:** the `*.filesquid.tech` service does not run `filesquid-api` and cannot see domain activity ([ADR 032](./032-hosting-and-privacy-model.md)). It keeps a separate **service-event journal** with the same entry shape for what it does observe (authentication, account and subscription changes, opaque blob uploads and deletions, quota and errors).
- **What is journaled, always:** every domain operation (import, upload, OCFL commit, rename, move, remove, restore, metadata write, C2PA read, mint and sign, export, sharing and permission changes), every job and process state change (queued, started, progress checkpoint, paused, resumed, failed, completed), every authorisation refusal and every credential issue (successful calls are not journaled one by one; the domain operations and process changes they cause are journaled as above), every warning and error, and host start and stop. Debug and trace verbosity is optional and off by default. Exception (owner decision 2026-10-19): a command that changes nothing produces no receipt, no OCFL version and no journal entry ([API-S-019](../../apps/filesquid/crates/api/resources/srs.md)).
- **Never lost:** a domain operation's journal entry is written in the same database transaction as the operational record of that change, or before the operation is acknowledged to the caller. Other events go through a bounded queue with **backpressure**: when the queue is full, the producer waits; events are not dropped. A test MUST prove no loss under load.
- **Content:** RFC 3339 UTC time, host, process or job ID, actor (user or system), operation, outcome, opaque IDs of what was touched, duration, and error codes. Redacted as ADR 014 requires: no file contents, titles, captions, pixels, keys or credentials. Every write path enforces this: an identifier field (actor, operation, process, targets, error code) that is not an opaque token is stored as `redacted`, and the entry is kept. On `filesquid-server`, a user's own activity is visible to that user and to administrators as opaque operational facts, never as another user's private metadata. Administrators use the admin routes below; a user reads only their own entries through a self-scoped journal route that arrives with accounts (Task FS-1.4) and grants none of the other admin routes.
- **Retention and export:** retention is configurable (default: keep everything). The owner or an administrator can export any time range as JSON Lines; a response that reaches the size cap carries an `X-Journal-Next-Cursor` header to continue from. Archiving older journal segments into an OCFL object is an allowed retention mode, so the activity record itself can be preserved.

**3. Live health monitoring.** Each host keeps a **process registry**: every long-running unit (job, worker, import, upload, migration, C2PA scan, sync session, host task) registers with type, state, progress and a heartbeat. A unit whose heartbeat is older than its declared timeout is reported as **stalled**. Health is derived from the registry plus host checks (storage roots reachable and writable, free space against thresholds, operational store reachable, queue depths, recent error rate) and reported as `ok`, `degraded` or `failing` with reasons.
- Admin API (the owner on desktop, administrators on `filesquid-server`): `GET /api/v1/admin/health`, `GET /api/v1/admin/processes`, `GET /api/v1/admin/journal` (filters: time range, actor, operation, outcome, level, process; paged), `GET /api/v1/admin/metrics` (counters and gauges as JSON), and live updates on the event channel topics `process.update`, `journal.entry` and `health.change`.
- `filesquid-server` keeps `GET /metrics` in Prometheus text format from the same counters, for anyone who wants to scrape it.

**4. FileSquid Monitor dashboard (own tooling).** `@filesquid/ui` ships a **Monitor** dashboard as custom elements, built in vanilla TypeScript with Vite: overall health with reasons; live processes and jobs (state, progress, rate, ETA, stalled flags); queues; throughput and error rates over time from journal aggregates; storage roots and free space; and a searchable, filterable journal with live tail and export. Every distribution can include it with its own labels and tokens; ImageSquid reaches it from its More menu. No Grafana, Perses or Loki UI is required.

**5. Optional external sinks (OpenTelemetry).** A host MAY export journal entries and metrics over **OTLP** to an endpoint the owner or administrator configures. Export is off by default. It is **store-and-forward**: the exporter reads the journal through a persisted cursor, so nothing is lost while the sink is down, and no message broker is needed. The repository's Compose file offers a reference `observability` profile with **VictoriaLogs** (OTLP log ingestion at `/insert/opentelemetry/v1/logs`) and **VictoriaMetrics** (scraping `/metrics` with `-promscrape.config`), both Apache-2.0 single binaries with built-in query UIs. They are optional and never bundled into the desktop app.

**6. Removed.** Promtail, Redpanda, Loki, Grafana, Grafana Alloy and Prometheus are removed from `docker-compose.yml` and `config/`. Redpanda's buffering role is covered by the journal and the cursor-based exporter.

## Rationale

- **Requirement (1) needs durable capture.** Stdout plus a scraper loses events whenever nothing is scraping, which on the desktop is always. A transactional journal in FileSquid's own store records every activity in scope on every host, with or without external infrastructure. Successful API calls are left out on purpose: routine polling would flood the journal, and what a call changes is journaled as the operation it causes. Stdout is not a durable record and is not counted as one.
- **Requirement (2) is domain-specific.** Generic infrastructure dashboards see CPU and HTTP codes; they do not know what an import, an OCFL commit or a stalled C2PA scan is. The process registry and Monitor dashboard show FileSquid's own processing directly.
- **Least friction.** No broker, collector or dashboard server to install, upgrade or secure. Self-hosters running a family server get monitoring with the server. Operators who already run an observability platform plug in through OTLP, a vendor-neutral standard.
- **Licences.** The core is MIT FileSquid code. The optional reference sinks are Apache-2.0. No AGPL or BSL component is needed.
- **Preservation fit.** A journal that can be archived as OCFL turns the activity log into part of the preserved record, which suits an archival product.

## Assumptions

- Journal volume at "always" verbosity is small next to file data: one row per operation or state change. Retention modes handle long-running servers. To be measured in Task FS-2.4.
- SQLite in WAL mode and PostgreSQL can absorb the write rate with batching while keeping the transactional rule for domain operations.
- `opentelemetry-otlp` is release-candidate quality for logs and metrics in Rust at writing. The optional exporter pins a version and is tested against VictoriaLogs and VictoriaMetrics in Task FS-3.6.
- VictoriaLogs' OTLP endpoint and VictoriaMetrics' `-promscrape.config` were taken from the VictoriaMetrics documentation ([OTLP ingestion](https://docs.victoriametrics.com/victorialogs/data-ingestion/opentelemetry/), [scraping without vmagent](https://docs.victoriametrics.com/victoriametrics/vmagent/)); they are verified by running the profile in Task FS-3.6.

## Alternatives Considered

- **Inherited stack (collector, Redpanda, Loki, Prometheus, Grafana):** five services, AGPL and BSL components, no domain awareness, and it still does not capture desktop activity. Rejected.
- **OpenTelemetry Collector + VictoriaLogs + VictoriaMetrics as the core (option B as first proposed):** lighter and permissive, but still external infrastructure that the desktop does not have and that cannot see FileSquid's processes. Kept only as the optional sink.
- **All-in-one platforms (OpenObserve AGPL-3.0, SigNoz mixed licence):** one service, but a vendor UI and licence and no domain view. Rejected.
- **In-memory ring buffer only (earlier draft):** drops old events, so it fails requirement (1). Replaced by the journal; the event channel provides the live tail.
- **Text log files with rotation:** violates the stdout-only rule and is hard to query or redact. Rejected.

## Consequences / Implications

Easier: every user, on desktop or server, gets complete activity history and live health without extra software; support can ask for a journal export; the activity record can be preserved as OCFL; operators keep a standards-based path to their own platform. Harder: FileSquid owns the journal schema, its write path (transactional and backpressured), retention, export, the process registry, health rules, the dashboard, and their tests; journal writes add load to the operational store and need batching and measurement; the redaction rules need dedicated tests.

## Related Decisions / Notes

- Supersedes in part: [ADR 014](./014-stdout-observability.md) (collector and stack). Related: [ADR 005](./005-sqlite-local-postgres-hosted.md), [ADR 007](./007-durable-jobs-and-interactive-performance.md), [ADR 029](./029-filesquid-unified-product-and-distributions.md), [ADR 030](./030-filesquid-api-and-transports.md), [ADR 031](./031-vanilla-typescript-ui-and-white-label-theming.md), [ADR 032](./032-hosting-and-privacy-model.md).
- Store placement (amendment 2026-10-18): the SQLite journal store and the data-directory checks are in `filesquid-native` since Task CL-0.1; the server's PostgreSQL journal store is part of SV-F1.2.
- Implementation: Task FS-2.4 (journal, process registry, health, admin API, Monitor dashboard) and Task FS-3.6 (OTLP store-and-forward exporter and the optional VictoriaLogs/VictoriaMetrics profile) in the [implementation plan](../implementation-plan.md).
- Licences checked 2026-10-15 from each project's repository: VictoriaMetrics and VictoriaLogs Apache-2.0, OpenTelemetry Collector and opentelemetry-rust Apache-2.0.
