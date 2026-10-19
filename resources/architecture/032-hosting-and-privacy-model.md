# Architecture Decision Record - FileSquid

032-hosting-and-privacy-model.md

## Title

Three FileSquid host types with a stated privacy boundary each: desktop, owner-operated multi-user filesquid-server, and the ciphertext-only hosted backup and sync service at *.filesquid.tech; the encrypted browser host stays on the roadmap

## Status

Accepted

Accepted: 2026-10-15. Amends [ADR 004](./004-axum-hosted-ciphertext-mailbox.md) (the ciphertext mailbox is re-scoped to the `*.filesquid.tech` service), [ADR 008](./008-browser-wasm-scope.md) and [ADR 025](./025-client-side-filesquid-engines.md) (modules MAY now run in `filesquid-server`; the browser host moves to the roadmap; modules still never process private content on the ciphertext-only service).

Note 2026-10-19 (owner decision, option 1a): the Decision 6 statements and their wording are unchanged, but the UI need not show them as permanent header text. The header MUST show a compact host indicator (icon and short host label) whose popover or tooltip carries the full statement, and the Settings page's Privacy section MUST show it as well ([UI-S-004 and UI-S-018](../../apps/filesquid/ui/resources/srs.md)).

## Context / Requirement Reference

Under [ADR 025](./025-client-side-filesquid-engines.md) the engines ran only on clients (desktop and browser WebAssembly), never on a server, and the only server was the Axum ciphertext mailbox ([ADR 004](./004-axum-hosted-ciphertext-mailbox.md)). The owner now wants:

