# Architecture Decision Record - FileSquid

037-c2pa-in-release-2.md

## Title

C2PA leaves the FileSquid MVP and release 1 and ships with release 2

## Status

Accepted

Accepted: 2026-10-18. Amends [ADR 035](./035-macos-first-desktop-mvp.md) (the MVP scope it cites from [C-056](../product-vision.md#release-sequence)) and the release-1 module set of [ADR 029](./029-filesquid-unified-product-and-distributions.md) ([C-025](../product-vision.md#release-sequence)). Does not change [ADR 026](./026-filesquid-provenance-engine.md): the `filesquid-c2pa` engine, its design and its isolation rules stay as decided; only its delivery moves.

Amended 2026-10-19 by [ADR 040](./040-prov-o-history.md) (this record's Decision 6; ADR 040 Decision 10): the `filesquid-core` PROV-O types of CO-0.8 are no longer C2PA leftovers; CO-0.11 extends them for the MVP's PROV-O history, which does not depend on the `c2pa` module. Also amended 2026-10-19 by the owner decision recorded in [ADR 038](./038-import-file-record.md): the import record of Decision 5 is the `import` section of each file's `<file-uuid>.metadata.md`.

Noted 2026-10-29: Decision 6 also keeps phase C from delaying encrypted backup (Task FS-4.2), which sits between the MVP and the rest of the first consumer release ([ADR 029](./029-filesquid-unified-product-and-distributions.md)).

Amended 2026-10-29 (owner decision): an upload into a node, Task 5.2 in phase F4, adds a ProofMode-shaped PROV-O record and a C2PA proof signed by the uploading device key (Decision 7). That proof records the upload and cites metadata stripped into the file's Markdown record. It does not move the Content Credentials panel, scans, edits or crate publishing out of phase C. Decisions 2 and 3 still hold for those.

## Context / Requirement Reference

- Requirements: [C-056](../product-vision.md#release-sequence) (FileSquid MVP scope), [C-025](../product-vision.md#release-sequence) (release sequence), [PRD F-043](../prd.md#f-043-c2pa-records-and-action-disclosure), [PRD F-054](../prd.md#f-054-c2pa-module), [PRD RC-009](../prd.md#rc-009-source-evidence-and-c2pa-lifecycle) and the [release order and dependency gates](../prd.md#release-order-and-dependency-gates).
- Until this decision the MVP carried C2PA read, validation and an unsigned import-time record, with the Content Credentials routes and panel; release 1 added signing, embedding, edits, scans and the published crate. That is about 118 planned hours inside the MVP (global Tasks 0.17 and 0.18, API-F1.5, UI-F2.5) and 86 more in release 1 (Tasks FS-3.12 and 2.6, CP-F3.1). The `filesquid-c2pa` engine is written from scratch ([ADR 026](./026-filesquid-provenance-engine.md)), so it carries specification, signature, certificate-chain and oracle risk on the MVP's critical path.
- The owner decided on 2026-10-18 to reach the MVP sooner: basic file management on top of OCFL is the MVP's story, and the C2PA narrative comes later, in release 2 (the ImageSquid release).

## Decision

1. **No C2PA in the MVP.** The MVP ([C-056](../product-vision.md#release-sequence)) ships no C2PA read, validation or import-time record, no Content Credentials routes and no Content Credentials panel. No MVP host and no MVP build of `filesquid-api` links `filesquid-c2pa`.
2. **No C2PA in release 1.** The first consumer release ships the `ocfl` and `metadata` modules. C2PA signing, embedding, edits, scans and the published `filesquid-c2pa` crate move with the rest of the module.
3. **C2PA ships with release 2.** The whole `c2pa` module, its API routes, the Content Credentials panel and the RC-009 evidence ship with the ImageSquid release, as FileSquid features behind the API ([ADR 029](./029-filesquid-unified-product-and-distributions.md)). C2PA is for image and media files, so it arrives with the image distribution: in release 2 `filesquid-c2pa` is added to FileSquid (linked by `filesquid-api` for the first time), and its functionality reaches the ImageSquid distribution and UI only through the single `filesquid-api`, never by a direct module call ([ADR 030](./030-filesquid-api-and-transports.md), [ADR 031](./031-vanilla-typescript-ui-and-white-label-theming.md)). The implementation plan collects them in phase C (Content Credentials), which precedes phase I1 and closes with a Content Credentials gate.
4. **Records for files imported earlier.** When the `c2pa` module is enabled for a library, FileSquid MUST give every file imported before then an initial record through a resumable backfill job. That record states its own creation time and cites the file's import record ([ADR 038](./038-import-file-record.md)) and OCFL version (`created` time and SHA-256 digest) as the import evidence (or, for a file with no import record, the OCFL version alone with that absence stated); it MUST NOT claim FileSquid import time ([F-043](../prd.md#f-043-c2pa-records-and-action-disclosure)). From then on every import initialises its record at import time, as F-043 requires.
5. **An import record instead, from the MVP on.** In place of the C2PA record, every import in the MVP and release 1 commits an import record of source-system and host facts ([ADR 038](./038-import-file-record.md)). The `generator:` search operator, which reads C2PA claim generators, is not part of the MVP search grammar; it joins the grammar with the `c2pa` module.
6. **Work already done stays.** CP-0.1 to CP-0.3 (crate foundation, manifest-store parsing, hard bindings and JPEG/PNG handlers) and the `filesquid-core` C2PA and PROV-O record types (CO-0.8) remain in the repository and in the workspace test run. The remaining C2PA rows MAY start after the MVP gate and MAY run beside backup and release-1 work, and MUST NOT delay the MVP gate, the backup gate (Task FS-4.2) or the release-1 gate.
7. **A node upload carries a proof.** An upload into a FileSquid node (phase F4, Task 5.2) MUST add a PROV-O record and a C2PA proof on the ProofMode model, signed by the uploading device key. The proof binds the content hash of the stored bytes (the content after embedded metadata and sidecars have been moved into `<file-uuid>.metadata.md`), the device, the time, and location when the device has it. It records the upload and cites the stripped metadata. It MUST NOT claim FileSquid witnessed the original capture. This proof is not the Content Credentials panel, a collection scan, an edit, or publication of the `filesquid-c2pa` crate.

## Rationale

- The MVP's purpose is to show durable, recoverable, versioned file management on OCFL. C2PA adds a second, complex story that the first users do not need to judge that.
- Taking the from-scratch C2PA engine off the critical path removes its specification and cryptography risk from the MVP and from release 1.
- The panel-slot architecture ([ADR 031](./031-vanilla-typescript-ui-and-white-label-theming.md)) and the one-owner rule for file facts ([C-055](../product-vision.md#module-rules)) let the Content Credentials panel and routes join later without changing the browser, the other panels or the OCFL layout.
- C2PA's strongest audience is photo and media work, which is what the ImageSquid release delivers.
- Not linking `filesquid-c2pa` shrinks the MVP DMG's linked inventory, and with it the C2PA part of the [ADR 011](./011-native-media-adapters-and-ffmpeg-license.md) licence review; the rest of that review still gates the MVP.

## Assumptions

- OCFL version metadata (`created`, digests, `user`, `message`) is enough evidence to give files imported before the module shipped honest initial records without back-dating them.
- MVP and release-1 users do not need Content Credentials to judge FileSquid's file management.
- No regulatory or partner commitment requires C2PA in the MVP or release 1.

## Alternatives Considered

- **Keep read, validate and unsigned import records in the MVP (ADR 035 scope).** Rejected by the owner: it keeps the C2PA engine on the MVP critical path.
- **Move C2PA to release 1 only.** Rejected by the owner: release 2 is where the C2PA narrative belongs, and release 1 then stays focused on the server host, sharing and installers.
- **Keep only unsigned import records in the MVP, without the engine's read and validate.** Rejected: the records still need the manifest builder (CP-0.6), which depends on the parser and validation work, so it saves little.
- **Drop the backfill and start records only at the next import.** Rejected: files imported before release 2 would have no record at all, which contradicts F-043's every-file intent.

## Consequences / Implications

- Easier: the MVP critical path loses Tasks 0.17 and 0.18 and the credential routes and panel; 118 planned hours leave the MVP, of which 46 were already done. The import record that replaces the C2PA record ([ADR 038](./038-import-file-record.md)) adds 20, so the MVP falls from about 856 to 758 hours.
- Easier: release 1 loses 86 hours and its RC-009 gate.
- Harder: release 2 carries more work (phase C, about 221 hours) and a backfill job whose records must be clearly distinguishable from import-time records.
- Harder: until release 2, a library has no Content Credentials. The MVP and release-1 privacy and feature statements MUST NOT claim C2PA, and the F-040 register lists Content Credentials as unsupported until the phase C gate.
- The Phase F3 MVP gate and the release-1 gate no longer demonstrate RC-009; the Content Credentials gate (Task FS-C.3) does.
- The pre-existing inconsistency where an MVP row (API-F1.5) depended on a post-MVP signing row (CP-0.7) disappears, because both now sit in phase C.
- The owner-maintained "Current milestone" in [pr-review-loop.md](../../.agents/rules/pr-review-loop.md) names phases F0 to F3 as its scope, so phase C tasks fall outside the current milestone without any change to that section.

## Related Decisions / Notes

- [ADR 015](./015-prov-o-jsonld-and-c2pa-sdk.md) (PROV-O and C2PA records), [ADR 026](./026-filesquid-provenance-engine.md) (the C2PA engine), [ADR 029](./029-filesquid-unified-product-and-distributions.md) (release order), [ADR 031](./031-vanilla-typescript-ui-and-white-label-theming.md) (panel slots), [ADR 035](./035-macos-first-desktop-mvp.md) (MVP scope), [ADR 011](./011-native-media-adapters-and-ffmpeg-license.md) (installer licence review), [ADR 038](./038-import-file-record.md) (import record), [ADR 030](./030-filesquid-api-and-transports.md) (one API).
- Open: the owner MAY pull phase C earlier (for example beside release 1) once the MVP has shipped; that changes only the implementation plan.
- Decided (owner, 2026-10-18): in release 2 the default FileSquid distribution also gets a Credentials tab: its sidebar mounts the Content Credentials panel, as the ImageSquid inspector does in its C2PA tab. The MVP and release-1 mockups show no Credentials tab.
