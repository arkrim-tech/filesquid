# Architecture Decision Record - FileSquid

012-s3-compatible-ciphertext-storage.md

## Title

S3-compatible object storage and resumable multipart for ciphertext

## Status

Accepted

Accepted: 2026-09-16T22:24:57Z. Revised: 2026-09-22T04:34:37Z.

Amended 2026-10-29: resumable upload does not move the hosted acceptor, or the backup stack, onto Elixir/BEAM. The durable offset lives in the client checkpoint and in the object store. A byte-terminating tus front door, if a deployment ever needs one, stays a separate process in front of the same parts.

Amended 2026-10-29 (owner decision): the owner of a Drive enrolls a device public key as permission to upload into that Drive. Each Drive's backup keeps recovery-point copies in more than one geography.

## Context / Requirement Reference

Hosted roots need object storage for ciphertext ([FE-033](../product-vision.md#portable-preservation), [FE-034](../product-vision.md#portable-preservation), [ADR 005](./005-sqlite-local-postgres-hosted.md)). The vision listed hosting provider/geography and cloud frameworks as undecided. Naming a single vendor would lock the stack before operations review.

## Decision

Hosted ciphertext uses an **S3-compatible** object API (put/get/head, listing as required, **multipart upload**, checksums). The application MUST NOT require Amazon-only APIs.

**Conflict guard (required):** completing or replacing an object that is an OCFL head or other canonical pointer MUST use a compare-and-swap. Prefer native conditional writes (`If-Match` / `If-None-Match` / equivalent). If a selected provider lacks them, the application MUST still fence the completion (for example a compare-and-swap record in PostgreSQL that authorizes the object key/generation before the new head is published). Competing multipart completions MUST NOT silently overwrite a canonical head. A provider that cannot support this MUST NOT be used for hosted OCFL.

Resumable uploads are S3 multipart (or an equivalent part protocol on that API). tus MAY be used as a front-door if it terminates into the same object parts; it is not a second media-identity model.

**Resume does not change the runtime (2026-10-29).** A resumable upload is a durable offset and an idempotent completion. That requirement does not make Elixir/BEAM the backup stack, and it does not replace the Axum mailbox ([ADR 004](./004-axum-hosted-ciphertext-mailbox.md)).

- The client MUST persist the checkpoint before sending bytes: the desktop job store, and IndexedDB only on the later browser host. After any interruption the client MUST ask the store for its offset and continue from that offset. The offset MUST survive a reboot of every FileSquid process. Process memory, including a BEAM process, is not that store.
- Preservation bytes go from the client to S3-compatible storage under a scoped presigned URL ([ADR 004](./004-axum-hosted-ciphertext-mailbox.md), [ADR 025](./025-client-side-filesquid-engines.md)). The hosted service authorizes the upload, checks length and checksum, and fences manifest publication. It does not terminate the byte stream. Objects are written before the recovery-point manifest; a manifest that names a missing part is not published.
- Completion MUST be idempotent. An upload the client failed to record as finished is finished when the store reports the offset equal to the length.
- A deployment that must terminate bytes itself (many half-open connections, for hours, on a bad network) MAY put a separate tus process in front of these same parts. That process MUST NOT become a second file identity, MUST NOT parse OCFL, and MUST NOT hold library keys. It does not replace the mailbox, `filesquid-ocfl`, or the desktop.
- The owner of a Drive installation grants upload permission by enrolling another user's device public key (Ed25519, [ADR 009](./009-cryptography-webcrypto-compatible-suite.md)). The mailbox authorises that key for append into that Drive. A password is not that permission. The uploader encrypts before the bytes leave the device. Revoking the key stops later uploads. The key cannot delete recovery points ([F-029](../prd.md#f-029-client-encryption-and-key-custody)).
- Each Drive has its own backup. That backup stores recovery-point ciphertext in more than one geography. One region is not the backup design ([F-031](../prd.md#f-031-encrypted-backup-and-verified-restore), [F-059](../prd.md#f-059-hosted-encrypted-backup-and-sync-service)).

Which providers and which regions remain an **operations ADR** before a hosted pilot. That choice MUST name more than one geography. Replication class and residency stay in the same operations ADR. The technical stack is the API shape, not “AWS” or “OVH.”

Default layout: shared bucket, enforced per-user prefix / OCFL root ([ADR 010](./010-ocfl-1-1-dual-profile.md)).

## Rationale

S3 is the de facto portable object API. Keeping the vendor out of the stack preserves mailbox portability.

The same split answers the runtime question. A supervision tree restarts a crashed handler. It does not remember a byte offset after the node reboots. The client’s checkpoint and the multipart upload in the bucket do. BEAM is the stronger runtime for a server that itself receives chunks from tens of thousands of stalled connections. That is the field-node shape, and it is not this path: the long-lived transfer is between the client and the bucket. The desktop checkpoint, the OCFL writer, the [ADR 009](./009-cryptography-webcrypto-compatible-suite.md) envelopes, and the WebAssembly build stay Rust so one implementation matches on the desktop and in the browser. An Elixir mailbox would be a third runtime beside Rust and the UI toolchain, which [ADR 004](./004-axum-hosted-ciphertext-mailbox.md) and [ADR 013](./013-cargo-svelte-workspaces.md) already refuse. Preemptive processes would matter only if this service terminated the bytes. This record keeps that case as an optional front-door process.

## Assumptions

- At least one S3-compatible provider will pass multipart, checksum, and conditional-write (or equivalent fence) tests before hosted beta.
- PostgreSQL remains the operational catalog ([ADR 005](./005-sqlite-local-postgres-hosted.md)).

## Alternatives Considered

- **Named AWS-only stack:** Rejected as premature vendor lock-in.
- **Azure Blob or GCS native APIs as the only interface:** Rejected; adapters MAY exist later, S3-compatible is the contract.
- **tus as the identity of a file:** Rejected; tus is transport.
- **Elixir/BEAM (Phoenix, Oban) as the backup stack or the upload acceptor, because resume is fundamental:** Rejected. Resume is a property of the checkpoint and the multipart store, not of the VM. The desktop, the OCFL library, and the browser host are outside the BEAM either way.
- **In-memory upload state as the source of the offset:** Rejected. The offset has to be readable after the acceptor process and the client process are both gone.

## Consequences / Implications

Easier: MinIO locally, any compatible host in production. Harder: test matrix across providers; the geographies are still unset, and a pilot names more than one. The FileSquid OCFL library is the client that reads and writes OCFL on this API ([ADR 016](./016-rust-ocfl-library.md)); S3 is not a second, non-OCFL blob store.

Resume tests kill the client and the acceptor at a part boundary, then continue from the store’s offset. A second run of a finished upload is a no-op. A future byte-terminating ingest edge is its own process, with its own operations, and it still must not parse OCFL or hold keys.

## Related Decisions / Notes

- [ADR 004](./004-axum-hosted-ciphertext-mailbox.md), [ADR 005](./005-sqlite-local-postgres-hosted.md), [ADR 009](./009-cryptography-webcrypto-compatible-suite.md), [ADR 010](./010-ocfl-1-1-dual-profile.md), [ADR 013](./013-cargo-svelte-workspaces.md), [ADR 016](./016-rust-ocfl-library.md), [ADR 025](./025-client-side-filesquid-engines.md)
- [tus](https://github.com/tus/tus-resumable-upload-protocol/blob/main/protocol.md) optional front-door only
- [Field media pipeline](../references/FileSquid%20field%20media%20pipeline.html) (the resume rules), [backup design report](../references/FileSquid%20backup%20design%20report.html) (how those rules land in phase F4) and [backup product vision](../references/FileSquid%20backup%20product%20vision.md) (the canvas for that release, under C-025)
