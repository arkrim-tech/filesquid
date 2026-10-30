# Architecture Decision Record - FileSquid

004-axum-hosted-ciphertext-mailbox.md

## Title

Axum hosted service as ciphertext mailbox and static web host

## Status

Accepted

Accepted: 2026-09-16T01:01:07Z. Revised: 2026-09-22T04:34:37Z. Revised: 2026-10-15T03:34:43Z. Amended by [ADR 025](./025-client-side-filesquid-engines.md): clients write OCFL objects to S3-compatible storage; Axum authorizes keys and fences heads but never links or parses OCFL.

Amended 2026-10-15 by [ADR 032](./032-hosting-and-privacy-model.md): this ciphertext mailbox is re-scoped to the paid `*.filesquid.tech` backup and sync service. The owner-operated `filesquid-server` is a separate deployment type that MAY link the modules and process plaintext. The former `apps/backend` scaffold is now `filesquid-server` at `apps/filesquid/crates/server/`.

Noted 2026-10-29: resumable upload does not reopen the Elixir/BEAM alternative. The durable offset is specified in [ADR 012](./012-s3-compatible-ciphertext-storage.md).

## Context / Requirement Reference

Novice mental model: a web app is a REST backend that holds files plus an HTML/JS frontend that asks for collections. FileSquid's hosted privacy rules forbid that. The hosted service MUST NOT receive library keys or ordinary private plaintext (titles, captions, search, pixels) ([T-001](../product-vision.md#client-processing-and-hosted-mailbox), [U-20](../product-vision.md#one-api-hosts-and-distributions), [FE-033](../product-vision.md#portable-preservation)). The browser MUST unlock, browse and export available uploaded content with desktops off ([FE-017](../product-vision.md#cross-platform-access), [T-002](../product-vision.md#client-processing-and-hosted-mailbox)).

Phoenix/LiveView and FastAPI were investigated as that "REST photo backend." They are the wrong shape.

## Decision

The hosted deployable is an **Axum** HTTP service on Tokio, using **SQLx** for PostgreSQL. It is the **mailbox**, not the library application.

It MAY:

- authenticate accounts and devices
- store and resume **ciphertext** objects
- record operational metadata (opaque IDs, sizes, quotas, membership, billing events)
- run hosted durable jobs that never require plaintext media
- serve the compiled Svelte UI as **static files** (HTML, CSS and JavaScript bytes already produced by the Svelte build)

It MUST NOT:

- link `filesquid-ocfl` or `filesquid-metadata`, or parse OCFL inventories ([ADR 025](./025-client-side-filesquid-engines.md));

- decrypt library content
- return plaintext collection titles, captions, faces, GPS or search hits
- run private collection search or a plaintext catalog
- run libvips/FFmpeg on private originals as an operator convenience
- **server-render** private pages (no templates, LiveView, or SSR that insert collection names, captions or pixels into HTML)
- substitute for the desktop Rust host

**Static file hosting is not a library backend.** Axum MAY `GET /` and return `index.html` plus hashed JS/CSS, the same way any static file server would. The browser then runs that JavaScript, unlocks keys locally, and paints the library views. Axum MUST NOT generate per-user HTML that already contains private library state. A blank or shell SPA document is expected; private meaning appears only after client decrypt.

The web app therefore has **two** "backends" with different jobs:

| Client call | Who answers | What comes back |
|---|---|---|
| Login, devices, billing, quota | Axum | Operational JSON with no private file meaning |
| Upload/download preservation bytes | Client `filesquid-ocfl` (desktop or browser WASM) to S3-compatible storage; Axum issues scoped presigned URLs and fences head publication ([ADR 025](./025-client-side-filesquid-engines.md)) | Ciphertext parts, resume tokens and receipts; Axum never parses OCFL |
| Show collection, search captions, star a file | Browser (Svelte + WASM/WebCrypto) after unlock, or desktop Rust via Tauri | Private meaning never computed on Axum |

Exact Axum and SQLx crate versions are **not** frozen here. Use current stable lines at first implementation (at writing: Axum 0.8.x, SQLx 0.9.x) and pin them in the service lockfile.

Hosted jobs use Rust plus PostgreSQL, not Oban or Celery.