- a desktop host and a multi-user `filesquid-server` host from the first release, and an encrypted browser host afterwards ([FE-075](../product-vision.md#platform-api-and-hosts), [U-19](../product-vision.md#one-api-hosts-and-distributions), [U-22](../product-vision.md#one-api-hosts-and-distributions));
- paid ciphertext-only backup and sync at `*.filesquid.tech`, before the ImageSquid release ([FE-078](../product-vision.md#platform-api-and-hosts), [U-21](../product-vision.md#one-api-hosts-and-distributions), [C-031](../product-vision.md#platforms-originals-and-service-limits), [T-054](../product-vision.md#api-and-hosts));
- multi-user accounts, groups and collection permissions on `filesquid-server` ([U-06](../product-vision.md#platform-ownership-and-access), [T-053](../product-vision.md#api-and-hosts));
- a precise privacy statement per host, shown in the UI ([U-20](../product-vision.md#one-api-hosts-and-distributions), [T-001](../product-vision.md#client-processing-and-hosted-mailbox), [C-053](../product-vision.md#module-rules)).

A server that runs the modules on plaintext is a second privacy posture. It MUST be stated so precisely that it cannot be confused with the zero-knowledge hosted service.

## Decision

**1. Host types.**

| Host | Runs the FileSquid API and modules | Canonical store | Who can read private files | When |
|---|---|---|---|---|
| **Desktop** (`filesquid-desktop`, Tauri 2) | Yes, native, in-process ([ADR 030](./030-filesquid-api-and-transports.md)) | Owner-readable local OCFL root ([ADR 010](./010-ocfl-1-1-dual-profile.md)) | The owner, on their own device | FileSquid release 1 |
| **Owner-operated server** (`filesquid-server`, Axum) | Yes, native | Local filesystem or S3-compatible OCFL root chosen by the operator | Its users, within their permissions, **and the operator of that server** | FileSquid release 1 |
| **Hosted backup and sync** (`*.filesquid.tech`, paid) | **No.** Ciphertext storage and relay only | Encrypted hosted OCFL profile, one root per user ([ADR 010](./010-ocfl-1-1-dual-profile.md), [ADR 012](./012-s3-compatible-ciphertext-storage.md)) | Nobody at the service: it holds ciphertext and no keys | Before the ImageSquid release (plan phase F4) |
| **Encrypted browser host** (roadmap) | Yes, as WebAssembly in the user's browser | Encrypted hosted OCFL profile; browser storage is a cache only | The owner, in their own browser after unlock | Roadmap (plan phase F5) |

**2. Desktop.** The API runs in-process with no TCP listener ([ADR 002](./002-tauri2-desktop-shell.md), [ADR 030](./030-filesquid-api-and-transports.md)). Native media workers run out of process. The desktop MAY back up and sync to `*.filesquid.tech` by encrypting before upload.

**3. Owner-operated `filesquid-server`.** This is a new, explicit deployment type for families, teams and institutions that run their own server (Nextcloud-style).

- It links `filesquid-api` and the modules and MAY process plaintext. Its operator can read every file it stores.
- It is multi-user from the start: user accounts, groups, user- and group-owned libraries, and collection-level permissions, enforced by `filesquid-api` on every call ([ADR 030](./030-filesquid-api-and-transports.md)). Group membership MUST NOT grant ownership of another member's libraries.
- It serves the API and one selected distribution's static UI from one process (`FILESQUID_UI_DIR`). It MUST NOT server-render private pages; the UI is static files, as in [ADR 004](./004-axum-hosted-ciphertext-mailbox.md).
- OCFL objects stay canonical. Catalogs and indexes MUST be rebuildable from them; operational data (accounts, sessions, permissions) lives in an operational store. The server datastore choice is open.
- Native media processing, when it arrives, runs in out-of-process workers, not on the Tokio runtime ([U-32](../product-vision.md#one-api-hosts-and-distributions)).

**4. Hosted backup and sync at `*.filesquid.tech`.** This is the ADR 004 ciphertext mailbox, re-scoped to FileSquid's paid service.

- It MUST store and relay ciphertext only. It MUST NOT hold library keys or private plaintext, and MUST NOT return or render private meaning.
- It MUST NOT link `filesquid-api`'s private-processing code, the `ocfl`, `metadata` or `c2pa` modules, or native media libraries. CI MUST fail its build if they appear in its dependency graph ([U-30](../product-vision.md#one-api-hosts-and-distributions)). Cargo features are not the privacy wall; this is a separate binary with a separate dependency graph ([U-31](../product-vision.md#one-api-hosts-and-distributions)).
- It MUST NOT parse OCFL. It authenticates accounts and devices, issues scoped presigned object URLs, checks object lengths, checksums and receipts, and fences head publication (fenced `vN` reservation before content, then compare-and-swap on the head) ([ADR 012](./012-s3-compatible-ciphertext-storage.md), [ADR 016](./016-rust-ocfl-library.md), [ADR 025](./025-client-side-filesquid-engines.md) Decision 5).
- Backup (independent retained recovery points) and sync are separate guarantees ([C-003](../product-vision.md#platforms-originals-and-service-limits)).
- Clients that write to it encrypt before upload: the desktop now, the encrypted browser host later. A `filesquid-server` MAY also use it as a backup target, encrypting before upload.
- It is a separate deployable from `filesquid-server`. Whether it is a build of the same Axum crate with a restricted dependency set or its own crate is decided when it is implemented, subject to the CI check above.

**5. Encrypted browser host (roadmap).** The FileSquid API and the `ocfl`, `metadata` and `c2pa` modules compiled to `wasm32-unknown-unknown` run in the user's browser against the encrypted hosted root, reached through a Service Worker or a worker plus a client shim ([ADR 030](./030-filesquid-api-and-transports.md)). The WASM build requirements on the modules from [ADR 025](./025-client-side-filesquid-engines.md) and [C-053](../product-vision.md#module-rules) stay in force now, so this host remains reachable. Codecs stay out of WASM ([ADR 008](./008-browser-wasm-scope.md)).

**6. Privacy statements.** Product, documentation and UI MUST use these statements and no stronger ones:

- **Desktop:** "Your files stay on this device. Only you can read them." When backup or sync to `*.filesquid.tech` is on, the UI MUST add: "Backup copies are encrypted on this device before upload."
- **Your own server:** "Your files are stored on a server run by *operator*. That operator can read them. Other users see only what they are permitted to."
- **`*.filesquid.tech` backup and sync:** "Your files are encrypted on your device before upload. FileSquid's service stores ciphertext and never has your keys." This does not hide service metadata (account, sizes, timing) ([C-004](../product-vision.md#platforms-originals-and-service-limits)).
- **Encrypted browser host:** "Your files are decrypted only in this browser."

Explicitly authorised sends (the owner's own AI provider, paid C2PA publication) remain scoped exceptions under [T-003](../product-vision.md#client-processing-and-hosted-mailbox) and never grant the service general keys.

**7. The UI shows the host.** Every distribution UI MUST show which host type it is connected to and, for a server, who operates it. The API exposes the host type and operator name; the UI MUST NOT infer them.

## Rationale

People who self-host expect the server to work on their files, as Nextcloud does. Refusing that would push them to another product. Allowing it without saying so would weaken the zero-knowledge promise. Naming each host and stating who can read files keeps both promises true.

Keeping the paid hosted service ciphertext-only preserves ADR 004's guarantee, which is the basis of the paid core ([C-031](../product-vision.md#platforms-originals-and-service-limits)). Keeping the WASM build rules now costs little and keeps the browser host possible without a rewrite.

## Assumptions

- Owner-operated servers are run by people the users already trust (family, employer, institution). FileSquid does not protect users from their own operator.
- The S3-compatible providers chosen under ADR 012 support presigned URLs and either conditional writes or a PostgreSQL compare-and-swap fence.
- Crypto suite and key custody follow [ADR 009](./009-cryptography-webcrypto-compatible-suite.md). The independent crypto-protocol audit, provider and region, payment processor and GDPR role analysis remain gates before `*.filesquid.tech` takes paying users ([Constraint requirements](../product-vision.md#constraint-requirements)).
- Whether the encrypted browser host is required before the ImageSquid release is not decided; the default is that it is not.

## Alternatives Considered

- **Engines never on any server (ADR 025 position):** Rules out self-hosted multi-user deployments, which the owner wants. Rejected.
- **Run the modules on `*.filesquid.tech` too:** The service would need plaintext and keys. Violates U-20, U-21 and T-054. Rejected.
- **Single-user `filesquid-server`:** Families and teams need shared and private libraries on one server. Rejected.
- **Build the encrypted browser host before backup and sync:** Larger, riskier work ahead of the paid core. Deferred to the roadmap.
- **One hosted product that is sometimes plaintext and sometimes ciphertext:** Users could not tell which promise applies. Rejected; host types are separate deployables with separate statements.

## Consequences / Implications

Easier: self-hosters get a full server; the paid service keeps a simple, auditable promise; each host has a sentence the UI can show.

Harder: two server deployables with different dependency rules and CI checks; `filesquid-server` needs accounts, groups, permissions and an operational store; the UI must carry host and operator information; documentation must never describe `filesquid-server` as zero-knowledge. ADR 004's "Axum MUST NOT link the engines" now applies to the `*.filesquid.tech` service only.

## Related Decisions / Notes

- Amends: [ADR 004](./004-axum-hosted-ciphertext-mailbox.md), [ADR 008](./008-browser-wasm-scope.md), [ADR 025](./025-client-side-filesquid-engines.md).
- Related: [ADR 001](./001-shared-rust-core-with-tauri-and-axum-hosts.md), [ADR 002](./002-tauri2-desktop-shell.md), [ADR 005](./005-sqlite-local-postgres-hosted.md), [ADR 009](./009-cryptography-webcrypto-compatible-suite.md), [ADR 010](./010-ocfl-1-1-dual-profile.md), [ADR 012](./012-s3-compatible-ciphertext-storage.md), [ADR 029](./029-filesquid-unified-product-and-distributions.md), [ADR 030](./030-filesquid-api-and-transports.md), [ADR 031](./031-vanilla-typescript-ui-and-white-label-theming.md).
- Requirements: [FE-075, FE-078](../product-vision.md#platform-api-and-hosts), [U-19–U-23, U-30, U-31](../product-vision.md#one-api-hosts-and-distributions), [U-06](../product-vision.md#platform-ownership-and-access), [T-001, T-003](../product-vision.md#client-processing-and-hosted-mailbox), [T-053, T-054](../product-vision.md#api-and-hosts), [C-053](../product-vision.md#module-rules).
- Decided 2026-10-15: ImageSquid sells backup and sync under its own brand as a white label of this service ([C-031](../product-vision.md#platforms-originals-and-service-limits)). Still open: server datastore and hosting region.
- PRD: [F-057](../prd.md#f-057-desktop-and-server-hosts), [F-059](../prd.md#f-059-hosted-encrypted-backup-and-sync-service).
