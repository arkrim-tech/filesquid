---
trigger: always_on
description: Ensure every FileSquid host logs JSON to stdout, records 100% of activity in the activity journal, and exposes live health through FileSquid's own monitoring.
copyright: Pieter Van Garderen (2026)
license: CC-BY
---

# Logging, Activity Journal and Health Monitoring Enforcement

**Purpose:** This file instructs the AI agent how to implement logging and monitoring to satisfy the "Activity Logging & Live Health Monitoring" principle in `AGENTS.md` and [ADR 033](../../resources/architecture/033-activity-journal-and-live-health-monitoring.md). The owner's requirements are **100% logging of all activity** and **live health monitoring of the system and its processes**, delivered by FileSquid's own tooling rather than an external telemetry stack.

## Execution Requirements

### 1. Stdout only, no log files
- Instrument Rust code with `tracing` and install `tracing-subscriber` with a JSON formatter writing unbuffered to `stdout` (RFC 3339 UTC timestamps).
- MUST NOT write text log files: no `tracing-appender` file guards, `log4rs`, `flexi_logger`, `simplelog` file targets, Tauri log-plugin file targets, or code that creates `*.log` files. `scripts/verify_centralized_logging.sh` rejects these in staged files.
- TypeScript UIs log to the browser console only; anything that must be recorded goes through the FileSquid API so the host journals it.

### 2. Activity journal (100% logging)
- Every domain operation (import, upload, OCFL commit, rename, move, remove, restore, metadata write, C2PA read/mint/sign, export, sharing and permission changes), every job and process state change, every authorisation refusal and every credential issue (not each successful call), host start and stop, every warning and every error MUST be written to the activity journal in the host's operational store (SQLite on desktop, PostgreSQL on servers).
- Domain operations MUST be journaled in the same database transaction as the change, or before the change is acknowledged to the caller. Other events MUST go through a bounded queue with backpressure: producers wait; events are never dropped. Write a test that proves no loss under load.
- An event class MUST NOT be exempted from the journal, or sampled, aggregated away or summarised in place of its own entries, for volume or performance reasons. This includes validation results such as OCFL warning findings (`W###`) and their closures. Batched transactional writes through the bounded backpressure queue are the only permitted relief. A known gap is a defect to fix, never a recorded exception (owner decision, 2026-10-19).
- Two owner-recorded exceptions (owner decisions, 2026-10-19), and no others:
  - A command or catalog index refresh that succeeds without changing anything produces no receipt, no OCFL version and no journal entry, because no activity took place. A refused or failed one is still journaled, with outcome `failed`.
  - The single unavoidable gap in the journal: the browser keeps undelivered client events in a bounded in-memory queue that spills to the page's IndexedDB store, persists them at page close and delivers them on the next open, so none is discarded. The gap is storage that is unavailable or cleared: when IndexedDB is unavailable or refuses a write (private mode, quota refused), the client counts the events it could not keep in a single page-close `ui.console.overflow` entry; events lost because the person cleared the site's data cannot be counted. Every delivered client event is its own entry, and no other undelivered event may be counted instead of journaled ([ADR 033](../../resources/architecture/033-activity-journal-and-live-health-monitoring.md) Decision 7).
- Entries carry time (RFC 3339 UTC), host, process or job ID, actor, operation, outcome, opaque IDs, duration and error code. They MUST be redacted: no file contents, titles, captions, pixels, keys or credentials. Write a redaction test for every new event type.
- Retention is configurable (default: keep everything); export is JSON Lines; archiving older segments into an OCFL object is an allowed retention mode.

### 3. Live health monitoring
- Every long-running process (job, worker, import, upload, migration, C2PA scan, sync session, host task) MUST register in the process registry with type, state, progress and heartbeat, and MUST update it as it runs. A heartbeat older than the process's declared timeout marks it stalled.
- Expose health (`ok` / `degraded` / `failing` with reasons), processes, journal and metrics through the FileSquid API admin routes, and push changes on the event channel. Only the desktop owner or server administrators may read them.
- Surface everything in the FileSquid **Monitor** dashboard (`@filesquid/ui`, vanilla TypeScript custom elements). Do not add Grafana, Perses or another dashboard server.
- `filesquid-server` keeps `GET /metrics` (Prometheus text) generated from the same counters as the JSON metrics route.

### 4. Optional external sinks
- OTLP export of journal entries and metrics is optional and off by default. It MUST be store-and-forward from the journal through a persisted cursor, so nothing is lost while the sink is down. Do not add a message broker.
- The root `docker-compose.yml` `observability` profile provides VictoriaLogs and VictoriaMetrics as reference sinks (`docker compose --profile observability up -d`). Do not add Promtail, Redpanda, Loki, Grafana, Alloy or Prometheus, and never bundle a telemetry server into the Tauri desktop app.

### 5. README Documentation
- Keep the root `README.md` `## View logs` section (under `# Setup Development Environment`) current: where stdout JSON appears, how to open the Monitor dashboard and export the journal, and how to start and query the optional `observability` profile. Every command written there MUST have been executed first (`command-verification.md`).