This mailbox is **not** by itself a synchronization or mutation protocol. Before implementing hosted writes, the following contracts MUST be adopted (later ADRs may refine them; they MUST NOT be invented ad hoc in Axum handlers): authenticated devices, signed operations, idempotent operation IDs, duplicate delivery, conflicting updates, revocation, deletion/tombstone replay, and recoverable upload completion ([T-001](../product-vision.md#client-processing-and-hosted-mailbox), [ADR 005](./005-sqlite-local-postgres-hosted.md), [ADR 007](./007-durable-jobs-and-interactive-performance.md)).

**Privacy guarantee (precise):** ordinary hosted operation has neither private plaintext nor the keys needed to decrypt it. That is distinct from explicitly authorized [FE-042](../product-vision.md#paid-c2pa-publication-watermarking-and-ai-disclosure) publication and [FE-049](../product-vision.md#ai-the-user-chooses) BYO-AI disclosure, which send **selected** content to a named recipient and never grant the mailbox operator general library keys ([T-003](../product-vision.md#client-processing-and-hosted-mailbox)).

**Browser-delivery trust:** Axum (or any host that serves the SPA) can distribute JavaScript that later sees unlocked content in the browser. Static hosting does not remove that supply-chain threat. Release signing and integrity of served assets remain required ([T-034](../product-vision.md#later-gates)); they are not solved by ciphertext-at-rest.

## Rationale

Axum is the HTTP face of the same Rust ecosystem as the desktop host. That avoids a second language. Ordinary hosted operation having neither private plaintext nor decryption keys is the mailbox guarantee. Serving static files from Axum is optional convenience so one hosted process can deliver the SPA shell and the ciphertext APIs. It is not permission to server-render private HTML, to treat served JavaScript as trusted by default, or to link libvips into the server because the desktop already has it.

FastAPI would still be a mailbox if used correctly, but it would be a second backend language. LiveView would push private state onto the server. Both fail the no-redundancy and privacy tests.

## Assumptions

- Clients hold keys; recovery envelopes follow [T-001](../product-vision.md#client-processing-and-hosted-mailbox) and [T-007](../product-vision.md#canonical-ocfl-state-and-evidence) and do not require the operator to read files.
- Object storage is S3-compatible ciphertext ([ADR 005](./005-sqlite-local-postgres-hosted.md)).
- Browser WASM/WebCrypto can open envelopes for the replica, and browser WASM runs `filesquid-ocfl` and `filesquid-metadata` against the encrypted hosted root ([ADR 008](./008-browser-wasm-scope.md), [ADR 025](./025-client-side-filesquid-engines.md)).
- Public marketing HTML, if any, never embeds private library state.
- Integrity of hosted SPA assets is a release/security ADR still to be written; this ADR does not claim that static files are uncompromisable.

## Alternatives Considered

- **Phoenix/Elixir/Oban:** Strong hosted jobs. Rejected as a second application runtime beside Rust. Resumable upload does not reopen this alternative: the durable offset is the client checkpoint plus S3 multipart, not a BEAM process ([ADR 012](./012-s3-compatible-ciphertext-storage.md)).
- **LiveView for the web UI:** Would need plaintext on the server for a conventional collection browse. Rejected.
- **FastAPI:** Acceptable mailbox in isolation; rejected as a parallel stack once Rust is the application language.
- **Server-side thumbnail workers on plaintext:** Forbidden by the privacy model.
- **Desktop required for browser use:** Forbidden by [T-002](../product-vision.md#client-processing-and-hosted-mailbox).

## Consequences / Implications

Easier: one language; clear threat boundary; static UI hosting. Harder: search, collections and edits in the browser are client features; engineers MUST NOT "just add a JSON title field" to Axum. Training and API review must treat plaintext leakage as a product defect. Scale-to-zero applies to eligible compute, not to durable ciphertext storage.

## Related Decisions / Notes

- [ADR 001](./001-shared-rust-core-with-tauri-and-axum-hosts.md), [ADR 003](./003-svelte-client-rendered-ui.md), [ADR 005](./005-sqlite-local-postgres-hosted.md), [ADR 008](./008-browser-wasm-scope.md)
- This ADR supersedes the former Phoenix/Elixir hosted-control-plane candidate.
