# Project Requirements Document: FileSquid

This PRD covers the FileSquid platform and its distributions: the FileSquid default distribution, ImageSquid (photo and video) and later Arkrim (digital archives).

Status: Derived from the [FileSquid vision](./product-vision.md). ImageSquid's photo-specific requirements are in the ImageSquid PRD under the same F- IDs.

This document specifies the full intended product, not implemented features. Design allocation is in the [System Architecture](./architecture.md). Every requirement ID in the FileSquid vision is allocated in [Vision allocation](#vision-allocation); the ImageSquid vision is allocated in the ImageSquid PRD. F-047 has a separate later institutional gate and is not a FileSquid release, ImageSquid release or Photos-parity dependency. Internal names in this PRD are **repository**, **library**, **collection**, **virtual collection**, **virtual exhibit** and **file**. User stories that describe an ImageSquid screen MAY use the ImageSquid labels repo, library, album, gallery, exhibit, photo, image or video.

# Executive Summary

FileSquid MUST provide a free, open-source file platform that keeps files in OCFL 1.1, with Dublin Core metadata and a W3C PROV-O history of every version and change attached ([F-067](#f-067-prov-o-history-of-every-version-and-change)), and C2PA Content Credentials once the `c2pa` module is enabled ([F-043](#f-043-c2pa-records-and-action-disclosure), [F-054](#f-054-c2pa-module)), on the owner's computer, on a server the owner or their organisation runs, or in an encrypted cloud the owner can leave at any time ([FileSquid vision](./product-vision.md#product-vision)). FileSquid is one Rust backend with one versioned API for every feature ([F-056](#f-056-single-filesquid-api)). It runs in a desktop host (macOS first, then Linux and Windows) from the MVP and a multi-user `filesquid-server` host from the first consumer release; the encrypted browser host and an installable mobile-web interface with visible capability limits follow ([F-057](#f-057-desktop-and-server-hosts), [FE-017](./product-vision.md#cross-platform-access)). Every user interface is vanilla TypeScript over that API. A distribution supplies views, design tokens, a label table, brand assets and a module set, and contains no backend code ([F-058](#f-058-white-label-distributions)).

The release order is fixed ([F-050](#f-050-filesquid-platform-and-distributions)). FileSquid, with its default Drive-style UI and the `ocfl` and `metadata` modules on desktop and `filesquid-server`, is the first consumer release. It is preceded by the FileSquid MVP, an early-access build on the macOS desktop host only ([C-056](./product-vision.md#release-sequence)). Paid ciphertext-only encrypted backup and sync at `*.filesquid.tech` follows and MUST precede ImageSquid ([F-059](#f-059-hosted-encrypted-backup-and-sync-service)). ImageSquid, the photo and video distribution, is the second consumer release; the FileSquid `c2pa` module (Content Credentials) ships with it ([C-025](./product-vision.md#release-sequence)). Arkrim, the digital-archives distribution with paid curation jobs, Parquet datasets and FAIR reporting ([F-060](#f-060-archival-curation-jobs-and-datasets)), comes later.

FileSquid MUST be highly performant on every host and distribution, with near-instant controls and prepared media, useful progressive previews and responsive browsing during background work on declared supported profiles. Default managed local collections MUST expose documented canonical files and metadata through ordinary filesystem tools, backed by OCFL versions and recovery from the first release. FileSquid MUST upload, store and give access to any file type, with disclosed interpretation limits for unknown formats ([C-027](./product-vision.md#release-sequence)). Optional paid encrypted **backup and/or** account **sync** on subscription accounts (personal through institutional scale) complement the free core. After that preservation base, version-compliant OCFL collection batch jobs (including FAIR reporting) are the next paid value layer.

ImageSquid, the photo and video distribution, presents photo views over the same API; its scope, parity claims and migration routes are specified in the ImageSquid PRD under the same F- IDs. Every backend capability a distribution needs (derivatives, editing recipes, people and places, migration routes, AI connections, publication) is a FileSquid module behind the API ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)).

Privacy and authenticity MUST be expressed through recoverable local OCFL collections and encrypted hosted preservation, source-aware web clipping with separately encrypted full recorded HTTP evidence, a PROV-O history of every OCFL version and retained change from the first import onward ([F-067](#f-067-prov-o-history-of-every-version-and-change)), C2PA records where the `c2pa` module is enabled (initial records for earlier files are backfilled, [F-043](#f-043-c2pa-records-and-action-disclosure)), and user-selected paid third-party C2PA publication with watermarking usable inside the app. Self-service bulk migration/upload tooling, collection-wide provenance scans/reports, local preservation/export, Dublin Core `/terms/` descriptive metadata and configurable metadata scrubbing MUST remain free; hosted capacity/transport and scoped human assistance may be paid. F-046 preserves extracted values and resource-bound `dcterms:` / `schema:` sidecars, supports single/batch authoring, and verifies outgoing metadata against the selected policy. BYO-AI connections use the user's own runner/account and direct provider billing. [FileSquid vision](./product-vision.md), ImageSquid vision.

Later institutional scope MUST support named GLAM/DAM workflows, selected collection-system and IIIF interoperability profiles, professional metadata exchange and batch publication under F-047, delivered as FileSquid modules with Arkrim as the primary UI. An annual institutional subscription option MUST have explicit service/capacity/support terms and uses the same backup/sync account model at higher quotas. Existing task-fidelity, partnership and funding gates determine implementation readiness and supported profiles; FileSquid and ImageSquid releases proceed independently. After the ImageSquid release, the owner chooses the order of Arkrim, IIIF/GLAM and ProofMode ([C-052](./product-vision.md#release-sequence)).

Requirements use RFC 2119 MUST/MUST NOT, SHOULD and MAY. **Mandatory** means required in the mature committed product; **conditional mandatory** means required for a declared supported capability or enabled paid service; **optional** means an expansion decision remains open. Release placement does not weaken mandatory acceptance criteria. New numerical budgets below are proposed drafting decisions requiring validation, not values claimed by the references.

Source labels used throughout link to exact files; section numbers identify source locations: A — the retired photo-manager feature inventory, whose section numbers are kept as provenance in the ImageSquid PRD, [I — Implementation Plan](./implementation-plan.md), [M — Migration Playbook](./references/filesquid-migration-playbook.md), [V — FileSquid vision](./product-vision.md), IV — ImageSquid vision (kept with the ImageSquid distribution). The [references README](./references/README.md) supplies document-management guidance, not product features. Section citations apply to every acceptance criterion in their requirement block unless marked as a proposed policy. Each feature states the product that delivers it: **FileSquid** for platform features, or the **ImageSquid distribution** for photo-specific features whose backend capabilities are FileSquid modules. F-003, F-004, F-007 and F-011–F-023 state the FileSquid capability and the default-UI criteria; the photo views for the same IDs are specified in the ImageSquid PRD. F-003, F-004, F-007 and F-011–F-023 state the FileSquid capability and the default-UI criteria; the photo views for the same IDs are specified in the ImageSquid PRD. F-003, F-004, F-007 and F-011–F-023 state the FileSquid capability and the default-UI criteria; the photo views for the same IDs are specified in the ImageSquid PRD.

# Product Vision

FileSquid solves dependence on platforms that keep files, versions, descriptions and sharing history in their own databases. It looks and works like a familiar cloud drive, but stores every file as an OCFL object with a readable metadata record, an import record and a PROV-O history of every change (C2PA Content Credentials join with the `c2pa` module), so the owner can leave with an archive other tools can read. Optional encrypted backup and/or account-sync subscriptions complement ownership. The same platform serves consumers through ImageSquid (photo and video), archivists through Arkrim, and third parties through their own distributions ([FileSquid vision](./product-vision.md#one-platform-many-audiences)).

Distributions carry that promise to specific audiences: ImageSquid applies it to photo and video collections (ImageSquid vision), and later Arkrim OCFL-job workflows, GLAM/DAM and ProofMode extend the same preservation, privacy and interoperability guarantees in an owner-chosen order.

## Problem Statement

File owners risk losing versions, descriptions, relationships, provenance or access when moving providers. Everyday drives keep that context in their own databases; preservation systems keep it in open standards but are built for institutions. Owners of photo and video collections additionally risk losing original quality, chronology, edit appearance and collection organization. Large collections and expensive decoding can stall previews, browsing and slideshows; interrupted bulk work can repeat hours of completed work. Self-managed alternatives can impose administrative work. FileSquid MUST combine practical everyday use with demonstrable ownership, preservation, recovery and measured high performance without sacrificing fidelity or privacy, on every host and distribution. Sources: [V](./product-vision.md), Product Vision, Value Propositions and [FileSquid U-11](./product-vision.md#platform-ownership-and-access); ImageSquid IV.

## Target Users

Primary actors are an individual library owner, a family or non-family group member, a shared-library organizer, an invited contributor/viewer, a migrating large-library owner, and a self-hosting operator who runs `filesquid-server` for a family, team or organisation ([FileSquid U-06](./product-vision.md#platform-ownership-and-access), [FileSquid U-19](./product-vision.md#one-api-hosts-and-distributions)). Secondary actors are an authorized support specialist, a `*.filesquid.tech` service operator, an extension provider, a developer embedding the `ocfl`, `metadata` or `c2pa` module, and a distribution builder ([FileSquid U-24](./product-vision.md#one-api-hosts-and-distributions)). Later institutional actors include a gallery, library, archive or museum (GLAM) collection manager, digital archivist or records manager (through Arkrim), metadata curator, authorized publisher and an external collection-system/IIIF consumer under F-047 and F-060. Optional professional client galleries remain separate under F-035. Billing administration and permission to decrypt a private library MUST remain separate; group support does not imply enterprise identity administration. Sources: [V](./product-vision.md), Target Users & Customer Profile and [FileSquid U-39](./product-vision.md#one-api-hosts-and-distributions); ImageSquid IV.

## Definitions and scope rules

- **File:** The item-level type at backend, schema, API, PRD and architecture. A file is a stored bytestream. A distribution MAY label a given file by media kind (ImageSquid: photo, image or video). Photo, Image or Video MUST NOT be minted as domain types ([FileSquid U-04](./product-vision.md#platform-ownership-and-access)).
- **Resource:** Immutable source bytes or an explicitly identified derivative of a file. Physical storage reuse does not merge logical source identities. A coherent file MAY own several resources such as still/motion, RAW/JPEG, depth, audio and source rendition.
- **Recipe/version:** Non-destructive adjustments plus engine/schema version; a rendered source edit is not an editable FileSquid recipe.
- **Repository / library / collection / virtual collection / virtual exhibit:** One installation is one **repository**. A **library** is a named set of files; a user or group MAY own more than one library on a repository ([FE-002](./product-vision.md#independent-library)). A **collection** is a physical grouping that shares one storage location and one access-permission set; a collection belongs to exactly one library; a file's bytes live in exactly one collection. A **virtual collection** is a filtered query inside one library, not a second store. A **virtual exhibit** is a public curated query that MAY span collections across libraries and repositories; every member is public. ImageSquid UI labels are repo, library, album, gallery and exhibit ([FileSquid U-04](./product-vision.md#platform-ownership-and-access), [FileSquid U-05](./product-vision.md#platform-ownership-and-access), [FileSquid U-07](./product-vision.md#platform-ownership-and-access), ImageSquid U-05, [terminology](./product-vision.md#terminology)).
- **Original availability:** Local verified original, remotely retrievable original, preview-only, unavailable, unsupported, or failed. Every relevant operation exposes its state.
- **Backup/sync:** Backup provides independently retained recovery points; sync reconciles current state, including deletions.
- **Distribution / module / host:** A **distribution** is a UI layer (views, design tokens, label table, brand assets and a module set) over the FileSquid API; it contains no backend code. A **module** is a FileSquid Rust crate that provides a capability behind the API. A **host** is the process that serves the API: `filesquid-desktop`, `filesquid-server`, or the later encrypted browser host. The `*.filesquid.tech` hosted service is not a host; it stores and relays ciphertext only ([terminology](./product-vision.md#terminology), [ADR 032](./architecture/032-hosting-and-privacy-model.md)).
- **Parity:** Equivalent supported user outcomes against the feature set of mainstream photo managers (ImageSquid distribution). A preservation-only limitation is recorded as a gap, not counted as editing parity.
- **Source evidence:** The recorded acquisition context and exact retained HTTP messages/bodies, distinct from source-supplied claims and from operational telemetry.
- **PROV-O history:** The W3C PROV-O records, in FileSquid's versioned JSON-LD profile, that describe every OCFL version FileSquid commits and every retained file or metadata change: one activity per committed version with its entities, agents and journal links, stored in OCFL beside the change ([F-067](#f-067-prov-o-history-of-every-version-and-change)). It does not depend on the `c2pa` module.
- **Stored name and display name:** On import FileSquid stores a file's original bytes under a universally unique **stored name**, its FileId plus its original extension (`<file-uuid>.<ext>`), never the owner's file name. The **display name** is the owner's original file name, recorded in the file's metadata record `<file-uuid>.metadata.md` beside it (which also holds the extension, format and MIME type, the import facts and the descriptive metadata); a rename changes the display name only. Every UI surface, search and owner download uses the display name; the stored name is never shown to people ([F-002](#f-002-safe-file-folder-and-device-import)).
- **C2PA record:** The durable per-resource provenance lifecycle of the `c2pa` module, including prepared claim/action data and any signed manifests. Where the module is enabled it begins at import; a file imported before then receives a backfilled initial record that states its own creation time ([F-043](#f-043-c2pa-records-and-action-disclosure)). A prepared/unsigned record is not a valid signed Content Credential.
- **C2PA watermark:** A watermark linked through supported soft-binding/retrieval mechanisms to C2PA provenance, alongside a signed manifest correctly bound to the final derivative. A logo, generic watermark or signature-only result does not satisfy F-044.
- **OCFL package:** An independently identifiable, versioned preservation object; coherent assets and versioned collection/recovery state together form a complete library recovery set.
- **Canonical local collection:** Documented owner-readable files, sidecars and OCFL versions/snapshots representing committed authoritative managed-local content. Operational catalogs/indexes accelerate access and are reconstructable from this state. Referenced or unavailable remote originals and encrypted sensitive HTTP evidence have explicit availability/key exceptions. An **OCFL snapshot** records collection organization; a job **checkpoint** is durable in-progress job state.
- **Prepared media:** A correctly bound, decoded or otherwise display-ready thumbnail/viewer/slideshow rendition at a declared quality. A placeholder or wrong/stale rendition is not useful-image readiness; a preview does not establish original availability or protection.
- **Institutional scope:** The committed later F-047 and F-060 scope (FileSquid modules, Arkrim as primary UI), validated by RC-014 and applicable shared gates independently of FileSquid and ImageSquid release gates (RC-001–013) and photo-manager parity; supported profiles and delivery readiness remain gated.
- **Share:** A grant of a role (viewer, commenter, editor) on a file, folder or collection to a user, group, team or public link. A share never moves bytes and never changes a file's home collection ([F-066](#f-066-file-and-folder-sharing-roles-links-and-shares-view)).
- **Version label:** A user-chosen name for one OCFL version of a file, stored in the version `message` (the `label` member of its structured operation message) or as recoverable application data ([F-065](#f-065-file-version-history-and-retention)). Labels never affect retention; FileSquid keeps every version.
- **Activity stream:** The permission-filtered, user-facing projection of the activity journal ([F-064](#f-064-file-activity-comments-and-user-activity-stream)). The journal itself remains administrator-only ([F-061](#f-061-activity-journal-and-live-health-monitoring)).

# Functional Requirements

Each F identifier is stable and can be referenced independently of release numbering. All criteria under a Mandatory feature are MUST requirements. Capability-dependent criteria are MUST requirements for the published supported combinations; limitations must pass F-040.

## F-001: Independent local library and ownership

**Priority:** Mandatory. **User Story:** As an owner, I want my library to work offline so I retain control of my collection.

**Delivered by:** FileSquid.

### Acceptance Criteria

- The application MUST create/open a local library without an account, internet connection or payment, and permit browsing, organization, supported local edits and original export offline.
- It MUST preserve full-resolution local originals without an application-imposed asset-count paywall; capacity errors MUST explain hardware limits without discarding data.
- Paid expiry/cancellation MUST NOT disable local use or local original export; account deletion MUST be separate from deleting local libraries.
- Library/export schemas MUST be versioned and documented; an export MUST include originals, selected derivatives, metadata, relationships and a machine-readable manifest sufficient for a fresh import to reconstruct supported organization.
- Default managed local libraries MUST continuously represent committed authoritative media, accepted metadata, recipes and collection relationships in documented owner-readable files/sidecars and F-042 OCFL versions/snapshots. Owners MUST be able to close FileSquid, copy and inspect a completed OCFL snapshot with ordinary filesystem tools, and reconstruct supported organization without its operational database, a running FileSquid service or first requesting an export ([FE-065](./product-vision.md#independent-library), [T-007](./product-vision.md#canonical-ocfl-state-and-evidence)).
- Rebuilding from such a copy MUST restore the catalog, the PROV-O history and the activity journal: every committed operation's journal entries and every other entry up to the last hourly seal ([F-061](#f-061-activity-journal-and-live-health-monitoring)).
- An owner MUST be able to recover every file's original file name from such a copy with ordinary tools: each stored original `<file-uuid>.<ext>` has, in the same directory, its metadata record `<file-uuid>.metadata.md` whose YAML front matter names the original file name ([F-002](#f-002-safe-file-folder-and-device-import)).
- Completion of an import or authoritative change MUST wait for verified canonical durability under F-042; buffered UI changes remain visibly pending until the snapshot is durable ([T-035](./product-vision.md#canonical-ocfl-state-and-evidence)). Exact originals MUST remain unchanged; optional TIFF/PNG or other preservation conversions create additional linked renditions ([C-028](./product-vision.md#platforms-originals-and-service-limits)). Referenced and remotely unavailable resources MUST expose coverage limits, and separately encrypted sensitive evidence requires authorized key access. Direct inspection and copy MUST NOT authorize in-place edits of OCFL inventories or signed originals ([T-036](./product-vision.md#canonical-ocfl-state-and-evidence)).
- High performance under NFR-002/003 MUST remain a free core requirement. Paid entitlements MUST NOT introduce artificial local browsing, processing or asset-count throttles. Free local access and export MUST survive paid cancellation. Local storage capacity MUST be limited by hardware, not an artificial asset-count paywall ([C-007](./product-vision.md#platforms-originals-and-service-limits)).
- Optional encrypted local packages and remotely unavailable originals MUST disclose key, tool and availability requirements. FileSquid MUST NOT promise unrestricted ordinary-file access for those packages ([C-030](./product-vision.md#platforms-originals-and-service-limits)).

## F-002: Safe file, folder and device import

**Priority:** Mandatory. **User Story:** As an owner, I want to bring media into FileSquid without risking the source.

**Delivered by:** FileSquid. Camera and phone device adapters are FileSquid modules that the ImageSquid distribution enables ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)).

### Acceptance Criteria

- Desktop MUST import selected/all-new files, folders, mounted camera cards and supported camera/phone device adapters; drag-and-drop and imports from other applications MUST use the same integrity workflow. Device support MUST be listed by platform.
- Import review MUST show proposed items, previously imported items, destination album/new-album choice, logical/resource counts and estimated capacity. Watched folders MUST be opt-in, bounded and idempotent.
- Managed import MUST copy source bytes into managed storage and hash/verify them. Explicit referenced import MUST instead verify and record the existing bytes/location under F-009 without claiming an independent copy. Both modes MUST register only verified resources, preserve unsupported originals and expose preview failures separately; migration MUST use an independently copied destination.
- Progress, pause/resume, cancellation, per-item errors and retry MUST survive restart. Cancellation MUST leave the source unchanged and identify retained staged data.
- Browser/mobile selected uploads MUST resume where supported and request source access again when required; browser closure MUST NOT be described as guaranteed continued local execution.
- Every imported resource MUST commit its import record, the append-only `import` section of the file's metadata record `<file-uuid>.metadata.md` (below), in the same OCFL version as its content before import completion, or, for an explicit referenced import, in the OCFL version that records the reference, stating that the bytes were referenced rather than copied ([ADR 038](./architecture/038-import-file-record.md)). The import record MUST hold every fact of the ADR 038 fact set (decision 3) that the source system and the host operating system provide, serialized as ADR 038 specifies (valid UTF-8 as strings, anything else as base64 marked `encoding: base64`), including: original name and path, import root, file size in bytes (as the source reported it and as verified from the bytes FileSquid read and hashed, which are the copied content for a managed import or the referenced bytes for a referenced import; a mismatch MUST fail verification and leave the import pending), birth, modified, status-changed, accessed and date-added times, owner, group, permissions, ACL and flags, link target, OS-reported content type, every extended attribute (decoded where known, such as quarantine origin, where-from URLs and Finder tags), source volume, host operating system and version, the importing account and the FileSquid version, plus the import method, time and job. A fact the source or host cannot provide MUST be listed as unavailable with a reason and MUST NOT be invented. The record MUST be append-only (a re-import adds an acquisition entry; earlier entries never change), MUST be bounded per acquisition entry (at most 256 extended-attribute names and 1 MiB of attribute value bytes inspected; values over 64 KiB recorded by size and SHA-256 only; anything cut by these bounds marked truncated or incomplete, never silently dropped) and MUST NOT be presented as a C2PA record. Import completion MUST require both the verified content and its import record; a failure between them MUST leave a retryable pending import, never a file without a record. The UI MUST receive a projection without raw filesystem paths. Its identity and location fields (paths, account and host names, numeric IDs, origin URLs) MUST fall under the [F-046](#f-046-media-metadata-inspection-correction-and-privacy-exports) outgoing disclosure policy and be scrubbed from exports and shares by default.
- On import every file MUST be automatically **tracked** and **described**, with no C2PA module required. Tracked: its verified bytes are committed as an OCFL object and version with their SHA-256 digest ([F-042](#f-042-ocfl-preservation-and-portable-recovery)), its import record in the same version (above), and the import's PROV-O activity in the same version ([F-067](#f-067-prov-o-history-of-every-version-and-change)). Described: supported embedded metadata is extracted and mapped onto `dcterms:` / `schema:` statements in the file's metadata record, and every later metadata edit commits a new OCFL version ([F-052](#f-052-metadata-module), [F-046](#f-046-media-metadata-inspection-correction-and-privacy-exports)). Import completion MUST require the content, its import record and its PROV-O record; F-041 web captures MUST retain their source/HTTP evidence under F-042. C2PA records are specified separately for the `c2pa` module in [F-043](#f-043-c2pa-records-and-action-disclosure).
- The original bytes MUST be stored under the stored name `<file-uuid>.<ext>`: the file's UUID v4 FileId ([ADR 021](./architecture/021-uuid-v4-identifiers.md)) plus its original extension when that extension is 1 to 16 ASCII letters or digits (otherwise no extension); the owner's file name MUST NOT appear in a storage path. In the same directory FileSquid MUST commit, in the same OCFL version, the file's metadata record `<file-uuid>.metadata.md` (one per file, [ADR 027](./architecture/027-filesquid-metadata-pluggable-stores.md), [ADR 038](./architecture/038-import-file-record.md) as amended 2026-10-19). It is THE metadata record of the file: it holds the owner's exact original file name (with its Unicode normalisation form), the file extension, the format and MIME type, the import facts (its append-only `import` section) and the file's descriptive metadata. The display name starts as the original name; a rename changes only the display name, recorded in a new version of the metadata record, and never the stored name or the recorded original name. The storage layout itself is the FileSquid OCFL application profile ([API-S-026](../apps/filesquid/crates/api/resources/srs.md)).
- Local import and supported upload batches MUST reuse verified completed resources and transport checkpoints after interruption, revalidating source version and authorization. Upload protocols MUST declare actual resumability; byte ranges alone do not establish resumable uploads. Where within-resource resume is unavailable, only the affected unit may replay, with repeated work reported under NFR-003.
- Import, verification and derivative generation MUST use bounded background scheduling under NFR-003. Verified items and suitable available previews MUST become usable incrementally; whole-batch thumbnail completion MUST NOT block browsing or imply that an incomplete original is ready.

## F-003: Browsing, selection and viewing

**Priority:** Mandatory. **User Story:** As a viewer, I want responsive browsing and inspection of the collection.

**Delivered by:** FileSquid modules behind the API ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)). FileSquid's default UI presents the generic file view in the criteria below. A distribution's views (for example ImageSquid's photo views) carry their own UI criteria in that distribution's PRD.

### Acceptance Criteria

**FileSquid capability**

- The API MUST serve paged, bounded browse queries with stable file keys and stable ordering: by year, month and day, by collection, by virtual collection and over all files. No view MUST require loading the whole library.
- The media module MUST produce thumbnails and display derivatives progressively, without complete-library preprocessing and without repeatedly decoding full originals for ordinary navigation. Orientation, colour interpretation and edit/resource version MUST be correct at every quality served.
- Video, audio and motion resources MUST be served for playback through ranged reads. HDR MUST be capability-detected and reported with an identified SDR fallback.
- A selection MUST be expressible to the API as individual items, ranges, non-contiguous sets or the whole current result (a frozen query plus exceptions). Batch operations MUST report the effective selection scope before they run.
- Display exclusions (for example screenshots, or received and shared items) MUST be query filters. They MUST NOT delete or alter collection membership.
- A client MUST be able to request derivatives for items before and after the current view within declared finite item, byte and time limits. The API MUST deduplicate such work, cancel obsolete requests and reject stale results.
- Thumbnail, display and view caches MUST use versioned content, view and permission keys, explicit invalidation and finite memory and disk budgets under NFR-003/004. Missing, evicted or failed media MUST be reported with a state and a recovery path; a placeholder alone MUST NOT satisfy NFR-002 useful-image timing.

**FileSquid default UI**

- The default UI MUST offer list and grid views with adjustable thumbnail size, a preview for images, video and audio, adjacent-item navigation, and every selection form above with keyboard alternatives.
- Virtualized lists and grids MUST retain focus, selection and accessible item semantics as the visible window changes.
- Missing, pending and failed previews MUST show an honest state and a recovery path.

**Distributions**

- A distribution MAY present this capability with its own views and wording; those views MUST use only the FileSquid API and their UI criteria belong to that distribution's PRD.
- Views, sort keys, selection UI and layout preferences for the default UI are specified in [F-062](#f-062-drive-style-views-sorting-selection-and-layout-preferences).

## F-004: Collections, folders and navigation

**Priority:** Mandatory. **User Story:** As a curator, I want flexible collections without duplicating originals.

**Delivered by:** FileSquid modules behind the API ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)). FileSquid's default UI presents the generic file view in the criteria below. A distribution's views (for example ImageSquid's photo views) carry their own UI criteria in that distribution's PRD.

### Acceptance Criteria

**FileSquid capability**

- Users MUST be able to create empty and from-selection collections, rename them, populate them by command or drag, duplicate, merge and delete them. A collection MUST belong to exactly one library. A file's bytes MUST live in exactly one collection. Membership of one file in several groupings without duplicating originals MUST be a virtual collection or other non-storage membership, not a second collection store ([FileSquid U-05](./product-vision.md#platform-ownership-and-access)).
- Removing or deleting a collection MUST retain its files. Deleting a file from the library MUST be a distinct, labelled action.
- Collections MUST support manual order, title and date sort (ascending and descending), cover selection, and deterministic membership handling on duplicate and merge.
- A virtual collection MUST be either **rule-based** (a saved query, [F-006](#f-006-virtual-collections-and-metadata-search)) or **hand-picked** (an explicit member list). A hand-picked virtual collection MUST support add and remove by command or drag, manual order, title and date sort, cover selection, duplicate, merge and delete, with the same deterministic membership handling as a collection. Removing a file from a hand-picked virtual collection MUST NOT affect its collection or its other memberships. Adding a file that already has a home collection to a second grouping MUST create or extend a hand-picked virtual collection, never a second copy.
- The API MUST expose every organization command (add to or remove from a collection or virtual collection, reorder, rename, delete a collection or virtual collection, favorite, keyword assignment) with a compensating inverse so a client can offer in-session **Undo** and **Redo** within a bounded history. The inverse MUST be journaled under [F-061](#f-061-activity-journal-and-live-health-monitoring) and MUST produce its own version under [F-065](#f-065-file-version-history-and-retention); it MUST NOT be a silent rollback.

**FileSquid default UI**

- The default UI MUST show collections, virtual collections and nested folders in its navigation tree under the internal names, with pinned destinations, reorderable sections, collapse and expand, and keyboard equivalents for reordering. These choices MUST persist across restarts.

**Distributions**

- A distribution MAY present this capability with its own views and wording; those views MUST use only the FileSquid API and their UI criteria belong to that distribution's PRD.
- Folder-on-top placement and sort preferences persist under [F-062](#f-062-drive-style-views-sorting-selection-and-layout-preferences).

## F-005: Favorites, metadata and keywords

**Priority:** Mandatory. **User Story:** As an owner, I want searchable descriptions and consistent batch corrections.

**Delivered by:** FileSquid (`metadata` module, favorites and keywords through the API). The ImageSquid distribution presents the photo information panel and Keyword Manager ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)).

### Acceptance Criteria

- Favorites, titles (`dcterms:title`), captions (`dcterms:description`), keywords (`dcterms:subject`, or `schema:keywords` on overflow) and accessibility descriptions MUST support single and applicable batch edits. A title change MUST NOT rename a managed original.
- Keyword Manager MUST create, rename, delete and organize tags, configure quick-access shortcuts, and update affected associations deterministically.
- Information panels MUST display available filename, format, dimensions, byte size, camera/lens and exposure fields; unknown data MUST remain unknown.
- Date/time/time-zone and GPS assignment/correction/copy/removal/restoration MUST retain original values and provenance. Capture dates lacking a zone MUST not acquire an invented UTC instant.
- F-046 MUST provide private EXIF/native metadata inspection, extraction and versioned corrections. Sharing/publication MUST apply F-046's user-selected scrubbing mode and retained EXIF/video fields; additional sensitive descriptive fields use the selected privacy profile independently of the retained local original. F-041 full HTTP evidence MUST remain privately retained even when a sanitized view/export omits its secrets; F-043 records relevant metadata/disclosure changes.
- Owners MUST be able to create, inspect, edit and remove resource-bound descriptive statements through F-046, including supported single and batch operations. Descriptive keys MUST be ISO 15836-2 `/terms/` properties (`dcterms:`), confirmed Schema.org overflow (`schema:`), or terms in a registered extension namespace ([F-052](#f-052-metadata-module)). Those statements MUST remain separate from immutable extracted source values and obey the same recovery, search and disclosure rules as supported metadata.

## F-006: Virtual collections and metadata search

**Priority:** Mandatory. **User Story:** As a curator, I want collections and searches that update as my library changes.

**Delivered by:** FileSquid (virtual collections and search through the API). The ImageSquid distribution presents rule-based virtual collections as smart galleries ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)).

### Acceptance Criteria

- Search MUST cover filename, `dcterms:title`, `dcterms:description`, `dcterms:subject`, dates and location, combine criteria, and offer suggestions without leaking hidden/private content.
- Rule-based virtual collections (saved queries) MUST support editable Any/All rules for text, capture/addition dates, named identities, collection membership/unfiled items, media/status and camera/lens/exposure properties; unavailable indexes MUST show pending/unsupported evaluation.
- Definitions MUST be named/organized with ordinary collections and update membership after relevant changes. Browser/mobile MUST expose saved views; synchronized definitions MUST use a versioned rule format.
- Quick filters MUST include photos/videos, favorites, edited items, screenshots, configured keywords, **unfiled** (in no collection or virtual collection) and, when the library is shared under [F-034](#f-034-family-accounts-and-shared-libraries), **captured or contributed by me**; the filter menu MUST offer a Manage Keywords entry, filters MUST be combinable and the active set MUST be shown. Date boundaries and case/Unicode behavior MUST be specified and tested.
- Search MUST include namespaced descriptive-field presence and supported value matching (`dcterms:creator`, `schema:certification`, and other populated keys). Rule-based virtual collections MUST expose the descriptive-field predicates declared by the nonempty F-046 supported type/operator matrix; unsupported predicates MUST be explicit rather than silently ignored. Single/batch field changes MUST invalidate affected searches and saved views after canonical commit without returning stale or unauthorized results.
- Paged search/view queries and their caches MUST remain bounded, preserve stable ordering across pages and obey version/permission invalidation. Query feedback and completed results MUST be measured separately under NFR-002 while indexing progresses visibly in the background.
- The operator grammar and filter chips shared with the generic file view are specified in [F-063](#f-063-file-search-query-language-and-filter-chips).

## F-007: Automatic and activity collections

**Priority:** Mandatory. **User Story:** As a viewer, I want useful ways back into my collection.

**Delivered by:** FileSquid modules behind the API ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)). FileSquid's default UI presents the generic file view in the criteria below. A distribution's views (for example ImageSquid's photo views) carry their own UI criteria in that distribution's PRD.

### Acceptance Criteria

**FileSquid capability**

- The API MUST provide activity collections for imports, recently added and saved, edited, viewed and shared files, favorites, and files received or shared with the user, with controls for activity retention and visibility.
- The API MUST provide media-type groupings for every media kind the metadata module identifies (for example video, motion, panorama, burst, screenshot, screen recording, slow motion, time lapse, animated, RAW and spatial media).
- Time-based and identity-based groupings MUST offer curated highlights and an All view without discarding omitted files.
- Classification-based collections (for example documents, receipts, handwriting, illustrations, QR codes) MUST expose classification corrections and analysis progress. Empty and pending collections MUST be distinguishable.
- Ingestion from third-party messaging or platform sources MUST be an adapter-dependent equivalent; private platform APIs MUST NOT be presumed.

**FileSquid default UI**

- The default UI MUST list activity collections (recent, imports, favorites, shared with me) in its navigation and MUST let the user filter any view by media kind.

**Distributions**

- A distribution MAY present this capability with its own views and wording; those views MUST use only the FileSquid API and their UI criteria belong to that distribution's PRD.
- The user-facing activity stream is [F-064](#f-064-file-activity-comments-and-user-activity-stream); this item keeps the activity collections and their retention and visibility controls.

## F-008: Deletion, hidden files and duplicate review

**Priority:** Mandatory. **User Story:** As an owner, I want private items protected and mistakes recoverable.

**Delivered by:** FileSquid.

### Acceptance Criteria

- Proposed local policy: recoverable deletion MUST default to 30 days, with the expiry shown before deletion; permanent deletion MUST require an explicit scope confirmation. Hosted backup retention is separately governed by F-031/F-032.
- Hidden files MUST be excluded from ordinary browsing, suggestions, memories, notifications and sharing by default. Hidden and Recently Deleted views MUST require local authentication, with an option to hide their navigation entry.
- Exact-byte duplicates and perceptual candidates MUST be distinguished. A review MUST show retained original quality, metadata/album effects and merge choices before an individual/batch merge.
- Merging MUST preserve all required resource/relationship information and make removed logical items recoverable; identical physical bytes alone MUST NOT collapse distinct source records.
- Recovered/orphaned-media review MUST permit validated reintegration or explicit removal; UI hiding MUST NOT be marketed as full local disk encryption.
- Hidden, deleted and duplicate-review views MUST identify each file by its display name, never by its stored name ([F-002](#f-002-safe-file-folder-and-device-import)). Hide, unhide, delete, restore and permanent delete MUST each produce PROV-O history ([F-067](#f-067-prov-o-history-of-every-version-and-change)).
- The deleted-files view and `is:deleted` / `is:unfiled` queries are specified in [F-062](#f-062-drive-style-views-sorting-selection-and-layout-preferences) and [F-063](#f-063-file-search-query-language-and-filter-chips).

## F-009: Multiple libraries and filesystem administration

**Priority:** Mandatory. **User Story:** As an archive owner, I want to manage storage without manual catalog surgery.

**Delivered by:** FileSquid.

### Acceptance Criteria

- A user or group MAY own more than one library on a repository; a repository MAY host several libraries ([FE-002](./product-vision.md#independent-library)). Desktop MUST create/switch those libraries, designate a default/system-integration library, relocate a complete library and import selected/new media between libraries. Group membership MUST NOT automatically grant ownership of another member's libraries.
- Managed and referenced modes MUST be explicit. Referenced originals MUST support reveal, missing-file detection, relinking by identity and consolidation with verification before changing the reference.
- Supported external volumes MUST retain stable volume identification. Missing drives MUST pause affected work rather than silently target a replacement path.
- Live catalog storage on unsupported network filesystems MUST be rejected or migrated safely; NAS backup targets MUST not imply multi-client SQLite catalog support.
- Catalog repair, schema migration, thumbnail rebuilding and recovery MUST preserve a verified pre-operation backup and expose completion/error reports.
- The documented canonical local layout MUST identify file objects, collection snapshots, sidecars and protected evidence/key requirements. Supported app-closed filesystem copy and catalog rebuild MUST preserve committed state; external edits or corrupt inventories MUST be detected and reconciled or reported without silently replacing canonical originals/history. Referenced resources MUST remain clearly outside managed-copy guarantees until verified consolidation completes. Copying referenced-library pointers MUST NOT be treated as preserving missing media ([C-029](./product-vision.md#platforms-originals-and-service-limits)).

## F-010: Export and portable backup

**Priority:** Mandatory. **User Story:** As an owner, I want to use my media elsewhere and prove I can recover it.

**Delivered by:** FileSquid.

### Acceptance Criteria

- Original export MUST reproduce verified source bytes; rendered export MUST identify the edit version, output settings and availability dependencies.
- Supported output MUST include JPEG, PNG, TIFF including 16-bit, and HEIC where licensed/supported, with dimensions, quality/compression, color profile and metadata controls; unsupported encoders MUST not silently substitute formats.
- Batch export MUST support FileId names (RFC 4122 / RFC 9562 UUID version 4, [ADR 021](./architecture/021-uuid-v4-identifiers.md)), source/title/sequential names, prefixes, collision-safe names, subfolder grouping and XMP sidecars. Interoperable XMP sidecars MUST carry the same F-046 `dcterms:` / `schema:` register as the OCFL sidecar JSON ([ADR 022](./architecture/022-dublin-core-descriptive-metadata.md)). Owner downloads and owner exports MUST name each file by its display name, the owner's original file name or its rename ([F-002](#f-002-safe-file-folder-and-device-import)), by default, made collision-safe; the other naming schemes remain options. A sharing or publication output MUST instead default to FileId plus the real media extension (for example `a1b2c3d4-e5f6-4789-abcd-ef0123456789.png`) under the F-046 outgoing policy ([C-033](./product-vision.md#provenance-metadata-and-disclosure)); the owner MAY choose the original name for it per saved policy. The stored name `<file-uuid>.<ext>` MUST NOT be offered as a display or download name except as that FileId scheme. Motion originals MUST export their still/video components with association metadata.
- Video export MUST support declared qualities including up to 4K on supported paths; drag-to-file-manager MUST produce explicit exported copies. Presentation export is covered by F-022/F-023.
- Portable backup MUST capture originals plus a consistent catalog/recipe/relationship snapshot and manifest; clean-library restoration MUST verify resources, relationships and offline original access, including referenced-resource coverage warnings. F-042 additionally requires OCFL packages, source/HTTP evidence, PROV-O history (F-067) and, where the `c2pa` module is enabled, C2PA records, with usable key recovery.
- All sharing/publication exports MUST pass F-046 application of the selected scrubbing mode/retained fields, outgoing filename policy and output inspection. Owner archival/original export MUST be explicitly identified separately and MUST NOT bypass sharing safeguards. Original names MAY appear in an owner archival/original package and MUST appear in its machine-readable manifest so a fresh import can reconstruct supported organization. Canonical OCFL object identity remains FileId as `urn:uuid:` plus that UUID.
- Every export, scrubbed export, ZIP download and share or publication output MUST record its PROV-O activity ([F-067](#f-067-prov-o-history-of-every-version-and-change)). A single-file download MUST be recorded in the activity journal only and MUST NOT create a PROV-O record. Where the `c2pa` module is enabled, rendered/published exports MUST also update required F-043 action disclosure; paid watermarked output MUST follow F-044 while original export remains byte-identical and free.
- Portable export MUST supplement the already accessible F-001 canonical local collection. Plain local and encrypted package manifests MUST match the bytes actually delivered; reports MUST distinguish omitted/unavailable references, authorization/key failures and excluded rebuildable caches from complete recovery. Free self-service tooling MUST include resumable supported bulk transfer/export without requiring paid storage or human assistance.

## F-011: Non-destructive editing workflow

**Priority:** Mandatory. **User Story:** As an editor, I want to try changes safely and reproduce them later.

**Delivered by:** FileSquid modules behind the API ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)). FileSquid's default UI presents the generic file view in the criteria below. A distribution's views (for example ImageSquid's photo views) carry their own UI criteria in that distribution's PRD.

### Acceptance Criteria

**FileSquid capability**

- Every edit MUST preserve the original. Versioned recipes MUST support undo, redo, revert, before/after comparison, duplicate versions, per-adjustment enable, disable and reset, and an automatic enhancement operation.
- Copying a recipe or part of it MUST apply to one or many files and MUST report adjustments skipped as incompatible.
- Recipe and rendering-engine versions MUST be retained. An engine upgrade MUST NOT silently change an approved appearance; old rendered results MUST remain available when an old engine cannot run.
- Every renderer (desktop, server and browser) MUST be checked against the same reference corpus and published tolerances. Save, crash recovery and concurrent edits MUST retain recoverable versions.
- Edit, undo, revert and rendered-output commits MUST participate in F-043 action recording without mutating previously signed manifests or original resources.
- Retained edit and metadata versions and derivatives MUST use the recoverable PROV-O history of [F-067](#f-067-prov-o-history-of-every-version-and-change); cache-only preview generation remains separate until a result is retained or disclosed ([T-037](./product-vision.md#canonical-ocfl-state-and-evidence)).
- Assistive standard editing that does not substantially alter the input or its semantics (crop, exposure and equivalent) MUST NOT be classified or labelled as AI-generated content ([C-017](./product-vision.md#ai-and-article-50)).

**FileSquid default UI**

- The default UI MUST show a file's edit versions in its Versions panel and MUST let the user revert to the original or to any retained version.

**Distributions**

- A distribution MAY present this capability with its own views and wording; those views MUST use only the FileSquid API and their UI criteria belong to that distribution's PRD.
- Recipe versions are distinct from the file's OCFL version history in [F-065](#f-065-file-version-history-and-retention).

## F-012: Composition and tonal/color editing

**Priority:** Mandatory. **User Story:** As an editor, I want complete everyday composition and exposure/color control.

**Delivered by:** FileSquid modules behind the API ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)). FileSquid's default UI presents the generic file view in the criteria below. A distribution's views (for example ImageSquid's photo views) carry their own UI criteria in that distribution's PRD.

### Acceptance Criteria

**FileSquid capability**

- The recipe model MUST represent composition (crop with free, preset, custom and original ratios; rotate; horizontal and vertical flip; straighten; horizontal and vertical perspective, and automatic crop and straighten suggestions), tone (coordinated light and brilliance-style balancing, exposure, highlights, shadows, brightness, contrast, black point, global and per-channel curves, levels), colour (saturation, vibrance, cast, white balance with grey, skin and temperature-tint modes) and monochrome (intensity, neutrals, tone, grain), each with an independent reset.
- Selective colour MUST support at least six independent sampled colours with hue, saturation, luminance and range parameters.
- The render worker MUST compute live histograms and MUST apply saved settings identically in preview and export.
- Every parameter MUST be readable and writable through the API with its current value, so any UI can expose it accessibly.

**FileSquid default UI**

- The default UI MUST show whether a file has an edit recipe. It need not offer editing controls; editing views belong to the distributions that present them.

**Distributions**

- A distribution MAY present this capability with its own views and wording; those views MUST use only the FileSquid API and their UI criteria belong to that distribution's PRD.

## F-013: Detail, filters, retouch and RAW

**Priority:** Mandatory. **User Story:** As an enthusiast, I want finishing tools and fidelity for camera originals.

**Delivered by:** FileSquid modules behind the API ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)). FileSquid's default UI presents the generic file view in the criteria below. A distribution's views (for example ImageSquid's photo views) carry their own UI criteria in that distribution's PRD.

### Acceptance Criteria

**FileSquid capability**

- The recipe model and render worker MUST support definition or clarity, noise reduction, sharpening, vignette and adjustable-strength colour and monochrome filters.
- Retouch operations MUST be recipe steps with retained history. Red-eye correction MUST be available on supported stills and MUST be refused on video.
- Supported RAW and ProRAW originals MUST be preserved, developed and exportable unchanged on request. RAW+JPEG pairs MUST remain related, with an explicit choice of editing original.
- RAW engine upgrades MUST allow deliberate reprocessing and comparison without silently replacing an approved render. Supported cameras and decoding limitations MUST be published in the F-040 capability register.

**FileSquid default UI**

- The default UI MUST show RAW+JPEG pairs as one related item with both resources listed in its Details panel.

**Distributions**

- A distribution MAY present this capability with its own views and wording; those views MUST use only the FileSquid API and their UI criteria belong to that distribution's PRD.

## F-014: AI-assisted cleanup and styles

**Priority:** Conditional mandatory for supported models/media. **User Story:** As an editor, I want to remove distractions and apply capture-aware looks safely.

**Delivered by:** FileSquid modules behind the API ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)). FileSquid's default UI presents the generic file view in the criteria below. A distribution's views (for example ImageSquid's photo views) carry their own UI criteria in that distribution's PRD.

### Acceptance Criteria

**FileSquid capability**

- Cleanup operations MUST accept a selection region and support preview, cancellation, reversible application and explicit generation-failure states.
- **Extend** MUST generate image content beyond the current frame to a requested aspect ratio or margin through a supported [F-045](#f-045-byo-ai-connections-and-per-task-model-choice) task route, with preview, cancellation, reversible application and explicit generation-failure states. **AI Reframe** MUST offer a model-assisted perspective change where a supported model exists and MUST fall back to the geometric perspective correction of [F-012](#f-012-composition-and-tonalcolor-editing) otherwise. Both are retained material AI manipulations under the last bullet of this list: F-043 action recording and the machine-readable AI mark apply, and the resulting rendition MUST be reported as AI-manipulated to every UI.
- Face obscuring MUST be a reversible local edit. An export intended to conceal identity MUST render the selected effect and omit recoverable source layers.
- Style operations MUST distinguish generic styling from capture-aware interpretation in their recorded metadata.
- AI outputs MUST retain input, version and model provenance and MUST NOT overwrite originals. Normal operation MUST follow F-029's private-library and selected-disclosure boundaries, MUST use F-045 for BYO models, and MUST update F-043 for required AI and action disclosure.
- When FileSquid, for itself or for a distribution under that distribution's name, performs a retained material AI manipulation (generative fill, person removal, face swap or equivalent), it MUST record the action in the PROV-O history ([F-067](#f-067-prov-o-history-of-every-version-and-change)) and, where the `c2pa` module is enabled, in the C2PA history ([F-043](#f-043-c2pa-records-and-action-disclosure)), and MUST attach a machine-readable AI-generated-or-manipulated mark to the retained derivative as far as technically feasible.

**FileSquid default UI**

- The default UI MUST show the AI-manipulation mark and its PROV-O provenance for any file that carries one, and, where the `c2pa` module is enabled, its C2PA provenance through the Content Credentials panel.

**Distributions**

- A distribution MAY present this capability with its own views and wording; those views MUST use only the FileSquid API and their UI criteria belong to that distribution's PRD.

## F-015: Portrait, depth, Cinematic and spatial media

**Priority:** Conditional mandatory for supported capture formats. **User Story:** As an owner of specialized captures, I want their information preserved and usable controls when supported.

**Delivered by:** FileSquid modules behind the API ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)). FileSquid's default UI presents the generic file view in the criteria below. A distribution's views (for example ImageSquid's photo views) carry their own UI criteria in that distribution's PRD.

### Acceptance Criteria

**FileSquid capability**

- Depth, focus, spatial and other opaque capture resources MUST be preserved even when the renderer cannot interpret them. A flattened rendition MUST remain linked to its source and labelled.
- Where source data is accessible, the render worker MUST support portrait effect on and off, depth blur, refocus and lighting variants, and for Cinematic captures time-based focus decisions, depth blur and audio control, as recipe operations on declared compatible captures.
- Spatial media MUST declare its hardware and output prerequisites and MUST retain an ordinary viewing fallback.
- Per-format import, edit and export fixtures MUST demonstrate each claimed capability; unsupported formats MUST be registered in F-040.

**FileSquid default UI**

- The default UI MUST preview the ordinary fallback rendition of such media and MUST list the preserved specialised resources in its Details panel.

**Distributions**

- A distribution MAY present this capability with its own views and wording; those views MUST use only the FileSquid API and their UI criteria belong to that distribution's PRD.

## F-016: Live Photos, bursts and motion images

**Priority:** Mandatory with format-specific capability gates. **User Story:** As a viewer/editor, I want paired captures to remain coherent.

**Delivered by:** FileSquid modules behind the API ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)). FileSquid's default UI presents the generic file view in the criteria below. A distribution's views (for example ImageSquid's photo views) carry their own UI criteria in that distribution's PRD.

### Acceptance Criteria

**FileSquid capability**

- Still and motion resources of a Live Photo or motion image MUST remain one logical file record with independent checksums. A missing motion component MUST produce a partial-pair state rather than a claimed complete pair.
- The recipe model MUST support key frame, trim, loop, bounce and long-exposure equivalents, Live and still duplication, eligible animated GIF export and revert on supported captures.
- Burst sequences MUST remain related. Selecting retained frames MUST use recoverable deletion for discarded frames and preserve source relationships.

**FileSquid default UI**

- The default UI MUST show a motion pair or a burst as one item whose resources are listed in its Details panel, and MUST play the motion resource in preview.

**Distributions**

- A distribution MAY present this capability with its own views and wording; those views MUST use only the FileSquid API and their UI criteria belong to that distribution's PRD.

## F-017: Video and audio editing

**Priority:** Mandatory with codec/capture gates. **User Story:** As a video owner, I want ordinary video finishing without losing recorded material.

**Delivered by:** FileSquid modules behind the API ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)). FileSquid's default UI presents the generic file view in the criteria below. A distribution's views (for example ImageSquid's photo views) carry their own UI criteria in that distribution's PRD.

### Acceptance Criteria

**FileSquid capability**

- Video edits MUST be non-destructive recipe operations: trim, poster frame, automatic enhancement, whole-clip tone, colour, curves, levels, detail and filters, crop, rotate, flip, straighten and perspective, still-frame extraction and full restoration.
- Supported paths MUST preserve, play and export 10-bit HDR and edit slow-motion ranges and eligible high-frame-rate speed. Output MUST disclose resolution, frame rate, bit depth, colour transfer and any conversion.
- Audio operations MUST support mute and compatible enhancement, and audio-mix equivalents (original, in-frame emphasis, noise and reverberation reduction, dialogue and environment balance with intensity) when the required audio resources are available.
- The API MUST refuse still-only operations (retouch, red-eye, still markup) on video.
- The video worker MUST be the isolated LGPL FFmpeg worker of ADR 011.

**FileSquid default UI**

- The default UI MUST play video and audio in preview, including HDR with its SDR fallback, and MUST show the output properties above as technical metadata in its Metadata panel.

**Distributions**

- A distribution MAY present this capability with its own views and wording; those views MUST use only the FileSquid API and their UI criteria belong to that distribution's PRD.

## F-018: Markup and companion annotation

**Priority:** Mandatory; device-specific input is conditional. **User Story:** As a communicator, I want to annotate a photograph accessibly.

**Delivered by:** FileSquid modules behind the API ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)). FileSquid's default UI presents the generic file view in the criteria below. A distribution's views (for example ImageSquid's photo views) carry their own UI criteria in that distribution's PRD.

### Acceptance Criteria

**FileSquid capability**

- Annotations MUST be stored as an editable recipe of objects (sketches, shapes, arrows, text, highlights, magnifiers, stickers) over a still image; a general professional layer compositor is not required.
- A companion workflow MUST pair authorized devices and return annotation edits without exposing the library to unrelated devices.
- Accessibility descriptions MUST be stored independently of visible image text and included in exports only according to the export or sharing policy.

**FileSquid default UI**

- The default UI MUST show whether a file has annotations and MUST let the user read and edit its accessibility description.

**Distributions**

- A distribution MAY present this capability with its own views and wording; those views MUST use only the FileSquid API and their UI criteria belong to that distribution's PRD.

## F-019: People, pets and groups

**Priority:** Mandatory. **User Story:** As a curator, I want to find people/pets and correct recognition mistakes.

**Delivered by:** FileSquid modules behind the API ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)). FileSquid's default UI presents the generic file view in the criteria below. A distribution's views (for example ImageSquid's photo views) carry their own UI criteria in that distribution's PRD.

### Acceptance Criteria

**FileSquid capability**

- Analysis MUST detect and group supported faces in stills and video and supported pets, assign multiple identities per file and expose processing status through the API.
- The API MUST support naming and renaming identities, manually added regions, confirming and rejecting matches, marking incorrect identities, merging duplicates, and propagating accepted corrections without silently reinstating rejected assignments.
- Hiding an identity MUST be distinguishable from deleting files. Groups (automatic co-occurrence and custom) MUST support membership edits, names and order.
- Recognition enable, disable, reset and rebuild MUST be explicit. Protected identity information MUST synchronize only under the encryption and permission model. Compatible model and task choice and durable corrections MUST follow F-045.
- Household people and pet organisation MUST remain available without a deployer-style notice.
- When people or biometric categorisation analysis is used under professional authority, FileSquid MUST give every UI what it needs to inform the natural persons exposed to that operation.
- Emotion recognition, untargeted facial-image scraping and remote biometric identification in publicly accessible spaces MUST NOT be implemented ([C-020](./product-vision.md#ai-and-article-50)). A notice MUST NOT be claimed as GDPR compliance.

**FileSquid default UI**

- The default UI MAY omit identity views. Where it shows identities on a file, it MUST show them as inferences with their correction state.

**Distributions**

- A distribution MAY present this capability with its own views and wording; those views MUST use only the FileSquid API and their UI criteria belong to that distribution's PRD.

## F-020: Semantic search, OCR and visual understanding

**Priority:** Mandatory with model/language gates. **User Story:** As a viewer, I want to find media by its content and use recognized information.

**Delivered by:** FileSquid modules behind the API ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)). FileSquid's default UI presents the generic file view in the criteria below. A distribution's views (for example ImageSquid's photo views) carry their own UI criteria in that distribution's PRD.

### Acceptance Criteria

**FileSquid capability**

- Search MUST cover objects, scenes and activities, people and pets, location and date combinations, OCR text, natural-language queries and time-coded video moments. Contributor search MUST respect shared-library permissions.
- OCR results MUST be stored with their source regions so any UI can offer selection, copy and user-initiated external actions. Every external action MUST require a user action and MUST disclose any text sent off the device.
- Visual lookup MUST support declared subject classes with copyable contextual information and its provenance. Landmark matching without GPS MUST have its own enable and disable control.
- Subject extraction MUST produce a previewable cutout that can be copied, shared or exported; corrections and unavailable models MUST be visible through the API.
- Before recognition and search release acceptance, a consented labelled corpus MUST specify languages, people and pet classes, scenes, video moments, relevant-result judgements and approved precision and recall or task-success and error thresholds. Performance MUST be published by supported class rather than averaged.
- Index rebuilds, model changes and erroneous classifications MUST be reviewable. No recognition or lookup result MUST be represented as certain identification when it is an inference. Connected model selection and external disclosure MUST follow F-045.

**FileSquid default UI**

- The default UI's search MUST accept natural-language and OCR text queries when those indexes are enabled, and MUST show pending or unsupported index states.

**Distributions**

- A distribution MAY present this capability with its own views and wording; those views MUST use only the FileSquid API and their UI criteria belong to that distribution's PRD.

## F-021: Places, Trips and curation controls

**Priority:** Mandatory. **User Story:** As a traveler, I want geographic and chronological discovery under my control.

**Delivered by:** FileSquid modules behind the API ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)). FileSquid's default UI presents the generic file view in the criteria below. A distribution's views (for example ImageSquid's photo views) carry their own UI criteria in that distribution's PRD.

### Acceptance Criteria

**FileSquid capability**

- Geographic search and map data MUST use recorded or explicitly inferred locations, MUST distinguish the two, and MUST respect unknown dates and time zones.
- Trips and curated time groups MUST retain their underlying files, offer highlights and All, and allow correction of grouping and classification.
- Map and lookup services MUST disclose what coordinates or queries leave the device. An unavailable network map MUST NOT prevent offline metadata use.
- Analysis progress, opt-out and reindexing MUST apply to generated collections without deleting originals or manually entered metadata.

**FileSquid default UI**

- The default UI MUST show a file's recorded or inferred location (labelled as such) in its Metadata panel and MUST allow location search.

**Distributions**

- A distribution MAY present this capability with its own views and wording; those views MUST use only the FileSquid API and their UI criteria belong to that distribution's PRD.

## F-022: Memories and storytelling

**Priority:** Mandatory with generation capability gates. **User Story:** As a family member, I want editable stories from my collection.

**Delivered by:** FileSquid modules behind the API ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)). FileSquid's default UI presents the generic file view in the criteria below. A distribution's views (for example ImageSquid's photo views) carry their own UI criteria in that distribution's PRD.

### Acceptance Criteria

**FileSquid capability**

- The API MUST generate automatic themed memories and MUST support pinning and sharing them (sharing the source files or the rendered presentation).
- The API MUST provide memories as editable stored definitions over existing files: title, subtitle, cover, duration, ordered content, look and soundtrack reference. Prompt-based creation MUST use supported F-045 task routes with owner authorization.
- A memory record MUST carry a **favorite** flag and a **creation origin** (automatic, from a collection or virtual collection, from a selection, from a prompt). The API MUST create a memory from a collection, a virtual collection or a selection, and MUST list memories filtered by origin. A memory favorite is memory state and MUST NOT favorite its source files.
- The render worker MUST render a memory to a movie in supported presentation proportions. Music rights MUST govern export; access to any commercial music service MUST NOT be presumed.
- Controls MUST disable featured-content generation for people and pets, places, and themes and holidays, and MUST reset recommendations and featured-content settings.
- Deleting a memory MUST retain its source files. Excluded, private and hidden content MUST NOT be reintroduced by generation.
- An AI system that interacts directly with a person (including a collection assistant and prompt-assisted memory creation driven by a model) MUST inform that person they are interacting with AI at the start of the first interaction, clearly and accessibly, unless that is obvious to a reasonably well-informed, observant person in that context ([FileSquid U-82](./product-vision.md#further-modules)). The API MUST expose this disclosure so every UI shows it.
- Memories MUST NOT add emotion recognition.

**FileSquid default UI**

- The default UI MAY omit memory views. A rendered memory MUST appear as an ordinary derived file with its provenance.

**Distributions**

- A distribution MAY present this capability with its own views and wording; those views MUST use only the FileSquid API and their UI criteria belong to that distribution's PRD.

## F-023: Slideshows, printing and photo projects

**Priority:** Mandatory; third-party ordering is conditional on an authorized provider. **User Story:** As an owner, I want to present and print collections without rebuilding them elsewhere.

**Delivered by:** FileSquid modules behind the API ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)). FileSquid's default UI presents the generic file view in the criteria below. A distribution's views (for example ImageSquid's photo views) carry their own UI criteria in that distribution's PRD.

### Acceptance Criteria

**FileSquid capability**

- The API MUST provide slideshow definitions over existing files, both instant (built from a selection with a default theme, not saved) and saved (sequence, theme, text, soundtrack reference, slide and presentation timing, transitions, loop). The API MUST publish the available themes, and a distribution MAY add themes. The render worker MUST preview and play a definition and MUST export it to video with its theme applied.
- The render worker MUST produce print-ready portable PDF output for single and multiple files, standard and custom sizes, contact sheets and multi-file layouts.
- Project adapters MUST support installed-provider management, creating, reopening and revising supported projects, and print ordering with explicit destination, cost and submission confirmation.
- Legacy project conversion MUST report unsupported layouts and assets and preserve source files. It MUST NOT claim lossless reconstruction without a validated mapping.
- Slideshow preparation MUST prepare upcoming and adjacent slides with bounded prefetch and suitable display derivatives, preserving sequence and edit versions. Jumps, reversals and setting changes MUST cancel stale preparation. Missing or failed slides MUST be reported, never silently skipped. Warm prepared-slide timing, cold readiness and transition lateness MUST be measured separately under NFR-002.

**FileSquid default UI**

- The default UI MUST let the user print or save a PDF of selected files.

**Distributions**

- A distribution MAY present this capability with its own views and wording; those views MUST use only the FileSquid API and their UI criteria belong to that distribution's PRD.

## F-024: System sharing and extension interfaces

**Priority:** Mandatory with platform-specific equivalents. **User Story:** As an owner, I want media to work with the applications and devices I use.

**Delivered by:** FileSquid (sharing and extension contracts through the API). The ImageSquid distribution adds the photo-specific surfaces (photo picker, wallpaper, external photo editors); capabilities as FileSquid modules ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)).

### Acceptance Criteria

- Platform adapters MUST expose supported share-sheet, email, messaging and nearby-transfer workflows, including AirDrop where the platform offers it; other systems MUST have ordinary export/share alternatives.
- The share control MUST present a small menu of destinations: the enabled app destinations (platform share sheet or installed extensions, email, messaging, nearby transfer, shared albums under [F-033](#f-033-shared-albums-and-hosted-galleries), the AT Protocol connection under [F-064](#f-064-file-activity-comments-and-user-activity-stream)) and **Save as** (an exported copy under [F-010](#f-010-export-and-portable-backup)). The user MUST be able to choose which destinations appear and in what order; each destination MUST show the [F-046](#f-046-media-metadata-inspection-correction-and-privacy-exports) policy that applies before anything is sent.
- External-editor handoff MUST create an explicit working rendition and safely ingest the returned result as a new version; cancellation/failure MUST preserve the original.
- Editing/project extensions MUST declare capabilities, permissions, supported recipe/schema versions and provenance; incompatible extensions MUST be isolated from the catalog and originals.
- System photo-picker/default-library, wallpaper/personalization and companion-input integrations MUST publish platform support and avoid claiming registration paths unavailable to third-party apps.

## F-025: Migration selection, inventory and pilot

**Priority:** Mandatory. **User Story:** As a migrating owner, I want a realistic preservation plan before transferring my collection.

**Delivered by:** FileSquid (migration module, [FE-029](./product-vision.md#migration-with-evidence), [FE-030](./product-vision.md#migration-with-evidence)). Photo-platform routes are presented by the ImageSquid distribution ([F-028](#f-028-supported-migration-routes)); capabilities as FileSquid modules ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)).

### Acceptance Criteria

- The wizard MUST select source namespace/library/account/export, route, destination and scope; estimate logical items, resources, bytes, cloud downloads and staging/destination capacity, revising estimates as discovery proceeds.
- Preflight MUST identify missing archives, unavailable originals, unsupported fields, authorization needs, and sensitive/hidden collection choices; users MUST explicitly include sensitive collections.
- A durable manifest MUST precede substantial transfer and record source IDs, resource roles, versions/fingerprints, expected sizes/checksums, metadata/album references, checkpoints, errors and destination IDs.
- Fallback identity/sidecar matching MUST document precedence and expose ambiguous matches rather than guess silently.
- A representative pilot MUST include stills, large videos, edits, paired media, RAW, multiple albums, old/uncertain dates, Unicode names and unusual metadata. Its fidelity report MUST be reviewable before full execution.
- F-025–028 self-service selection, bulk ingestion/upload, recovery, verification and reports MUST be available without a FileSquid subscription. Managed hosted capacity/transport may require its own entitlement; free tooling MUST NOT imply unlimited free service capacity. Scoped human migration assistance remains a separate F-035 offer.

## F-026: Migration preservation and reconstruction

**Priority:** Mandatory. **User Story:** As a migrating owner, I want my original content and organization preserved with provenance.

**Delivered by:** FileSquid (migration module, [FE-029](./product-vision.md#migration-with-evidence), [FE-030](./product-vision.md#migration-with-evidence)). Photo-platform routes are presented by the ImageSquid distribution ([F-028](#f-028-supported-migration-routes)); capabilities as FileSquid modules ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)).

### Acceptance Criteria

- Migration MUST preserve exact source-provided originals, separate linked edited renditions, raw/unknown sidecars, opaque adjustment data and Live/motion/RAW-JPEG associations; previously lost quality MUST not be claimed restored.
- Capture dates/zones/GPS, names/captions/keywords and source organization MUST retain source values and mapping provenance. Proposed precedence: explicit reviewed correction, stable-ID sidecar, embedded metadata, then labeled file-time fallback; conflicting raw values MUST remain available.
- Supported folder hierarchy, album names/membership/order, favorites and hidden/archive states MUST be reconstructed across archive parts. Smart rules MUST map only when supported; otherwise offer a labeled membership snapshot if supplied. When a source lets one item belong to many albums (Apple exports and PhotoKit, Takeout cross-part albums), the imported files MUST land in one collection per import route and every source album MUST become a hand-picked virtual collection ([F-004](#f-004-collections-folders-and-navigation)) that preserves the source order and cover, so no source membership is dropped and no "primary" album is chosen for the owner.
- Available people/pet labels, memories/creations, sharing/comments MUST be retained as source data where possible; unavailable recognition and dynamic experiences MUST be reported rather than invented.
- Imported sharing MUST require new destination authorization. Storage deduplication MUST preserve distinct logical identities and all relationships even when files are byte-identical.
- Imported existing C2PA credentials MUST be retained byte-exact, every migrated file MUST receive its F-002 import record and its PROV-O import activity ([F-067](#f-067-prov-o-history-of-every-version-and-change)), and where the `c2pa` module is enabled a new FileSquid import record MUST begin under F-043; F-042 MUST preserve the complete metadata, source-evidence and credential recovery set.

## F-027: Migration execution, recovery and reconciliation

**Priority:** Mandatory. **User Story:** As a migrating owner, I want interruptions to preserve verified progress.

**Delivered by:** FileSquid (migration module, [FE-029](./product-vision.md#migration-with-evidence), [FE-030](./product-vision.md#migration-with-evidence)). Photo-platform routes are presented by the ImageSquid distribution ([F-028](#f-028-supported-migration-routes)); capabilities as FileSquid modules ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)).

### Acceptance Criteria

- Execution MUST follow extract → transfer → verify → catalog registration → staged-data release, with independent resource recovery and confirmed within-file offsets/parts where supported.
- Resume MUST reconcile source version, destination state and confirmed checkpoints; renewed URLs MUST never combine incompatible versions. ETags MUST NOT be assumed cryptographic hashes.
- Pause/resume/cancel, bounded retries/backoff, authorization expiry, throttling, missing drives, full disks and corrupted chunks MUST retain verified work. Persistent failures MUST enter visible quarantine; retry-unresolved MUST avoid repeating completed resources.
- Crashes between file write and catalog commit MUST be reconciled; partial data MUST never be represented as a verified original. Archive decompression or PhotoKit extraction MAY restart only the affected unit when offset resume is unavailable.
- Progress MUST distinguish discovery, extraction, transfer, verification and relationship reconstruction. Readable and machine-readable reports MUST account for every inventoried item and relationship as verified/reused/excluded/unsupported/unavailable/failed/ambiguous, with full success distinct from completion with issues.
- Live follow-up scans MUST catch permitted source changes without propagating source deletions into destination copies; snapshot exports MUST explain that a new export is needed for later changes. Completion MUST verify offline destination independence and leave source cleanup/subscription cancellation entirely to the user.
- Verification/registration completion MUST include F-042 canonical durability and F-043 provenance. Fair adaptive scheduling MUST preserve interactive responsiveness and measurable background progress; view caching/prefetch MUST NOT bypass hashes, overwrite originals or starve preservation indefinitely. Batch restarts MUST reuse completed work, with affected-unit replay and actual resumed/repeated bytes visible.

## F-028: Supported migration routes

**Priority:** Mandatory baseline routes; optional conveniences explicitly identified. **User Story:** As an ecosystem user, I want a supported route that matches where my media actually resides.

**Delivered by:** FileSquid modules behind the API ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)). FileSquid's default UI presents the generic file view in the criteria below. A distribution's views (for example ImageSquid's photo views) carry their own UI criteria in that distribution's PRD.

### Acceptance Criteria

**FileSquid capability**

- SRC-01 Apple export folders MUST accept originals, XMP and separate edited renders without assuming flat exports include albums or faces. SRC-02 macOS PhotoKit MUST inventory only authorized accessible libraries and resources, request cloud originals in bounded batches and guarantee completed-resource recovery; arbitrary libraries and byte-offset extraction MUST NOT be assumed.
- SRC-03/04 Google Takeout archives and extracted folders MUST support multiple parts, late JSON sidecars, filename collisions, duplicate content and cross-part albums. Download time MUST NOT replace capture time.
- SRC-07 Windows folders and SRC-08 hydrated OneDrive folders MUST identify local, OneDrive and iCloud origins, detect placeholders, preserve folders and embedded metadata, report application-only metadata gaps, avoid cross-route duplicates, and copy to a destination outside provider synchronization.
- SRC-05 Google Picker selected imports and SRC-09 Graph/OneDrive direct downloads MAY be added; neither makes selected import a whole-library Google route. SRC-06 provider-to-provider transfer MAY be researched only after eligibility and preservation exclusions are verified.
- Each route MUST publish its supported source, export and OS versions, permissions and preservation matrix. Reading a private Apple database MAY be an isolated read-only experiment; it is not the supported foundation.
- No route MUST claim transfer of Apple adjustment history, source sharing permissions, faces or dynamic memories unless that transfer is implemented and tested. Automatic source deletion MUST NOT be included.
- Every inventoried item MUST receive an outcome, a preservation explanation and a recovery path ([FE-029](./product-vision.md#migration-with-evidence)). Migration completion and feature parity MUST remain separate claims.
- Safe migration MUST remain a reusable capability whose placement is governed by its dependencies ([C-041](./product-vision.md#parity-preservation-and-sequence)). Native Apple acquisition MUST remain a macOS adapter, not a Linux prerequisite.
- Broad Google Library API reading and Google Data Portability Photos coverage MUST NOT be assumed; direct connectors MUST revalidate official contracts before implementation.

**FileSquid default UI**

- The default UI MUST offer the folder- and archive-based routes (SRC-01, SRC-03/04, SRC-07/08) through its import flow, with the route's preservation matrix and per-item outcomes visible.

**Distributions**

- A distribution MAY present this capability with its own views and wording; those views MUST use only the FileSquid API and their UI criteria belong to that distribution's PRD.

## F-029: Client encryption and key custody

**Priority:** Mandatory for all hosted private content. **User Story:** As a subscriber, I want operated storage unable to read my private media.

**Delivered by:** FileSquid.

### Acceptance Criteria

- Authorized clients MUST encrypt originals, derivatives/thumbnails, sensitive metadata, recipes and sensitive indexes before private backup/sync upload. This includes full F-041 HTTP exchanges and F-043 provenance under F-042. Ordinary backup operators MUST have no plaintext keys or media-processing path.
- Account authentication, device enrollment, key unlock, recovery and revocation MUST be distinct. Password reset alone MUST NOT imply content recovery; users MUST test recovery material during onboarding.
- New devices MUST receive keys only through an authorized existing device or explicit recovery method. Revocation MUST deny future access and rotate affected future key material; already downloaded copies cannot be recalled.
- Private content MUST be integrity-checked at a key-holding endpoint; hosted workers MAY validate ciphertext and operate transport/accounting without decryption. Cross-account plaintext-hash deduplication MUST NOT expose content equality.
- Browser decryption MUST work independently of a running desktop; unavailable processing MUST expose supported alternatives under F-045. Selected plaintext disclosure to the user's own AI provider or the paid F-044 C2PA publisher is an adopted, scope-bound exception. It MUST NOT disclose library decryption keys, raw HTTP credentials or unrelated content, and MUST NOT become an automatic fallback from private local processing.
- Full captured HTTP evidence MUST be encrypted at rest on the client as well as remotely. User-authorized preservation access/export is distinct from sanitized UI, support diagnostics, telemetry and public publication. Account payment and provider signing authority MUST NOT confer access to that evidence.
- Recovery material MUST include a human-readable **recovery phrase** (a word list from a published word list) as one supported method, shown once, re-displayable after re-authentication, and required to add a device when no authorised device is available. Enabling encryption, adding a device and disabling encryption MUST each require an explicit confirmation that states what becomes inaccessible without the phrase and whether the step is reversible.
- The UI MUST show encryption status for the connected host (enabled on this device, enabled on another device with a set-up action, or not enabled) and a lock indicator on encrypted hosted content; see [F-057](#f-057-desktop-and-server-hosts) for the privacy boundary statement.

## F-030: Personal synchronization and availability

**Priority:** Mandatory for the paid synchronization service. **User Story:** As an owner, I want my devices to converge without losing conflicting work.

**Delivered by:** FileSquid.

### Acceptance Criteria

- Encrypted sync MUST cover originals/derivatives, metadata, collection and rule-based virtual collection definitions, hand-picked virtual collection membership and order ([F-004](#f-004-collections-folders-and-navigation)), favorites, identities, recipes and deletion/recovery state with stable IDs and resumable transport. Source/HTTP evidence, C2PA history and recovery associations MUST remain within F-042's consistent encrypted version model.
- Selective sync, Download Originals, Optimize Storage and on-demand retrieval MUST show preview-only/local/remote/unavailable states. An only verified original MUST not be evicted without another verified recoverable copy under the selected policy.
- Offline changes MUST reconcile idempotently. Concurrent same-field edits and edit recipes MUST retain conflicts/versions; delete-vs-edit MUST not silently discard the edit.
- Device management MUST expose last contact, pending/failed work and revocation. Remote browser access and search MUST use the accessible encrypted replica without requiring desktop uptime.
- Propagated deletions MUST retain recovery according to policy; tombstone compaction MUST not permit a long-offline device to resurrect permanently deleted items.
- Upload/download batches MUST retain durable verified item/part checkpoints across supported interruptions and revalidate authorization, source version and destination state before reuse. Adaptive transfer/prefetch scheduling and cache invalidation MUST follow NFR-003/004, with visible pauses, bounded queues and no loss of completed work or mandatory provenance.

## F-031: Encrypted backup and verified restore

**Priority:** Mandatory for the paid protection service. **User Story:** As an owner, I want recovery independent of synchronized mistakes.

**Delivered by:** FileSquid.

### Acceptance Criteria

- Backup MUST retain versioned recovery points independently of live sync deletions and include encrypted catalog/metadata/recipes with verified media coverage. The managed preservation contract MUST satisfy F-042, including metadata-only history, source/HTTP evidence, C2PA records and independent recovery associations.
- Status MUST distinguish selected, queued, uploaded, verified and recoverable bytes/items; a thumbnail-only transfer MUST not count as original protection.
- Restore MUST support chosen recovery points, scoped/full restores and clean-device key recovery. It MUST verify plaintext integrity on the client and report missing keys/resources/versions.
- Hosted quota, versions counted, retention, deletion expiry and restore traffic terms MUST be visible before purchase. No reliability claim MUST be made before representative loss/recovery exercises pass.
- User-controlled backup destinations SHOULD be supported through published portable formats; managed multi-destination assurance remains an optional service extension in F-035.
- Synchronization MUST NOT be treated as an independent backup ([C-003](./product-vision.md#platforms-originals-and-service-limits)). Cloud encryption MUST NOT be claimed to eliminate service metadata, compromised endpoints or infrastructure-provider dependence ([C-004](./product-vision.md#platforms-originals-and-service-limits)).
- OCFL MUST NOT be treated as encryption or a disaster-recovery service by itself. Ciphertext verification, independent copy policy, user-held keys, retention/erasure and actual restore tests are all required ([C-015](./product-vision.md#provenance-metadata-and-disclosure)).

## F-032: Accounts, entitlements, billing and cancellation

**Priority:** Mandatory for operated paid services. **User Story:** As a subscriber, I want clear charges and continued ownership after cancellation.

**Delivered by:** FileSquid.

### Acceptance Criteria

- Each offered Community, Personal, Family/group or Institutional package MUST state included services, storage/traffic quotas, versions, retention, support scope, billing cadence and cancellation rules before payment. The F-047 institutional service MUST offer annual billing; prices, allowances and contracts are not fixed by this PRD.
- Entitlements MUST govern operated services without weakening encryption or blocking local original access/export. Trial expiry, quota exhaustion and payment failure MUST retain already owned local content.
- Cancellation MUST offer a documented hosted download/restore window independent of subscription renewal, show the scheduled hosted deletion date, and preserve the local library. Window duration is a legal and policy decision required before hosted-service launch.
- Usage accounting MUST use the minimum operational data needed, reconcile retries without double-charging, and distinguish payer from private-library access.
- Account export/deletion, service notices, consent preferences and support access MUST be separately controlled; destructive hosted deletion MUST require explicit scope confirmation.
- The FileSquid paid core is a user subscription account for managed encrypted OCFL **backup and/or** encrypted account **sync**, scaling from personal to institutional quotas on the same infrastructure. Backup and sync remain distinct guarantees. The service is hosted at `*.filesquid.tech` ([F-059](#f-059-hosted-encrypted-backup-and-sync-service)); distributions MAY offer it under their own brand. Local capture, self-service bulk migration and upload tooling, C2PA record maintenance, collection-wide provenance/C2PA scans and exportable reports, metadata controls, preservation packaging and export MUST remain free ([C-031](./product-vision.md#platforms-originals-and-service-limits)).
- After verified paid preservation, FileSquid MAY offer fast, reliable paid **batch jobs** on the owner's OCFL collection with full versioning compliance. Outputs MAY include curation jobs such as transcoding, Parquet datasets and research-grant FAIR reporting, delivered as FileSquid modules with Arkrim as their primary UI ([F-060](#f-060-archival-curation-jobs-and-datasets)). Those jobs MUST NOT replace the backup/sync subscription as the core paid package ([C-051](./product-vision.md#platforms-originals-and-service-limits)).
- Free tooling MUST NOT imply free unlimited hosted capacity or transfer. Scoped human migration assistance and selected publication are separately paid services ([C-032](./product-vision.md#platforms-originals-and-service-limits)). Scale-to-zero MUST apply only to eligible work, not all paid-service infrastructure ([C-006](./product-vision.md#platforms-originals-and-service-limits)).
- Hosted-service rollout MUST follow evidence of useful local operation and verified protection under the [service rollout gates](#service-rollout-gates). These are gates, not calendar commitments ([C-042](./product-vision.md#parity-preservation-and-sequence)).
- Paid F-044 publishing/watermark entitlements MUST be separate from free F-043 import records and F-042 local export; retries MUST not duplicate successful-result billing, and cancellation MUST retain owned output artifacts. F-045 frontier inference MUST remain user-funded without FileSquid credits or resale.
- The free boundary MUST include high local performance, F-025–028 self-service bulk migration/upload tools, F-043 collection-wide scans/reports, F-046 Dublin Core descriptive metadata/privacy controls and F-042 local preservation/recovery/export. Paid capacity/transport, operated verification/support and scoped human assistance MUST be disclosed separately; an unsubscribed owner MUST retain these tools and owned results.
- Managed encrypted backup and/or account sync MUST be presented as the core paid package, with distinct backup/recovery and synchronization guarantees, on subscription accounts that scale from personal to institutional numbers. Extra destinations, OCFL batch jobs, or assistance MUST disclose separate entitlements and MUST NOT require a second basic preservation subscription to keep the library protected. In-app F-044 publication and annual institutional services MUST disclose their separate entitlements and costs.

## F-033: Shared albums and hosted galleries

**Priority:** Mandatory at maturity; hosted operation may be paid. **User Story:** As an owner, I want selected audiences to view or contribute to an album.

**Delivered by:** FileSquid.

### Acceptance Criteria

- Albums MUST support creation, invitations/acceptance, membership removal/unsubscribe, optional contributions, likes/comments, notifications and activity with permissions enforced at every operation.
- Shared deliveries/downloads/contributions MUST use F-046 policy-verified outgoing copies and reviewed accompanying metadata; recipient access MUST NOT expose private originals or their extraction sidecars. Recipients MUST be able to save authorized shared resources into a personal library with provenance. Album deletion MUST not delete the owner's original library assets.
- Public/link galleries MUST require an explicit owner action, disclose bearer access, support expiration/revocation and declare resolution/download/contribution limits. Anonymous access MUST not expose unrelated private assets or keys.
- Recipient encryption and authorization MUST be distinct from storage pooling. Expiration/removal MUST prevent future service access while explaining that downloaded copies remain outside revocation control.
- Comments on individual files are [F-064](#f-064-file-activity-comments-and-user-activity-stream); file and folder shares with roles are [F-066](#f-066-file-and-folder-sharing-roles-links-and-shares-view).

## F-034: Family accounts and shared libraries

**Priority:** Mandatory family and non-family group outcome; operated packaging conditional on the offered service. **User Story:** As a family or group member, I want collaboration while keeping personal media private.

**Delivered by:** FileSquid.

### Acceptance Criteria

- Shared libraries MUST support at least an organizer plus five participants as a proposed parity minimum; actual plan limits MUST be published separately.
- The same shared-library membership and permissions MUST support invited non-family groups. Family relationships MUST NOT be a prerequisite for participation; this extension neither changes the proposed minimum above nor implies enterprise identity provisioning, federation or institution-wide administration.
- Personal/shared/both views, import/move/contribute, collaborative edit/favorite/caption/delete and contributor search MUST respect current membership and client keys.
- F-046 MUST apply the owner's selected scrubbing mode/retained fields and verify each contributed/shared resource before another participant can receive it. Private source-original bytes and metadata remain outside shared keys; personal-device preservation sync remains F-030. This privacy rule takes precedence over any source-platform original-sharing behavior.
- Sharing suggestions MUST be reviewable before contribution; source personal content MUST not automatically become visible through suggestions or pooled storage administration.
- Invitations, departure, removal, closure, ownership transfer where allowed, storage accounting and eligible-copy retention MUST have explicit policies shown before acceptance. Members MUST receive relevant deletion notifications and recovery options.
- Concurrent edits/deletions MUST preserve recoverable alternatives; removal MUST revoke future access and rekey future shared data without exposing personal spaces to the payer.
- File- and folder-level shares inside a shared library follow [F-066](#f-066-file-and-folder-sharing-roles-links-and-shares-view).

## F-035: Optional service extensions

**Priority:** Optional; listed safeguards are mandatory if offered. **User Story:** As an owner with additional needs, I want help or protection beyond the standard service.

**Delivered by:** FileSquid.

### Acceptance Criteria

- FileSquid MAY offer scoped migration/repair, NAS setup/maintenance, priority support, extended retention, multi-destination assurance/recovery exercises, encrypted physical seeding/restore; each offer MUST define its scope, terms, data access, cancellation and evidence of completion.
- Assisted work MUST use explicit owner-granted access and redact diagnostics; the operator MUST NOT gain an undeclared key-recovery override.
- Trusted-contact/inheritance MAY be offered only with opt-in key arrangements, revocation, rehearsed recovery and a defined trigger/authorization policy.
- Physical transfer MUST use encrypted media, verified manifests and explicit custody/return/erasure procedures.
- Professional client galleries MAY add branding, selections and guest uploads as a separate defined offer; they MUST reuse permission/encryption boundaries rather than weaken personal-library protection.
- F-044 paid C2PA publishing/watermarking is committed product scope and MUST NOT be classified as one of these unselected optional extensions.
- F-047 institutional DAM/interoperability and its annual offer are separately committed later scope. Optional professional client galleries remain an independent extension; human migration assistance MUST NOT paywall the free F-025–028 self-service tools.

## F-036: Browser and mobile-web experience

**Priority:** Mandatory. **User Story:** As a user away from my desktop, I want useful access with clear limitations.

**Delivered by:** FileSquid.

### Acceptance Criteria

- Every distribution UI MUST run in an ordinary browser against `filesquid-server` with the same API as the desktop host ([F-056](#f-056-single-filesquid-api), [F-057](#f-057-desktop-and-server-hosts)). The browser/mobile companion MUST be installable as a PWA on supported browsers, with a manifest, install/update flow and clearly scoped offline cache. The encrypted browser host MUST be independently deployable and usable for encrypted uploaded content when all desktops are off ([FileSquid U-22](./product-vision.md#one-api-hosts-and-distributions)). A browser account/session MUST be distinguishable from an optional desktop-only local library, and the UI MUST show which host the user is connected to ([FileSquid U-20](./product-vision.md#one-api-hosts-and-distributions)).
- Mobile web MUST support browse/search, active selected upload, favorites/captions/albums, accessible original download, common metadata/batch actions, saved views, identity corrections, maps, shared participation and simple crop/rotate/straighten/enhance/color/filter/revert.
- Mature mobile web MUST also provide selected-preview offline caching, presentation playback/share/simple edits, simple annotation/trim, supported Live Photo playback, memory-generation requests with endpoint availability status, migration monitoring/review, protection status and supported simple ordering.
- Desktop and `filesquid-server` MUST retain filesystem administration and durable long-running migration. Browser/mobile surfaces of the encrypted browser host MUST display limits for RAW/HDR/depth/audio/AI compute, background uploads, missing originals and suspended jobs; they MUST not silently upload plaintext to compensate.
- Cache size/eviction and logout/lock behavior MUST be visible; native continuous phone-camera backup is explicitly deferred to a separate native-mobile decision.
- Supported browser/mobile browsing MUST use progressive media, bounded adjacent prefetch, cache correctness and responsive controls under NFR-002–004. Browser quota/eviction, storage loss and suspension MUST produce honest availability/resumption states; a cached preview MUST NOT be the only purportedly protected original.
- When F-044 ships, supported desktop and browser surfaces MUST provide its complete in-app selection, price/disclosure review, submission, progress, permitted cancellation/retry, result inspection and retained-output retrieval. Supported provider authentication/payment handoffs MUST return to that workflow; an external provider site alone is insufficient.
- In the encrypted browser host, browser access MUST work when the desktop is off, using uploaded encrypted content and browser-side decryption ([T-002](./product-vision.md#client-processing-and-hosted-mailbox)). The WASM graph MUST NOT depend on native processing. It MUST include the `filesquid-ocfl` and `filesquid-metadata` modules working against the encrypted hosted OCFL root, which is the browser's canonical store; browser storage is a cache ([C-053](./product-vision.md#module-rules), [ADR 025](./architecture/025-client-side-filesquid-engines.md), [ADR 032](./architecture/032-hosting-and-privacy-model.md)). Compiling to WASM MUST NOT be treated as providing a local-disk OCFL root, libvips, FFmpeg or PhotoKit ([FileSquid U-23](./product-vision.md#one-api-hosts-and-distributions), [FE-019](./product-vision.md#cross-platform-access)). Expensive WASM MUST run on workers, incrementally, not on the UI thread ([FileSquid U-36](./product-vision.md#one-api-hosts-and-distributions)).
- A desktop MAY generate advanced derivatives for later browser use. Unavailable work MUST have a visible unsupported or waiting state ([T-004](./product-vision.md#client-processing-and-hosted-mailbox)). Browser cache loss MUST produce clear availability and recovery states ([T-022](./product-vision.md#caching)).
- Mobile web MUST NOT promise continuous background camera backup or long-running work after browser closure ([C-001](./product-vision.md#platforms-originals-and-service-limits)).

## F-037: Setup, updates and diagnostics

**Priority:** Mandatory. **User Story:** As an owner/operator, I want installation and maintenance without fragile manual steps.

**Delivered by:** FileSquid.

### Acceptance Criteria

- Supported platforms MUST receive signed/verifiable installation and update paths from the first consumer release. MVP exception: the FileSquid MVP's early-access macOS DMG MAY ship unsigned, not notarized and without an updater ([ADR 035](./architecture/035-macos-first-desktop-mvp.md)); it MUST be labelled early access and its install guide MUST explain how to open it. Initialization MUST create the local catalog/schema automatically and retain rollback/recovery for failed migrations.
- Optional service credentials MUST be requested only when that service is enabled, using secure prompts/configuration guidance; missing configuration MUST produce actionable UI states without secrets in logs.
- Diagnostics MUST expose job failures, codec capability and redacted environment information; support bundles MUST be user-reviewed and exclude media, keys and sensitive metadata by default.
- Update compatibility MUST cover library/recipe/sync protocol versions, older clients, interrupted updates and safe refusal of incompatible downgrade.
- Installation/update and diagnostic support MUST follow the release platform register: macOS desktop first (the MVP), then `filesquid-server` and Linux and Windows desktop; the encrypted browser host follows ([FE-017](./product-vision.md#cross-platform-access)). `filesquid-server` MUST ship as a container image with documented configuration for its UI directory, storage root and bind address. Platform-specific adapters such as PhotoKit MUST remain explicitly macOS-bound; export-file migration remains a separate portable route.
- Desktop host isolation MUST satisfy [FE-064](./product-vision.md#desktop-host-isolation): the API served in-process with no TCP listener unless an ADR records otherwise, deny-by-default authorisation of every API call, opaque handles, no raw paths or SQL in the UI, out-of-process media workers in the installer, and UI-visible errors without crashing the desktop host ([FileSquid U-18](./product-vision.md#one-api-hosts-and-distributions), [FileSquid U-26](./product-vision.md#one-api-hosts-and-distributions)). Every FileSquid API call MUST be deny-by-default. Native file access, workers and OS adapters MUST stay out of the untrusted webview. Rust MUST authorize the caller before files, SQL or keys ([FileSquid U-18](./product-vision.md#one-api-hosts-and-distributions)).
- Closing the last window MUST NOT destroy job checkpoints. Keep-running after quit MUST be an explicit product setting. Default: checkpoint and resume on next launch; heavy decode/upload MUST NOT continue silently after quit ([FileSquid U-27](./product-vision.md#one-api-hosts-and-distributions)).
- Bundled native libraries MUST ship in the per-OS installer; the product MUST NOT require ImageMagick or FFmpeg on PATH ([FileSquid U-34](./product-vision.md#one-api-hosts-and-distributions)). The default public installer MUST remain LGPL-FFmpeg-clean (`--enable-gpl` and `--enable-nonfree` forbidden) ([FileSquid U-33](./product-vision.md#one-api-hosts-and-distributions)).
- Ordinary restart MUST reach a resumable job state within 30 seconds without rescanning completed media ([FileSquid U-27](./product-vision.md#one-api-hosts-and-distributions)). Declared 4-core/16 GB/SSD desktop defaults remain ≤4 active transfers, ≤2 CPU-heavy workers, 10 GiB staging, and migration process memory ≤2 GiB excluding separately budgeted ML packs ([FileSquid U-28](./product-vision.md#one-api-hosts-and-distributions)).

## F-038: Open interfaces and product exit

**Priority:** Mandatory. **User Story:** As an integrator/owner, I want documented interfaces and a credible exit from FileSquid.

**Delivered by:** FileSquid.

### Acceptance Criteria

- Domain operations MUST be exposed through the one versioned FileSquid API ([F-056](#f-056-single-filesquid-api)); the in-process desktop transport and the `filesquid-server` transport MUST enforce authorization at their respective trust boundaries without exposing arbitrary filesystem access.
- Importers, processors, exporters and extensions MUST use replaceable contracts with declared capabilities; transport checkpoints and keys MUST not be embedded in collaborative review documents.
- The project MUST publish an explicit component/license matrix and contribution policy before public distribution; a component MUST not be marketed as open source before its source/license is available.
- Export/import round trips MUST retain supported content and organization without a FileSquid subscription; optional self-hosted operated-service distribution requires a separate support/licensing decision.

## F-039: Consent, notifications and activity privacy

**Priority:** Mandatory. **User Story:** As an owner, I want private analysis and sharing to remain intentional.

**Delivered by:** FileSquid.

### Acceptance Criteria

- Users MUST control recognition, landmark lookup, memories/holiday suggestions, automatic playback, sensitive collections, activity histories and notification categories without deleting originals.
- Notifications MUST avoid private titles, locations, face names and thumbnails on untrusted/locked surfaces by default.
- Operational analytics MUST minimize data and separate essential service accounting from opt-in product research. Local-only use MUST not require telemetry.
- Consent revocation MUST stop future optional processing/disclosure and offer index/history cleanup where applicable; retaining original media MUST remain a separate choice.
- Users MUST be able to choose, per activity type grouped by category (files, sharing, comments, versions, jobs), whether to receive in-app, mail and push notifications; defaults MUST follow the privacy rules above (no titles or thumbnails on untrusted surfaces). Mail frequency MUST be selectable among immediate, hourly, daily and weekly batches, and a separate daily summary MUST be switchable independently. Events MUST include public-link downloads and uploads to a folder shared by link ([F-066](#f-066-file-and-folder-sharing-roles-links-and-shares-view)).

## F-040: Compatibility and parity register

**Priority:** Mandatory. **User Story:** As a prospective user, I want to know exactly which outcomes and formats are supported.

**Delivered by:** FileSquid (capability register for every host and distribution). The photo-manager parity register belongs to the ImageSquid distribution (ImageSquid C-036, ImageSquid C-037).

### Acceptance Criteria

- Each catalogued capability MUST receive a register entry with platform, format/capture prerequisite, equivalent outcome, requirement ID, test evidence and status: equivalent, adapter equivalent, limited, preservation-only, or unavailable.
- Each committed conditional capability MUST declare a nonempty minimum supported format/model/platform set before its implementation gate; declaring all formats unsupported cannot satisfy a mandatory feature. Unmet parity outcomes require explicit stakeholder acceptance as a documented remaining gap before any qualified parity claim.
- Broad family coverage MUST not hide unresolved child features. A parity claim MUST name the products and versions it was compared against and list remaining limitations; preservation-only MUST not count as editable parity.
- Compatibility fixtures MUST cover macOS/Windows/Linux webviews, major desktop/mobile browsers, supported cameras/codecs, ICC/HDR paths, input methods and languages as those platforms ship. The register MUST also cover F-041 recorder field/body completeness, F-044 in-app surface/publisher/watermark/format/destination coverage, F-045 model/task contracts, F-046 extraction/`dcterms:`/`schema:`/native-write/scrubbing profiles and F-047 institutional mappings/consumers at its separate delivery gate; required capabilities cannot pass with an empty supported set.
- Unsupported states MUST explain what is preserved, viewable, editable and exportable and offer safe alternatives. Apple services MUST map to FileSquid or ImageSquid services/authorized partners rather than imply Apple affiliation or proprietary compatibility.
- Each supported performance profile MUST declare device, browser/webview, media format/dimensions, cache/readiness and concurrency coverage under NFR-002/003, with actual evidence and unresolved limits. macOS desktop is the initial profile, followed by `filesquid-server`; Linux/Windows desktop and the encrypted browser host remain mandatory at maturity. Difficult formats MUST NOT all be labeled unsupported to satisfy a mandatory capability or performance gate.
- The mature product MUST address every feature family that mainstream photo managers offer through an equivalent user outcome, a documented platform adapter, or an explicit capture/compatibility limitation (ImageSquid C-036, ImageSquid FE-027). Equivalence MUST NOT promise identical Apple rendering, access to proprietary services, or reconstruction of information a source never exports (ImageSquid C-037).
- Preservation MUST remain possible even when interpretation or editing is unavailable ([C-038](./product-vision.md#parity-preservation-and-sequence)). Within ImageSquid, ownership and dependable organization MUST precede deeper editing, discovery, connectivity, storytelling, advanced media, and a final parity audit. These are gates, not calendar commitments (ImageSquid C-040). Desktop delivery and validation MUST come first, macOS first (the MVP), then `filesquid-server` and Linux and Windows desktop; the encrypted browser host follows ([FE-017](./product-vision.md#cross-platform-access), [ADR 035](./architecture/035-macos-first-desktop-mvp.md)).
- The photo-manager parity register MUST name the products and versions it was compared against and MUST record **no star ratings** as an unavailable-by-decision entry (favorites and keywords are the classification model; `xmp:Rating` values in imported XMP remain preserved as extracted metadata under [F-046](#f-046-media-metadata-inspection-correction-and-privacy-exports) but are not an owner-editable field).
- The register MUST include a **drive-parity** table listing each documented difference from mainstream cloud drives and self-hosted file-sharing platforms, with its reason: no version thinning, deletion or free-space cap ([F-065](#f-065-file-version-history-and-retention)); no per-folder end-to-end encryption and no administrator switch for it; no login-password-derived keys, no manual re-share after key changes, no master key ([F-029](#f-029-client-encryption-and-key-custody), [F-034](#f-034-family-accounts-and-shared-libraries)); no operator-key encryption at rest and no administrator recovery key on self-run servers ([F-057](#f-057-desktop-and-server-hosts)); thumbnails, previews, indexes and trash inside the encryption boundary; no default AI overview ([F-063](#f-063-file-search-query-language-and-filter-chips)); no activity feed, publication to the user's own AT Protocol account only per explicit request ([F-064](#f-064-file-activity-comments-and-user-activity-stream)); empty folders not persisted and copy creates a new file ([F-062](#f-062-drive-style-views-sorting-selection-and-layout-preferences)); no comment emoji or smart picker and no `sharedwith:external` ([F-063](#f-063-file-search-query-language-and-filter-chips), [F-064](#f-064-file-activity-comments-and-user-activity-stream)); no document editors, calendar, reminders or federated mounts.

## F-041: Source-aware web clipping and HTTP evidence

**Priority:** Mandatory. **User Story:** As a source collector, I want saved images/videos to retain the complete recorded acquisition evidence so I can inspect their source and subsequent history.

**Delivered by:** FileSquid.

### Acceptance Criteria

- A user-invoked browser action/context menu MUST save selected supported images and direct video files with source context; URL-paste and file-import fallbacks MUST label user-supplied context separately from recorder-observed evidence. Local clipping and export MUST NOT require a FileSquid subscription.
- Source records MUST distinguish page/frame URL, selected rendition URL, requested target, final observed location and each observed redirect; retain title, caption/alt text, displayed creator/publisher, stated licence/rights and dates with observation time, extraction method and source attribution. Originality MUST be supported by evidence or labeled source-claimed/unknown.
- For each exchange within the selected acquisition scope, the recorder MUST preserve outgoing HTTP method (including GET, POST and other observed methods), request target, observed protocol version, full retained headers and request body. Bodies MUST include recorded form submissions, API payloads and uploaded data; a sensitive-field allowlist MUST NOT silently discard them from private preservation evidence.
- Request/browser context MUST include recorded User-Agent, Referer, accepted languages/formats, authorization credentials and Cookie values when retained by the recorder. Incoming records MUST include observed response protocol version, status, headers and response body, including error pages, and recorded content type, encoding, cache instructions, modification dates, ETag, redirect destinations and Set-Cookie values. Repeated headers and request/response association MUST survive serialization.
- Binary bodies MUST be preserved as binary resources with digests and lengths, referenced by the provenance sidecar; they MUST NOT be replaced by lossy text, a screenshot or a hash alone. Raw versus decoded/decompressed representations, browser cache/service-worker origin, empty versus absent bodies and any normalization MUST be explicit. Protocol details not observed MUST NOT be invented.
- At least one declared desktop recording route MUST demonstrate complete controlled GET and POST acquisitions, submitted forms/API/upload bodies, credentials/cookies, redirects and error responses. An ordinary extension/HAR/CDP integration MUST NOT be assumed complete without that evidence. Missing, evicted, unsupported, cancelled or truncated components MUST produce per-component gaps and a partial-evidence state; they MUST NOT be called complete capture. Recorder bounds MUST respect NFR-003 and be published for the supported route.
- Capture MUST be scoped to user-selected acquisition and its declared related requests, not unrestricted background browsing. Complete sensitive HTTP evidence MUST be encrypted at rest on the client and before remote upload, with locked access and controlled decryption/export. Sanitized UI/publication copies MUST remain separate from preserved full evidence. Raw records MUST NOT enter operational logs, analytics, automatic AI prompts or third-party publication payloads; stored credentials MUST NOT be replayed automatically. Missing evidence MUST NOT trigger an automatic repeat of a mutating request solely to reconstruct its body; any new acquisition requires the appropriate authorization.
- Each acquired resource MUST receive a versioned PROV-O/JSON-LD sidecar with stable entity/activity/agent IDs, retrieval times in UTC, exact resource digest/length and OCFL object/version/logical-path linkage. Observations, extracted source claims and user corrections MUST remain separately attributed, including extraction location and unavailable/redacted statuses. A source licence statement MUST NOT be represented as verified ownership or permission.
- Media, source sidecars, retained HTTP bodies and optional bounded page evidence MUST be committed into F-042's recovery set. Sidecar construction MUST avoid self-hash/inventory-hash cycles. A snapshot MUST retain its acquisition/sanitization history and be rendered inert. Changed bytes at one URL and identical bytes from different sources MUST retain distinct observations.
- Capture progress/cancellation/retry MUST preserve successfully acquired media and expose incomplete evidence; successful import MUST also initialize F-043. A link, poster or partial segment set MUST NOT count as a completed video. Supported remux/transcode/recording operations MUST identify inputs, missing tracks and output rendition; unsupported streaming/DRM MUST be explicit.
- A Source & History view MUST support inspecting evidence, copying attribution, safe source revisiting, corrections and sidecar export. JSON-LD contexts MUST be inline or pinned and locally resolvable. Untrusted page/OCR/metadata content MUST NOT authorize tool actions or execute active content. Optional AI enrichment MUST follow F-045 and remain labeled inference.
- Acquisition sidecars MUST participate in the common PROV-O history contract of [F-067](#f-067-prov-o-history-of-every-version-and-change) and, where the `c2pa` module is enabled, in F-043's free collection-wide scan/report workflow. Scan/export MUST distinguish source observations, byte integrity, credential validation/trust and depicted truth; no source claim or completed HTTP capture establishes the truth of the depicted scene.

## F-042: OCFL preservation and portable recovery

**Priority:** Mandatory for canonical local preservation from the first FileSquid release and for the paid protection service. **User Story:** As an owner, I want a directly accessible, complete local archive and private hosted protection that remain recoverable after software, provider or account changes.

**Delivered by:** FileSquid.

### Acceptance Criteria

- FileSquid MUST create/version/export OCFL 1.1 objects for coherent **files**, retaining exact acquired originals, compound resources, opaque source sidecars, selected renditions, recipes, the PROV-O history of every version ([F-067](#f-067-prov-o-history-of-every-version-and-change)), HTTP evidence and, where the `c2pa` module is enabled, C2PA records/manifests. Each original is stored under its stored name `<file-uuid>.<ext>` with its metadata record `<file-uuid>.metadata.md` beside it, so original file names are recoverable with ordinary tools ([F-002](#f-002-safe-file-folder-and-device-import)). Each library's storage root MUST also hold its journal OCFL object of sealed JSON Lines segments, and each committed version its own journal entries beside its PROV-O record ([F-061](#f-061-activity-journal-and-live-health-monitoring)). Collection membership MUST NOT duplicate a master for each album. A file's bytes live in exactly one collection.
- Default managed local objects MUST retain ordinary owner-readable canonical media and documented sidecars alongside valid OCFL inventories/version history, without an export prerequisite. Sensitive HTTP evidence MUST remain separately encrypted even in this local profile. Referenced or remote-only originals MUST have explicit location/availability/coverage records and MUST NOT be claimed as independently copied local originals.
- Versioned collection snapshots MUST preserve collections/order, virtual collection definitions including hand-picked membership and order, relationships, authoritative metadata including custom definitions/values, confirmed identities/rejections and exact completed file-object head references. A completed authoritative change MUST produce durable OCFL file versions and a consistent collection snapshot; a database write alone is not sufficient ([T-005](./product-vision.md#canonical-ocfl-state-and-evidence)). Interrupted writes MUST recover to a coherent committed or visibly pending state. Metadata-only changes MUST become recoverable; the operational database or an external vault MUST NOT be the only copy ([FileSquid U-03](./product-vision.md#platform-ownership-and-access)). Processing completion, media verification and preservation protection MUST remain separate states ([T-029](./product-vision.md#durable-jobs)). Uncommitted changes remain pending; unverified hosted coverage remains pending protection.
- Each hosted user's opaque stable identity MUST map to a private, independently exportable OCFL storage root, with enforced authorization separate from path naming and billing. Shared buckets with per-user root prefixes SHOULD be the default; dedicated buckets MAY serve justified policy/residency/isolation needs. Provider limits and isolation/export behavior MUST be validated; changing email/login/subscription MUST NOT change asset identity or disclose another user's root.
- For hosted preservation and explicitly encrypted portable packages, a published FileSquid encryption profile MUST retain standard readable OCFL inventories/digest sidecars over actual stored ciphertext while encrypting sensitive content, names, plaintext digests and metadata. This profile MUST NOT make the default canonical local collection opaque or require an export to access ordinary media/metadata. Opaque encrypted paths MUST minimize disclosure; visible counts/sizes/version activity MUST be documented. Wrapped recovery envelopes and bootstrap instructions MUST be accessible through independently held recovery material without first decrypting the library being recovered. Local encrypted evidence MUST identify its own recovery/key dependency without exposing secrets in readable sidecars.
- Writes MUST serialize competing changes per object and recover interrupted content/inventory publication. OCFL writers MUST reserve `vN` before publishing object-root bytes, write the root inventory sidecar last, and treat incomplete `vN` as unpublished. S3 heads MUST use compare-and-swap ([FileSquid U-42](./product-vision.md#ocfl-module)). A `.git` directory MUST NOT appear in an OCFL object root or storage root ([FileSquid U-50](./product-vision.md#ocfl-module)). Community OCFL tools MAY be CI oracles; they MUST NOT be FileSquid's OCFL engine ([FileSquid U-41](./product-vision.md#ocfl-module), [ADR 016](./architecture/016-rust-ocfl-library.md), [ADR 023](./architecture/023-filesquid-ocfl-engine-and-parquet.md)). The engine default API MUST remain generic OCFL 1.1 ([F-051](#f-051-ocfl-module)). Incomplete versions, root inventory/digest mismatches and unverified recovery associations MUST NOT be labeled protected. Existing completed versions MUST remain recoverable. Retries MUST be idempotent, bounded and corruption-detecting.
- The service MUST verify ciphertext object lengths, checksums and receipts without parsing OCFL; an authorized client MUST verify OCFL structure, decrypted bytes, metadata relationships and usable restoration. Independently protected checkpoints and declared recovery-copy failure boundaries MUST address replacement/corruption of both content and inventory. ETags or same-account regional replication alone MUST NOT establish independent administrative protection.
- Retention, versioned bytes, copies, verification cadence, deletion/purge and export/restore terms MUST be visible before purchase. Lifecycle expiry MUST NOT remove inventory-referenced content; retained/locked copies MUST not be described as immediately erased. A deployed copy policy MUST be priced and verified, including pre-existing media and recovery material.
- Owners MUST be able to export the complete encrypted root with documented recovery tooling and, on an authorized client, ordinary decrypted files or regenerated plaintext OCFL packages. Exported inventories MUST match exported bytes. Local packaging/export, records and previously issued artifacts MUST remain usable after subscription expiry and without the online catalog.
- Recovery acceptance MUST include metadata-only edits, compound media, source-clipped videos, full HTTP evidence, prior credentials, lost primary catalog, interrupted writes and failed/missing keys. Rebuildable caches/model weights MAY be excluded under an explicit policy; accepted corrections and essential evidence MUST NOT be omitted as caches. Format-migration renditions MUST retain originals and their derivation history.
- A clean client MUST reconstruct supported committed media, organization, metadata and provenance from an app-closed ordinary filesystem copy of the documented local collection without its original operational database or a running FileSquid service. Local create/version/copy/export/recover MUST be usable for supported imports and metadata in the first FileSquid release; hosted encryption/copy-policy evidence remains a separate gate. Arbitrary external edits to inventories or signed resources MUST be detected and reported rather than silently adopted as valid history.

## F-043: C2PA records and action disclosure

**Priority:** Conditional mandatory: mandatory wherever the `c2pa` module is enabled, for every import and disclosure-relevant action from then on, and for the backfill of files imported earlier. **User Story:** As an owner of media, I want Content Credentials for my files and for the actions FileSquid takes on them.

**Delivered by:** FileSquid `c2pa` module ([F-054](#f-054-c2pa-module)), which ships with the ImageSquid release ([C-025](./product-vision.md#release-sequence)); FileSquid builds without the module make no C2PA record and no C2PA claim. Every import is tracked and described without this module: OCFL object and version, digest, import record, extracted metadata and PROV-O history ([F-002](#f-002-safe-file-folder-and-device-import), [F-067](#f-067-prov-o-history-of-every-version-and-change)).

### Acceptance Criteria

- Where the `c2pa` module is enabled, every new camera-roll/device, desktop/file/folder, migration, web, received-share or generated-resource import MUST initialize a durable C2PA record bound to verified resource identity, hashes, source/ingredient references and FileSquid import time. This MUST work for free users and offline local imports; no subscription, external signer or embedding support is a prerequisite to starting the record.
- A file imported before the `c2pa` module was enabled for its library MUST receive its initial record from a bounded, resumable backfill job. That record MUST state its own creation time, MUST cite the file's import record ([F-002](#f-002-safe-file-folder-and-device-import)) and the OCFL version that first stored the file (`created` time and SHA-256 digest) as the import evidence and MUST NOT claim FileSquid import time. Where a file has no import record, the backfilled record MUST cite the OCFL version alone and state that no import record exists.
- Where the `c2pa` module is enabled, import completion MUST require both verified resource registration and its initial C2PA record. Failure between them MUST retain recoverable staged media and a retryable pending import rather than silently omit provenance. Re-import/retry MUST not duplicate the same event, while a distinct acquisition retains its own observation.
- Existing original bytes, embedded/external credential resources and available ingredient history MUST remain preserved. FileSquid MUST link its new import/action record to available prior credential history without modifying a signed manifest or original in place. Its import event MUST be distinguishable from prior camera capture and source-supplied claims; importing an existing file MUST NOT invent an original creation/capture event or mark absent credentials as fake. History continuation means linked new events/results, not mutation of prior signed artifacts.
- FileSquid MUST create, validate and display these records through its `filesquid-c2pa` module on every host that processes private content (desktop, `filesquid-server`, and the encrypted browser host), and MUST sign them through it only when a compliant signer is available; prepared/unsigned records remain valid ([F-054](#f-054-c2pa-module), [ADR 026](./architecture/026-filesquid-provenance-engine.md)).
- The record MUST contain versioned prepared claim/assertion/action data suitable for the selected C2PA profile, even before signing. UI/API/export MUST distinguish prepared/unsigned, signed with explicit validation/trust results, failed/unavailable checks and paid published/watermarked states. A prepared record MUST NOT be called a valid signed C2PA manifest. Local signing MAY use a validated compliant credential route; no shared test/private production key may be distributed in clients.
- Every media-changing, metadata-disclosure-changing, AI-generated/modified, ingredient/compositing, export, redaction or watermark operation MUST be evaluated by a versioned C2PA disclosure policy. Actions requiring disclosure MUST append an event with inputs/outputs, application/tool/model/version as applicable, time, parameters needed for truthful disclosure and actor/evidence origin. Catalog-only organization MUST remain preserved without falsely claiming a pixel change.
- Rendering, external-editor return, undo/revert and compound/video operations MUST participate in the same action contract. Publishing MUST not bypass an unrecorded required action. The policy MUST map to supported standard C2PA actions and document extensions, avoiding claims about activity the generating environment did not observe.
- Each C2PA action MUST link, by its operation ID, to the PROV-O activity of the same operation in the PROV-O history of [F-067](#f-067-prov-o-history-of-every-version-and-change), which records every retained media or metadata transformation with or without this module; the C2PA record MUST NOT replace or rewrite that history.
- Signed manifests MUST remain immutable. Subsequent signed results MUST use the appropriate new manifest and ingredient/history linkage; restricted Update Manifests MUST be used only where their unchanged-content/action conditions apply. External manifests or prepared sidecars MUST preserve unchanged originals; embedding/watermarking MUST target a derivative when it changes source bytes.
- Records, prepared data, signed manifests, validation observations and useful status evidence MUST be preserved/exported under F-042. Status MUST identify trust policy and evidence age; offline unavailable checks, missing ingredients, expiry and revocation MUST not collapse into a blanket truth judgment. Public output MUST contain only its authorized disclosure profile, not private HTTP secrets.
- Free users MUST be able to invoke a collection-wide or selected-scope provenance/C2PA scan with bounded background jobs, progress, cancellation and durable resumption. A scan MUST preserve completed work, revalidate changed resources and expose item-level findings plus an exportable human/machine-readable aggregate report. It MUST NOT silently rewrite originals, append invented acquisition claims or sign records.
- Reports MUST identify selected scope, coverage, incomplete/unavailable items, resource/history versions, validation time and trust policy. Findings MUST distinguish absent credentials, prepared/unsigned records, signed binding failures, unavailable checks and separate trust outcomes; source observations, integrity, signed provenance and depicted truth MUST remain distinct ([C-035](./product-vision.md#provenance-metadata-and-disclosure)). Report export MUST obey the selected disclosure policy, excluding private HTTP secrets unless the owner explicitly authorizes a separate archival evidence export. Offline and unsubscribed operation MUST remain useful with honest unavailable-check states.
- When the provenance/AI-mark retain-class is selected, share and publication MUST NOT strip those marks. When it is not selected, stripping MUST be an explicit policy outcome, previewed and verified ([FE-039](./product-vision.md#c2pa-from-import-onward), [FileSquid U-66](./product-vision.md#metadata-module), [C-014](./product-vision.md#provenance-metadata-and-disclosure)).
- Free bounded collection scans MUST warn when an item has disclosable AI-action history and the selected share or publication route has no human-perceivable AI disclosure ([FE-040](./product-vision.md#c2pa-from-import-onward), [FileSquid U-75](./product-vision.md#c2pa-module)). Scans MUST remain free and MUST NOT silently sign, rewrite or publish media ([FE-041](./product-vision.md#c2pa-from-import-onward)).
- Ephemeral cache generation MUST NOT become a permanent C2PA action unless the output is retained or disclosed; the same rule for PROV-O history is in [F-067](#f-067-prov-o-history-of-every-version-and-change) ([T-037](./product-vision.md#canonical-ocfl-state-and-evidence)).

## F-044: Paid C2PA publishing and watermarking

**Priority:** Mandatory product capability; activation is optional for the user and requires the selected paid entitlement. **User Story:** As a subscriber, I want a third-party publisher to return a C2PA-published and watermarked version of my image with verifiable provenance.

**Delivered by:** FileSquid.

### Acceptance Criteria

- FileSquid MUST integrate a third-party C2PA publisher that supplies signing/publication and a C2PA-linked watermark for supported selected images. A nonempty supported image-format/publisher matrix MUST pass release verification. Provider selection and conformance work MUST resolve implementation; they MUST NOT downgrade the feature to a possible future experiment or silently substitute signature-only output.
- Supported desktop and browser surfaces MUST expose an in-app workflow for selection, price/disclosure review, authorization, submission, progress, permitted cancellation/retry, result inspection and retained-output retrieval. Provider authentication/payment handoffs MAY occur externally where required but MUST return a recoverable status to the app; the provider's website MUST NOT be the sole product workflow. Supported surfaces and any mobile limitations MUST be declared in F-040 before delivery.
- The user MUST be able to select an image/output or bounded automatic publication rule under their paid entitlement. The service MUST show provider, selected media/claims/identity, watermark/output profile, price/quota and retention/retrieval terms. Saved scoped authorization MAY cover recurring jobs; a new destination or expanded disclosure requires updated authorization.
- Publication MUST prepare an F-046 policy-verified derivative using the user's enabled/disabled setting and retained-field choices before any media is sent to the third-party publisher, and retain the untouched original privately. The third-party workflow MUST add the watermark before final format-aware binding/signing of the resulting content and record the applicable watermark/action/soft-binding data. The watermark MUST resolve through the supported mechanism to the intended credential record; a decorative logo alone does not satisfy this requirement.
- The returned package MUST include the watermarked derivative, signed manifest store or supported external manifest, watermark algorithm/version and retrieval locator, signer/certificate/trust evidence, relevant timestamp status and a unique operation receipt. FileSquid MUST verify final asset binding, selected disclosure, watermark/manifest association and required output properties, including F-046 final EXIF/privacy verification, before reporting success.
- Watermarking pixels remotely requires the selected derivative; a digest-only signing endpoint MUST NOT be represented as performing that operation. Client-side provider tooling MAY implement part of the workflow under a validated contract. The private library's decryption keys, raw HTTP credentials/cookies/bodies and unrelated originals MUST NOT be sent as implicit publication context. Provider-observed assertions and client-declared history MUST remain distinguishable.
- Jobs MUST have idempotent submission/result recovery, cancellation boundaries, finite retries, explicit pending/failed status and no duplicate successful-result billing on transport retries. Outages or quota failure MUST preserve free local use and the current C2PA record; they MUST NOT erase history or mark an unpublished result published.
- Named destination and transformation tests MUST validate credential retrieval, binding and watermark recovery under the declared supported conditions, including stripping/reencoding where claimed. Unavailable retrieval, altered content or mismatches MUST be visible. A watermark or valid signature MUST NOT be sold as proof of scene truth, copyright ownership or unobserved historical capture.
- The supplier contract MUST define exact generator/product coverage, key custody, allowed assertions, certificate/timestamp renewal/revocation, output retention, locator portability/availability and incident handling. Already issued artifacts MUST remain downloadable locally after cancellation; any dependency on continuing external lookup MUST have explicit terms and portable recovery evidence.
- Billing MUST distinguish the paid publishing/watermark service from free import records and from storage. Watermarked outputs and all required evidence MUST join the same F-042 preservation history. User-paid frontier inference remains separate under F-045; FileSquid MUST NOT fund it.
- The professional publication profile MUST, at first public exposure of AI-generated or materially AI-manipulated image, audio or video that would falsely appear authentic, apply a human-perceivable disclosure and keep machine-readable provenance/AI marks that the selected policy did not strip ([FE-043](./product-vision.md#paid-c2pa-publication-watermarking-and-ai-disclosure), [FileSquid U-84](./product-vision.md#further-modules)).
- The default visible labelling set MUST be the [EU icons for labelling AI-generated content](https://digital-strategy.ec.europa.eu/en/policies/eu-icons-labelling-ai-generated-content). Another clear, distinguishable, accessible visible or audible form MAY be offered in addition where icons are unsuitable for the medium. A valid C2PA manifest, watermark, or tool-only badge MUST NOT be treated as that human-perceivable disclosure ([FE-044](./product-vision.md#paid-c2pa-publication-watermarking-and-ai-disclosure), [C-016](./product-vision.md#provenance-metadata-and-disclosure), [FileSquid U-84](./product-vision.md#further-modules)).
- Where the owner identifies the content as part of an evidently artistic, creative, satirical, fictional or analogous work, the profile MAY apply a limited appropriate disclosure that does not hamper display or enjoyment. That choice MUST be explicit and previewed ([FE-045](./product-vision.md#paid-c2pa-publication-watermarking-and-ai-disclosure)).
- Original media MUST remain unchanged. Publication MUST remain optional for the user and required as a product capability ([FE-047](./product-vision.md#paid-c2pa-publication-watermarking-and-ai-disclosure)).
- FileSquid MUST NOT market this workflow as EU AI Act certification, as proof that an image is or is not AI-generated in law, as proof of depicted truth, or as satisfaction of every applicable law ([FE-048](./product-vision.md#paid-c2pa-publication-watermarking-and-ai-disclosure), [C-018](./product-vision.md#ai-and-article-50)). FileSquid MUST NOT describe the consumer application as a whole as an Article 50 AI system ([C-021](./product-vision.md#ai-and-article-50)). Releasing FileSquid under a free and open-source licence MUST NOT be documented or marketed as removing Article 50 marking or disclosure duties for features that fall under that article ([C-022](./product-vision.md#ai-and-article-50)). FileSquid MUST NOT require retroactive visible labelling of content generated before 2 August 2026 ([C-023](./product-vision.md#ai-and-article-50)).
- Supported provider authentication and payment handoffs MAY return to the app after a paid C2PA publication job ([FE-066](./product-vision.md#paid-c2pa-publication-watermarking-and-ai-disclosure)).
- Article 50 publication and disclosure work MUST NOT precede ownership, recovery and paid encrypted preservation ([C-024](./product-vision.md#ai-and-article-50)).

## F-045: BYO AI connections and per-task model choice

**Priority:** Mandatory with model/task capability gates. **User Story:** As an owner, I want to use my existing local runner or provider account and choose a suitable model for each collection task.

**Delivered by:** FileSquid.

### Acceptance Criteria

- Users MUST connect an already-running local model service or their own supported frontier-provider account. They MUST pay any inference charges directly through that account/subscription/credits; FileSquid MUST NOT purchase, subsidize or resell frontier inference. Basic library use MUST remain available without an AI connection.
- Connections MUST support configurable OpenAI-compatible endpoints, user-owned authentication and model identifiers as a baseline. Chat Completions MUST provide the broad compatibility profile; Responses SHOULD serve direct OpenAI and compatible routes, with native/runtime adapters where supported account authentication or capabilities require them. An API shape MUST NOT imply a chat-subscription entitlement.
- Users MUST assign task defaults and override the compatible model for an individual job. Capability checks MUST cover image inputs, tool/structured output, embeddings, image outpainting with a mask or target canvas size (the [F-014](#f-014-ai-assisted-cleanup-and-styles) Extend and AI Reframe task class) and relevant limits; model listing alone MUST NOT certify task support. Dedicated people/pet recognition contracts MUST remain distinct from general chat inference. The minimum supported task/model matrix MUST be nonempty.
- Local processing MUST be the default. No local failure, model retirement or quota limit may silently route private data to an unapproved destination. The task MUST disclose selected inputs, model/provider and any additional remote gateway; scoped user authorization governs external content and source-evidence disclosure.
- The AI Connections configuration surface MUST show standing disclosure that is readable without starting a job. The product name in this copy MUST come from the active distribution's brand. Canonical English copy (FileSquid default): "FileSquid sends only the files and text you approve for a task to the endpoint you configure. You pay that service. Encrypted backup does not see this traffic. A local failure does not send your library elsewhere." Localized or shortened variants MUST preserve those facts ([FE-067](./product-vision.md#ai-the-user-chooses)).
- The same surface MUST provide an inspectable explanation (info control or equivalent) titled **How AI Connections work** that MUST include these facts: the owner connects an OpenAI-compatible endpoint (a runner on this computer, a runner they host, or their own provider account); FileSquid does not operate that server and does not sell inference; billing stays with the account on that endpoint; each task has its own connection and model; the library still works with no AI connection; default is processing on this device; FileSquid does not switch a job to another destination if a local model fails or a quota runs out; before a remote job the owner sees destination, selected files, image size, metadata included, and any extra gateway; saved permission can cover a repeating job of that same scope; outgoing files can drop location and other metadata and can use smaller stills instead of originals; face names, album titles, and library keys stay in FileSquid unless a task needs them and the owner allows it; a remote endpoint can read what is sent; turning a connection off later does not erase copies already made there; where the provider allows it, FileSquid asks them not to store responses; suggestions come back for preview and undo; accepted names, captions, and album membership stay in the library if models change.
- Before a newly authorized remote scope, a job overlay MUST show destination identifier, any extra hop, file count, image size, metadata policy, and payer, with Confirm and Cancel. Saved permission MAY cover a repeating job of that same scope.
- When user or developer manuals are published under `docs/`, they MUST include an AI Connections page covering the same facts as the standing disclosure, the inspectable explanation, and the per-job preview.
- Credentials MUST be protected and destination-bound, with supported login/API routes and explicit browser/native limitations. Neither `*.filesquid.tech` nor a distribution's hosted service (for example imagesquid.app) MAY be required in the inference path. LiteLLM MAY be an implementation or a user-operated endpoint; a full LiteLLM clone or user gateway deployment MUST NOT be mandatory.
- Bounded jobs MUST normalize supported responses/errors, streaming/cancellation, retry limits and advisory user usage budgets while preserving provider/model/version. Proposed catalog changes MUST pass permissions/scope validation, support preview/undo and resist instructions embedded in source content.
- Owner-confirmed identities, rejected matches, accepted captions/tags and relationships MUST survive model changes and remain portable. Incompatible embedding indexes MUST rebuild with explicit scope/status without losing the previous usable index before replacement succeeds. Model weights and caches MUST follow the declared F-042 policy.
- AI-generated/modified content and other disclosure-relevant actions MUST update F-043; accepted organization and provenance MUST be included in F-042's recovery set. Connection support and task outcomes MUST be measured separately from any paid-backup conversion hypothesis.
- FileSquid MUST NOT place a general-purpose AI (GPAI) model on the market and MUST NOT mark every BYO output as FileSquid-generated ([FE-051](./product-vision.md#ai-the-user-chooses), [C-019](./product-vision.md#ai-and-article-50)). BYO inference MUST remain the user's provider relationship ([FE-011](./product-vision.md#ai-the-user-chooses)).
- FileSquid MUST preserve the provider's marks and MUST record FileSquid's own actions honestly ([FE-052](./product-vision.md#ai-the-user-chooses)). Direct AI interaction MUST disclose that the person is interacting with AI as in F-022 ([FE-053](./product-vision.md#ai-the-user-chooses)).

## F-046: Media metadata inspection, correction and privacy exports

**Priority:** Mandatory, free core capability. Scrubbing is user-configurable; every sharing/publication route MUST honor the selected policy. **User Story:** As an owner, I want Dublin Core descriptive metadata and EXIF technical metadata in my private collection, and control over which fields remain when I share or publish.

**Delivered by:** FileSquid.

### Acceptance Criteria

- Each import MUST extract all accessible supported EXIF and other native metadata into a private, resource-bound record, preserving namespace/tag identifiers, duplicate/conflicting values, types/units, source location and reader/version. Exact source bytes and opaque metadata blocks MUST remain preserved; unsupported/encrypted/malformed fields MUST have explicit unreadable/unknown states rather than fabricated values or a false complete-extraction claim.
- Photo support MUST distinguish EXIF, IPTC and XMP. Video support MUST use the file's actual container/track/timed metadata and supported XMP, including MOV/MP4 QuickTime-style fields where applicable; it MUST NOT assume a universal video EXIF block. Publish a nonempty photo/video read, edit-overlay, native-write and privacy-export capability matrix, with read-only/sidecar-only outcomes kept distinct.
- FileSquid MUST divert extract, inspect, correct, author, and sidecar-write of metadata to `filesquid-metadata` ([F-052](#f-052-metadata-module)). FileSquid MUST NOT keep a parallel metadata path. The inspector MUST show Obsidian-style populated properties and Add property. Canonical storage is `metadata.md` in OCFL ([T-046](./product-vision.md#canonical-ocfl-state-and-evidence)).
- A private metadata inspector MUST show recorded and effective values with provenance and expose single/batch correction, copy, removal, restoration and conflict review for supported descriptive fields, capture dates/zones and locations. Every displayed pair MUST show its namespace prefix (`exif:`, `dcterms:`, `schema:` or the registered extension prefix). The default view MUST list populated keys only (Markdown-style progressive disclosure); empty `/terms/` properties MUST NOT appear as blank form rows. Camera-clock offsets, unknown zones and capture/container/filesystem dates MUST remain distinguishable. Values recorded by a device, derived by a parser, asserted by the owner or inferred by AI MUST NOT become interchangeable historical facts.
- Owners MUST be able to create, inspect, edit and remove descriptive statements for files through single and batch operations, even when the field was absent from the source. Owner-authored descriptive keys MUST be ISO 15836-2 `/terms/` properties (`dcterms:`), confirmed Schema.org overflow, or terms in a registered extension namespace ([F-052](#f-052-metadata-module)). The `/elements/1.1/` namespace (`dc:`) MUST map to the matching `dcterms:` property on write. When the intended meaning has no `/terms/` property, the product MUST suggest the closest Schema.org term and, after owner confirmation, store it as `schema:{term}`. Versioned sidecars MUST bind statements to stable file/resource identities and record namespace-qualified field IDs, definition/schema versions, supported value types and authorship. The supported matrix MUST include at least text values, field-presence and value-matching search, and corresponding supported virtual-collection predicates; richer types/operators and schema limits MUST be declared before implementation rather than implied universally.
- Descriptive authoring MUST reject unknown `dcterms:` local names and MUST NOT author collection-domain `accrualMethod`, `accrualPeriodicity` or `accrualPolicy` on a file. Extracted IPTC/XMP title, creator, caption, keywords and copyright MUST map onto the matching `dcterms:` properties for the owner-facing record while the raw extraction remains a separate observation. Owner assertions MUST NOT overwrite extracted original values or silently acquire a professional standards namespace. Batch changes MUST preview scope, handle mixed/unsupported values explicitly, support undo and resume without repeating completed items or corrupting metadata bindings.
- Effective metadata MUST drive existing search, chronology, Places/Trips, virtual collections and camera/lens grouping through F-005/006/019/021. Batch operations MUST preview affected items, preserve undo/history and report unsupported or conflicting values. Deterministic extraction/correction/privacy operations MUST work locally without cloud inference; optional inferred enrichment uses F-045 and requires review.
- F-042 OCFL content MUST include versioned extraction records, effective/corrected metadata, `dcterms:` / `schema:` statements, provenance and schema/mapping versions, using a documented JSON profile and interoperable XMP sidecars where supported. Sidecars MUST identify exact file/resource/version and digest; duplicate filenames MUST NOT cause misassociation. Metadata-only corrections, descriptive-statement changes and disclosure-policy changes MUST become recoverable without altering original bytes. A generic JSON extraction dump MUST NOT be represented as an embedded EXIF segment or a conformant XMP file; professional mappings require a named validated profile under F-047.
- **Users MUST be able to enable or disable metadata scrubbing and select the EXIF or embedded video fields that remain populated when media is shared or published.** The privacy default MUST be enabled with an empty retained-field list. In enabled mode, an empty list means full EXIF removal; selected exceptions retain only the approved fields. Disabled mode MUST skip privacy-driven scrubbing and remain clearly identified as such. Both modes and field selection MUST be free, usable locally and independent of paid C2PA publication.
- Saved policies MUST record mode, scope/destination, namespace-qualified retained fields and whether the outgoing filename uses FileId or the original name; users MUST be able to inspect and change them for an individual sharing/publication operation. The outgoing disclosure preview MUST show the effective policy, actual populated retained fields, known unsupported areas and the outgoing filename. Disabling scrubbing or deliberately retaining GPS, identity, camera data or the original filename MUST remain a user choice; there MUST NOT be a hidden mode change or a false scrubbed status. Saved scoped settings MAY avoid repeated prompts for the same choice.
- All FileSquid sharing/publication routes MUST enforce the effective policy, including native share actions, shared albums/libraries and contributions/downloads, hosted galleries/links, partner delivery and the selected media sent to a third-party C2PA publisher. In enabled mode, removal MUST examine every EXIF block and nested GPS/Exif subdirectory, MakerNotes, embedded thumbnails and applicable compound companions. Preserving one chosen field MUST NOT preserve its entire containing block or unrelated opaque data. When exact selective retention is unavailable, offer a supported output/profile or a deliberate policy change rather than silently broaden disclosure.
- Enabled policies MUST also control duplicate or mirrored fields in XMP/IPTC and applicable native video container/track/timed metadata, previews and attached resources; an unselected private value MUST NOT survive merely because it occurs outside the main EXIF block. Necessary structural playback/color/timing data MUST be distinguished from user-disclosable descriptive metadata. The format profile MUST declare what can be inspected, selectively retained and removed. Full EXIF removal MUST NOT be described as removal of identifying content visible or audible in the media.
- Original bytes and full extracted metadata MUST remain private in F-042. Editing an OCFL sidecar alone MUST NOT count as scrubbing the actual outgoing file. Owner archival/original export and encrypted backup/personal-device sync MUST remain distinct preservation operations. Sharing uses only the authorized delivered resource and accompanying disclosure; source-original keys, full private extraction/history/HTTP records MUST NOT be attached implicitly, even when scrubbing is disabled. Public album fields, filenames, sidecars and C2PA assertions MUST obey their reviewed disclosure profile and MUST NOT silently reintroduce values excluded by an enabled policy. For native share actions, downloads, shared albums/libraries and contributions, hosted galleries/links, partner delivery and paid F-044 C2PA publication, the privacy-on default outgoing filename MUST be the file's FileId (UUID version 4) plus the real media extension. Owners MUST have an explicit **Use original filename** control, in the same class of choice as disabling scrubbing or retaining GPS. Choosing it MUST be visible in the outgoing disclosure preview. Saved scoped settings MAY remember that choice per destination. The original filename MUST remain on the private original in F-042. FileId remains the logical identity either way; the control only changes the label on the copy that leaves FileSquid.
- `dcterms:` / `schema:` and extension-namespace statements, unknown extracted fields and their mirrors in sidecars, manifests or reports MUST obey the outgoing disclosure policy. An unrecognized namespace MUST NOT bypass selected privacy controls; unknown disclosure capability MUST be reported and handled as unsupported. Restart, portable export/reimport and metadata-only OCFL recovery MUST preserve supported descriptive schemas/values and exact bindings without requiring a subscription. An unconstrained custom-field namespace MUST NOT be accepted as consumer descriptive metadata ([C-043](./product-vision.md#provenance-metadata-and-disclosure)).
- Native metadata writes and scrubbing MUST preserve media essence, presentation orientation, color/HDR interpretation, audio/video tracks, timing and compound associations on supported lossless paths. Required interpretation MUST be applied before removing its metadata; any needed reencoding/remuxing MUST be explicit and use a tested output profile. Original resources MUST never be rewritten in place. Unsupported native writes MAY remain effective sidecar corrections with an honest status.
- Each outgoing result MUST be reopened and inspected against its selected mode and field rules, checking both removed fields and populated retained fields, including nested/duplicate metadata, previews, companions and delivery metadata. Tool exit-code success alone MUST NOT pass. Unsupported, failed or unverifiable enabled-mode removal MUST block delivery until a supported derivative or user-selected policy change is available; there MUST NOT be a silent original fallback or automatic disabling. Disabled-mode output MUST not be called scrubbed, and format-driven loss MUST not be mistaken for intentional removal. Extraction, policy application and verification outcomes MUST remain distinct.
- F-043 MUST record metadata correction, policy-controlled scrubbing, export and other disclosure-relevant operations with resource lineage. Existing signed originals/manifests MUST remain private and immutable; altered output MUST NOT be presented as the unchanged signed original. For F-044, the effective policy MUST be applied and checked before selected media reaches the publisher; watermarking then final binding/signing MUST use that approved result. FileSquid MUST recheck the returned final asset for unapproved metadata reintroduction and valid credential/watermark binding before delivery. Public provenance MUST use a privacy-reviewed assertion/ingredient profile and expose resulting validation limits truthfully.
- Sharing and publication scrubbing MUST default to enabled with an empty ordinary retained-field list (GPS, camera serials, and similar). Privacy-on outgoing filenames for sharing and publication MUST default to FileId plus extension ([C-033](./product-vision.md#provenance-metadata-and-disclosure)); this is the one deliberate exception to showing and downloading files under their original names ([F-002](#f-002-safe-file-folder-and-device-import), [F-010](#f-010-export-and-portable-backup)), and the owner MAY choose the original name per saved policy. PROV-O history records ([F-067](#f-067-prov-o-history-of-every-version-and-change)) MUST be excluded from shared and published outputs by default; when the owner selects them, agent identities and identity-bearing logical paths MUST be scrubbed and the preview MUST say so. Professional publish MUST offer the provenance/AI-mark retain-class. Verification of professional AI-disclosure routes MUST include presence or permitted absence of the visible label, presence of selected provenance/AI marks, and the watermark/C2PA result of a paid job ([FE-057](./product-vision.md#metadata-ownership-and-sharing-privacy), [C-034](./product-vision.md#provenance-metadata-and-disclosure)). A sidecar edit MUST NOT be presented as removal of embedded metadata ([C-013](./product-vision.md#provenance-metadata-and-disclosure)).

## F-047: Institutional DAM and IIIF interoperability

**Priority:** Mandatory later institutional product; independently gated from consumer releases and photo-manager parity. **User Story:** As a GLAM collection manager, I want to curate, preserve and publish selected institutional media through declared professional metadata and collection-system interfaces without losing control of originals or private evidence.

**Delivered by:** FileSquid modules ([FE-063](./product-vision.md#institutional-interoperability), [FE-079](./product-vision.md#institutional-interoperability)). The Arkrim distribution is the primary UI; the ImageSquid distribution MAY expose the photo-collection subset.

### Acceptance Criteria

- The institutional product MUST support documented collection-manager, metadata-curator and authorized-publisher workflows for galleries, libraries, archives and museums. Before implementation, the named task set MUST define inputs, outputs, permitted roles, collection volumes and measurable acceptance; any claim to meet or exceed another DAM MUST name the comparison tasks, baseline, criteria and evidence.
- A nonempty supported interoperability matrix MUST identify named collection-management systems/consumer interfaces, versions, import/export directions, stable identifier mappings and professional metadata profiles. At least one declared collection-system integration and each adopted IIIF import/publishing profile MUST pass end-to-end fixtures before institutional completion. Missing/unknown fields, mapping collisions, unsupported relationships and lossy conversions MUST be reported, with original source metadata retained under F-042/046.
- Professional metadata authoring, single/batch mapping, validation and exchange MUST reuse resource-bound F-046 `dcterms:` / `schema:` schemas and attributable corrections. Consumer descriptive metadata remains available independently of F-047; arbitrary JSON or a copied namespace MUST NOT be presented as conformance to a professional profile without profile-specific validation. A registered extension namespace ([F-052](#f-052-metadata-module)), for example RiC-O, stores a profile's terms; it is not conformance on its own.
- Selected IIIF import/viewing MUST follow [IIIF-2](#future-iiif-milestones), with named collections, finite retrieval limits, faithful source attribution, explicit partial/derived states, F-041 evidence and F-042/043 recovery/history. Selected institutional publishing MUST follow [IIIF-3](#future-iiif-milestones), with a concrete consumer, access/withdrawal and funded operational contract. Implementing one profile MUST NOT imply support for every IIIF API.
- Institutional batch watermark/publication MUST allow authorized scope review, preview, bounded resumable jobs and item-level success/failure/retry outcomes, preserving originals and lineage. A C2PA-linked watermark claim MUST satisfy F-044's verified provider/output contract and optional user purchase; ordinary branding MUST be labeled separately. Selected sharing/publication MUST obey F-046 before delivery and after any returned derivative.
- Published files, thumbnails, manifests and metadata MUST come from a separate authorized delivery projection. Private canonical/hosted OCFL roots, keys and full HTTP evidence MUST NOT become public endpoints. Permissions, consent, revocation and downloaded-copy limits MUST remain explicit; group support alone MUST NOT imply an unselected enterprise identity administration system.
- An annual institutional subscription option MUST define operated hosting/capacity/transfer, integrations, support, retention, cancellation and export responsibilities before launch under F-032/NFR-010. Prices and contracts MUST NOT be inferred from this requirement. Local tools, original access and portable recovery MUST survive service cancellation.
- Task-fidelity, concrete partnership and funded-maintenance gates MUST determine implementation readiness, supported profiles and timing. F-047 and RC-014 remain required for institutional completion even while these gates are pending; consumer RC-001–013 and photo-manager parity MUST NOT wait for them. Optional F-035 professional galleries remain a separate offer. Institutional Article 50 work MUST NOT become a prerequisite for the FileSquid or ImageSquid consumer release ([FE-063](./product-vision.md#institutional-interoperability)).
- Publishing of AI-assisted restoration or other materially AI-manipulated collection items MUST include visible first-exposure disclosure (EU icons as the default labelling set), preserved originals and valid C2PA/provenance for delivered derivatives ([FE-061](./product-vision.md#institutional-interoperability)).
- Private OCFL roots MUST NOT become public IIIF endpoints ([FE-062](./product-vision.md#institutional-interoperability)). All-filetype records management is FileSquid platform scope ([F-050](#f-050-filesquid-platform-and-distributions), [FileSquid U-15](./product-vision.md#platform-ownership-and-access), [C-025](./product-vision.md#release-sequence)); archival curation jobs and datasets are [F-060](#f-060-archival-curation-jobs-and-datasets). Every F-047 capability MUST be a FileSquid module exposed through the API, with the Arkrim distribution as its primary UI ([FileSquid U-39](./product-vision.md#one-api-hosts-and-distributions), [FE-063](./product-vision.md#institutional-interoperability)).

## F-048: Later hosted private OSS inference

**Priority:** Optional later hosted service after verified paid encrypted preservation. **User Story:** As a subscriber, I want private collections-management inference on FileSquid-operated GPUs running free open-source models, without sending my library to a frontier vendor.

**Delivered by:** FileSquid.

### Acceptance Criteria

- FileSquid MAY offer a later optional hosted service with quota limits, operated alongside `*.filesquid.tech`, that runs free open-source models on FileSquid-owned or operated GPUs for private collections-management tasks.
- That service MUST remain separate from the end-to-end encrypted backup mailbox. It MUST NOT grant mailbox operators general library keys.
- Job inputs MUST be discarded after the job. User media MUST NOT be used for training.
- The service MUST NOT be a general chat API, MUST NOT place a GPAI model on the market, and MUST NOT resell frontier inference ([C-019](./product-vision.md#ai-and-article-50)).
- Delivery MUST wait until paid encrypted preservation is verified. It MUST NOT precede ownership, recovery and the core paid backup offer ([C-024](./product-vision.md#ai-and-article-50), [C-042](./product-vision.md#parity-preservation-and-sequence)).

## F-049: Product component terminology and item type

**Priority:** Mandatory. **User Story:** As an owner, I want FileSquid and every distribution to share one internal vocabulary, while the ImageSquid interface still talks like a photo app.

**Delivered by:** FileSquid.

### Acceptance Criteria

- Internal code, schemas, APIs, this PRD and [architecture.md](./architecture.md) MUST use **repository**, **library**, **collection**, **virtual collection**, **virtual exhibit** and **file** with the meanings in the vision terminology table.
- The item-level type MUST be **file**. MUST NOT mint Photo, Image or Video as domain types. ImageSquid UIs MAY label a given file photo, image or video.
- ImageSquid UIs, help and the ImageSquid SRS UI layer MUST use repo, library, album, gallery and exhibit. They MUST NOT present repository, collection, virtual collection or virtual exhibit as ImageSquid UI nouns where the terminology table defines a distinct ImageSquid label. Primary chrome MUST label the preservation store **Archives**. **OCFL** MAY appear in the advanced inspector, diagnostics, CLI and the FileSquid default UI. Collection skim origin labels MUST be **AI** or **RW** (Real World). The Content Credentials pin MAY remain **CR**. The ImageSquid screenshots are canonical, so these exceptions apply: the sidebar heading over the automatic galleries reads **Collections**; the search placeholder reads “Search photos, collections, dates…”; the C2PA floatover's origin badge uses **Real** or **AI**; the smart query **Hardware Certified** selects hardware-signed C2PA capture claims and is not a certification claim ([C-018](./product-vision.md#ai-and-article-50)).
- The FileSquid default UI MUST use the internal names and MUST NOT inherit ImageSquid album, gallery, exhibit, repo, photo, image or video labels as product nouns. A distribution MUST replace internal names only through its label table keyed by internal names ([FileSquid U-24](./product-vision.md#one-api-hosts-and-distributions)).
- A user or group MAY own more than one library on a repository. A collection belongs to exactly one library. A file's bytes live in exactly one collection. A virtual collection belongs to exactly one library. A virtual collection MUST offer the same UI features as a physical collection and MUST NOT be a second store ([FileSquid U-05](./product-vision.md#platform-ownership-and-access)).
- ImageSquid UIs, help and the ImageSquid SRS UI layer MUST talk like a photo app: repo, library, album, gallery and exhibit, with **Archives** as the preservation-store label (ImageSquid U-05).

## F-050: FileSquid platform and distributions

**Priority:** Mandatory. **User Story:** As an owner, I want one platform that preserves and retrieves any file type, with a user interface made for my audience on top of it.

**Delivered by:** FileSquid.

### Acceptance Criteria

- FileSquid MUST be one product: one Rust backend, one versioned API ([F-056](#f-056-single-filesquid-api)), the `ocfl` and `metadata` modules ([F-051](#f-051-ocfl-module), [F-052](#f-052-metadata-module)), the `c2pa` module ([F-054](#f-054-c2pa-module)) from the ImageSquid release, later modules, and the hosts in [F-057](#f-057-desktop-and-server-hosts). FileSquid inherits every requirement of the former module products.
- FileSquid MUST upload, store and give access to any file type. Unknown types MUST remain exact-byte preservable with disclosed interpretation limits. FileSquid MUST NOT promise a working viewer, editor or metadata mapper for every existing format ([C-027](./product-vision.md#release-sequence), [FileSquid U-15](./product-vision.md#platform-ownership-and-access)).
- FileSquid (the default distribution, with the `ocfl` and `metadata` modules, on desktop and `filesquid-server`) MUST be the first consumer release. Encrypted backup and sync at `*.filesquid.tech` ([F-059](#f-059-hosted-encrypted-backup-and-sync-service)) MUST precede the ImageSquid release. ImageSquid MUST be the second consumer release, delivered as a FileSquid distribution, and the FileSquid `c2pa` module MUST ship with it. FileSquid releases MUST NOT wait for interpretation support for every file type ([C-025](./product-vision.md#release-sequence)).
- The FileSquid MVP MUST be limited to the default UI over the `ocfl` and `metadata` modules on the macOS desktop host, with the scope and exclusions of [C-056](./product-vision.md#release-sequence). The first consumer release MUST add `filesquid-server` and the remaining release-1 scope, and MUST still be limited to the default UI over the `ocfl` and `metadata` modules on both hosts. Media, migration, AI and curation modules MUST NOT be prerequisites for it ([constraint requirements](./product-vision.md#constraint-requirements)).
- After the ImageSquid release, the owner MUST choose the order of the later milestones: the Arkrim archives distribution ([F-060](#f-060-archival-curation-jobs-and-datasets)), IIIF/GLAM interoperability ([F-047](#f-047-institutional-dam-and-iiif-interoperability)) and ProofMode integration. Hosted private OSS inference ([F-048](#f-048-later-hosted-private-oss-inference)) remains a separate optional service after verified paid encrypted preservation. The backup/sync subscription (C-031) and the OCFL-job layer (C-051) stay in force regardless of that order ([C-052](./product-vision.md#release-sequence)).
- Every distribution (ImageSquid, Arkrim, third parties) MUST reuse the FileSquid architecture through the FileSquid API. A distribution MUST NOT introduce a third application language or a backend of its own ([C-026](./product-vision.md#release-sequence), [FileSquid U-29](./product-vision.md#one-api-hosts-and-distributions)).
- The ImageSquid distribution MUST remain the photo and video distribution. Records management for mixed record types (documents, email, datasets, audiovisual files) MUST be FileSquid scope, presented by the default UI and by Arkrim (ImageSquid enterprise evolution).
- Every FileSquid command-line tool MUST be invoked as `fsquid` ([C-057](./product-vision.md#module-rules), [ADR 036](./architecture/036-single-fsquid-cli.md)). One crate, `filesquid-cli`, MUST build the `fsquid` binary with every module's commands; no other CLI binary name is shipped. A command that only one module defines MUST run as `fsquid <command>`; every command MUST also run as `fsquid <module> <command>`; the module group MUST be required only for a command name that more than one module defines, and always for `validate`.
- FileSquid MUST NOT depend on any distribution's code. `apps/filesquid/` MUST stay extractable to its own repository. Each distribution MUST be extractable to its own repository and MUST consume FileSquid only through the published API and UI package ([constraint requirements](./product-vision.md#constraint-requirements), [ADR 029](./architecture/029-filesquid-unified-product-and-distributions.md)).
- FileSquid UIs MUST use the internal names (repository, library, collection, virtual collection, virtual exhibit, file) as in [F-049](#f-049-product-component-terminology-and-item-type). Delivery follows the [release sequence](./product-vision.md#release-sequence): FileSquid first, then encrypted backup and sync, then ImageSquid, then the owner-chosen order among Arkrim, IIIF/GLAM and ProofMode.

## F-051: OCFL module

**Priority:** Mandatory for FileSquid preservation; the module crate MUST remain independently installable and publishable. **User Story:** As an institution or another program, I want to install `filesquid-ocfl` as its own library or tool without installing the FileSquid app.

**Delivered by:** FileSquid (`ocfl` module, crate `filesquid-ocfl`).

### Acceptance Criteria

- `filesquid-ocfl` MUST expose a generic OCFL 1.1 engine: storage-root open, object discovery, NAMASTE/inventory/sidecar load, SHA-256, structural validation, forward-delta write, and one async, runtime-agnostic storage backend interface with local filesystem (native), S3-compatible (native and `wasm32`) and in-memory backends. Logical files MUST stream in bounded chunks; no API may need a whole object in memory. Git MUST NOT be the engine. A `.git` directory MUST NOT appear in an OCFL object root or storage root ([FileSquid U-44](./product-vision.md#ocfl-module), [FileSquid U-50](./product-vision.md#ocfl-module), [FileSquid U-41](./product-vision.md#ocfl-module)). [OCFL community extensions](https://ocfl.github.io/extensions/) MUST be supported where adopted.
- Logical state MUST be resolved through inventory `state` and `manifest` at a caller-pinned object version. Unchanged bytes stored in older version directories MUST appear in that state. Directory-name inference MUST NOT be the resolver ([T-039](./product-vision.md#canonical-ocfl-state-and-evidence), [FileSquid U-43](./product-vision.md#ocfl-module)).
- Writes MUST reserve `vN` exclusively before content (local lease or fenced S3 reservation), write the root inventory sidecar last, treat an incomplete `vN` as unpublished, and reconcile cleanly after a crash. On S3-compatible storage a second writer MUST NOT upload into the same `vN` prefix, and head publication MUST use compare-and-swap. A writer that fails to reserve or loses the head race MUST rebase or report a conflict, never overwrite ([FileSquid U-42](./product-vision.md#ocfl-module), [FileSquid U-45](./product-vision.md#ocfl-module)).
- The module MUST implement the forward-delta change model as a documented public API: stage, status, diff (staged against `head`, and between any two versions), commit, log, checkout, revert and reinstate. Given a working state and a base version it MUST compute the change set by digest (added, changed, renamed, deleted, reinstated) and write only bytes whose digest is not already in the object's manifest. Each commit MUST create exactly one new OCFL version. Revert and reinstate MUST create a new version and MUST NOT modify earlier versions. Staging and checkout MUST NOT write inside the storage root. Every commit on every backend MUST go through the `vN` reservation and head compare-and-swap; a commit whose base `head` is no longer current MUST rebase or report a conflict and MUST NOT publish. Native builds, WebAssembly builds and every version-producing UI action MUST use this same model ([FileSquid U-51](./product-vision.md#ocfl-module), [FileSquid U-52](./product-vision.md#ocfl-module), [FileSquid U-53](./product-vision.md#ocfl-module)).
- The engine MUST stream or copy a logical file and MUST enumerate committed logical state (object id, version, logical path, digest, content locator) without FileSquid catalog, recipe, C2PA, collection-snapshot, or SQLite types. It MUST store opaque caller-supplied bytes and MUST NOT require plaintext ([C-044](./product-vision.md#module-rules), [FileSquid U-47](./product-vision.md#ocfl-module), [FileSquid U-48](./product-vision.md#ocfl-module)).
- `filesquid-ocfl` MUST build for `wasm32-unknown-unknown`. It MUST run natively in the desktop host and in `filesquid-server`, and as WebAssembly in the encrypted browser host. It MUST NOT process private content on the ciphertext-only `*.filesquid.tech` service. Browser storage MUST NOT be a canonical root ([C-053](./product-vision.md#module-rules), [FileSquid U-46](./product-vision.md#ocfl-module), [ADR 032](./architecture/032-hosting-and-privacy-model.md)).
- FileSquid file-object layout, collection snapshots, dual-profile encryption, intent recording and catalog rebuild MUST live above this module in FileSquid's application layer (`filesquid-api` and later modules) and MUST be callers of its public API ([F-042](#f-042-ocfl-preservation-and-portable-recovery), [C-044](./product-vision.md#module-rules), [ADR 010](./architecture/010-ocfl-1-1-dual-profile.md)).
- Default crate features MUST exclude Apache Arrow, Apache Parquet, DuckDB, Iceberg, Axum, Tauri, libvips, FFmpeg and SQLite. A later `parquet` Cargo feature (default off) MAY project through the enumerator to a caller-chosen analytical location ([F-060](#f-060-archival-curation-jobs-and-datasets)). Analytical outputs MUST NOT be written into OCFL reserved paths ([C-045](./product-vision.md#module-rules), [C-046](./product-vision.md#module-rules), [T-041](./product-vision.md#canonical-ocfl-state-and-evidence)). The default build MUST stay small enough for a desktop installer and a browser bundle.
- Each FileSquid packaged release that links `filesquid-ocfl` MUST declare the exact crate version in its lockfile and release notes ([T-040](./product-vision.md#canonical-ocfl-state-and-evidence)).
- `filesquid-ocfl` MUST remain independently installable as a FOSS library for the digital preservation community, with its commands in the `fsquid` CLI ([C-057](./product-vision.md#module-rules)): stage, status, diff, commit, log, checkout, revert, reinstate, list, cat, and `fsquid ocfl validate`. Other programmes MUST be able to install the library and CLI without installing the FileSquid app. The CLI MUST use only the public API. The crate lives in the FileSquid Cargo workspace (`apps/filesquid/crates/ocfl/`), MUST NOT take a default dependency on another FileSquid crate, and MUST build, test and publish on its own (`cargo test -p filesquid-ocfl`). It MAY carry `publish = false` until its API is versioned; after that, crates.io publication under the MIT license SHOULD be the distribution and FileSquid hosts MUST pin the version they link. FileSquid MUST NOT absorb the crate into a private module ([FileSquid U-40](./product-vision.md#ocfl-module), [FileSquid U-49](./product-vision.md#ocfl-module), [C-047](./product-vision.md#module-rules), [T-042](./product-vision.md#canonical-ocfl-state-and-evidence), [T-052](./product-vision.md#api-and-hosts)). Existing community OCFL programs fell short as an embeddable engine; this module fills those gaps and adds crash-safe OCFL 1.1 version publication, S3-compatible storage and later Parquet projection. Layout remains OCFL 1.1.
- FileSquid hosts MUST compile the library into their native graph and the browser WASM bundle. FileSquid MUST NOT shell out to `fsquid` or download a separate OCFL executable for canonical OCFL I/O. GitHub Release `fsquid` binaries are for independent install ([T-043](./product-vision.md#canonical-ocfl-state-and-evidence)).
- `filesquid-ocfl` MUST pass the pinned OCFL 1.1 community fixture suite ([OCFL/fixtures](https://github.com/OCFL/fixtures)). It MUST accept every `good-objects` fixture, report the named warnings for every `warn-objects` fixture, and reject every `bad-objects` fixture. Objects it writes from the `content` fixtures MUST also pass an independent community validator (`ocfl-py` or `rocfl`) before each release. Community fixtures and validators are test oracles only. They MUST NOT be the writer, a bundled sidecar or a server dependency ([FileSquid U-41](./product-vision.md#ocfl-module), [OCFL-S-009](../apps/filesquid/crates/ocfl/resources/srs.md)–[OCFL-S-011](../apps/filesquid/crates/ocfl/resources/srs.md)).
- The user-facing Versions panel and the no-thinning rule are [F-065](#f-065-file-version-history-and-retention).

## F-052: Metadata module

**Priority:** Mandatory for FileSquid metadata; the module crate MUST remain independently installable and publishable. **User Story:** As an institution or another program, I want to install `filesquid-metadata` as its own metadata CRUD engine, extract metadata from files in OCFL, and create or enhance Dublin Core and Schema.org records without installing the FileSquid app.

**Delivered by:** FileSquid (`metadata` module, crate `filesquid-metadata`).

### Acceptance Criteria

- `filesquid-metadata` MUST extract metadata from file bytes or a reader in any supported store (`sidecar`, optional `ocfl`, optional `sqlite`), without a subprocess or filesystem path, then create, read, update and delete namespaced statements (`exif:`, `dcterms:`, `schema:` and registered extension namespaces) without FileSquid catalog, recipe, C2PA, collection-snapshot, distribution or UI types. Unknown formats MUST return an explicit unsupported result ([FileSquid U-54](./product-vision.md#metadata-module)).
- The engine MUST support creating and enhancing Dublin Core `/terms/` records and Schema.org overflow. The `/terms/` register MUST reject unknown `dcterms:` local names and MUST NOT author collection-domain `accrual*` on a file. The fifteen `dc:` names MUST map to `dcterms:` on write ([T-038](./product-vision.md#descriptive-metadata), [FileSquid U-55](./product-vision.md#metadata-module)). The module MUST suggest the closest Schema.org term and MUST store it only after confirmation; close matches MUST NOT silently alias ([FileSquid U-56](./product-vision.md#metadata-module)).
- The engine MUST keep a namespace registry. An owner, distribution or module MAY register an extension namespace (prefix, IRI and an optional term list; for example RiC-O for archivists) and author statements in it, and an owner MAY declare a custom namespace whose keys are free key:value pairs. A registered namespace MUST NOT redefine `dcterms:`, `dc:`, `schema:` or `exif:`. An extension prefix MUST be unique within a repository and map to exactly one IRI; re-registering an existing prefix with a different IRI MUST be rejected. A namespace with a term list MUST reject unknown terms, and a statement whose prefix is not registered MUST be rejected. Extension statements MUST be stored in `metadata.md` as their own YAML map, MUST round-trip through CSV, JSON and XMP interchange, and MUST NOT be mapped onto `dcterms:`. Namespace registrations MUST be recoverable from canonical OCFL state ([FileSquid U-55](./product-vision.md#metadata-module), [C-043](./product-vision.md#provenance-metadata-and-disclosure), [module specification](./product-vision.md#module-specifications)).
- Extract MUST cover EXIF, ID3, IPTC, XMP and other supported embedded packets, mapped onto `dcterms:` or `schema:` while native observations keep their prefix. The module MUST map a deliberate subset of tags and report the rest as native observations or unsupported ([T-044](./product-vision.md#canonical-ocfl-state-and-evidence)).
- The stand-alone UI MUST be the metadata commands of the `fsquid` CLI ([C-057](./product-vision.md#module-rules)): `extract`, `get`, `set`, `export`, `import`, and `fsquid metadata validate`. CSV import/export MUST ship in the first FileSquid release (one row per object; headers `prefix:term`; RFC 4180 quoting; unknown `dcterms:` columns and columns with an unregistered prefix rejected). XMP sidecar read/write MUST ship with CSV. Spreadsheet users and scripts MUST be able to use the CLI without the FileSquid app ([T-045](./product-vision.md#canonical-ocfl-state-and-evidence), [FileSquid U-60](./product-vision.md#metadata-module), [FileSquid U-63](./product-vision.md#metadata-module)).
- JSON sidecar import/export MUST ship with CSV and XMP ([C-049](./product-vision.md#module-rules)). A JSON sidecar is one UTF-8 JSON document per OCFL object that binds `{namespace, term, value}` statements to the OCFL object identifier and the SHA-256 digest of the described logical file ([ADR 022](./architecture/022-dublin-core-descriptive-metadata.md)). The CLI MUST expose it as `export json` and `import json`. Import MUST apply the same `/terms/` register checks as CSV and MUST reject a sidecar whose digest does not match the target file. A JSON sidecar is interchange. It MUST NOT replace `metadata.md` as the canonical store ([T-046](./product-vision.md#canonical-ocfl-state-and-evidence)).
- The authoring API MUST support an Obsidian-style properties UI: populated key/value rows with prefixes (`dcterms:creator`), and Add property searching `/terms/`, then Schema.org, then registered extension namespaces, with an option to add a key in a declared custom namespace. The Metadata panel ([F-055](#f-055-default-ui-and-panel-composition)) MUST present that API; the ImageSquid inspector shows it in its right-hand column ([F-046](#f-046-media-metadata-inspection-correction-and-privacy-exports), [FileSquid U-65](./product-vision.md#metadata-module)).
- Interchange MUST NOT include MODS or SQL ([C-049](./product-vision.md#module-rules)).
- Whenever FileSquid or a distribution stores metadata in an OCFL object, the default MUST be a Markdown logical file in the `metadata.md` format with YAML front matter (Obsidian-style); FileSquid keeps one per file, `<file-uuid>.metadata.md`, beside the stored original `<file-uuid>.<ext>`, holding the original file name, extension, format and MIME type, the append-only `import` section and the descriptive statements; there is no separate `file-record.md` and no per-object `metadata.md` ([F-002](#f-002-safe-file-folder-and-device-import), [ADR 038](./architecture/038-import-file-record.md)). Namespaces are YAML maps (`dcterms:`, `exif:`, `schema:`, and one map per registered extension namespace). The Markdown body MAY hold long-form notes. JSON, XML, XMP, or CSV MAY be stored in OCFL only when a named requirement or integration profile explicitly requires that format. SQLite, PostgreSQL, Elasticsearch and other indexes MUST be rebuildable from the Markdown file. They MUST NOT be the only copy ([T-046](./product-vision.md#canonical-ocfl-state-and-evidence), [FileSquid U-57](./product-vision.md#metadata-module)).
- Persistence MUST go through a pluggable store; the core MUST NOT depend on a particular store. The `sidecar` store is the native default; the `ocfl` store is a feature on native and WebAssembly; the `sqlite` store is a native-only feature. FileSquid MUST use the `ocfl` store, which writes each file's `<file-uuid>.metadata.md` through `filesquid-ocfl` so each edit is a new OCFL version, as canonical. An optional SQLite index MUST be rebuildable from the canonical store and MUST NOT be the only copy, except when the caller explicitly chooses the `sqlite` store ([ADR 027](./architecture/027-filesquid-metadata-pluggable-stores.md), [FileSquid U-58](./product-vision.md#metadata-module), [FileSquid U-59](./product-vision.md#metadata-module), [FileSquid U-61](./product-vision.md#metadata-module)). The crate MUST NOT reimplement OCFL layout or `vN` publication ([C-048](./product-vision.md#module-rules), [T-044](./product-vision.md#canonical-ocfl-state-and-evidence)).
- FileSquid MUST divert all metadata handling to this module. FileSquid MUST NOT keep a parallel extract or write path. Writing metadata back into original bytes is out of scope for the canonical path; originals stay untouched and records live beside them.
- Other programmes MUST be able to install the library and the `fsquid` CLI without installing the FileSquid app. The crate lives in the FileSquid Cargo workspace (`apps/filesquid/crates/metadata/`), MUST NOT take a default dependency on another FileSquid crate except through its optional `ocfl` store, and MUST build, test and publish on its own (`cargo test -p filesquid-metadata`). It MAY carry `publish = false` until its API is versioned; after that, crates.io publication under the MIT license SHOULD be the distribution ([FileSquid U-64](./product-vision.md#metadata-module), [T-052](./product-vision.md#api-and-hosts)).
- After the crate is published, FileSquid MUST pin the published version it links and MUST NOT shell out to `fsquid` for canonical writes.
- Default dependencies MUST exclude Axum, Tauri, SQLite, every distribution and FileSquid application crates. Format extract adapters belong to this crate and MUST NOT pull native media processing (libvips, FFmpeg). The ciphertext-only `*.filesquid.tech` service MUST NOT link `filesquid-metadata` ([FileSquid U-64](./product-vision.md#metadata-module), [FileSquid U-30](./product-vision.md#one-api-hosts-and-distributions)).
- `filesquid-metadata` MUST build for `wasm32-unknown-unknown`. It MUST run natively in the desktop host and in `filesquid-server`, and as WebAssembly in the encrypted browser host. Extract adapters MUST read bytes supplied by the store (through `filesquid-ocfl` in FileSquid), not filesystem paths, and MUST stream within bounded memory; the CLI and file-path helpers stay native-only ([C-053](./product-vision.md#module-rules), [FileSquid U-62](./product-vision.md#metadata-module), [ADR 032](./architecture/032-hosting-and-privacy-model.md)).

## F-053: Virtual exhibit

**Priority:** Mandatory. **User Story:** As a curator, I want a public exhibit of already-public files from my libraries, other repositories and social feeds, without copying those files into a second store.

**Delivered by:** FileSquid.

### Acceptance Criteria

- A virtual exhibit MUST belong to a user on a repository. It MUST NOT belong to a single library. Every host and distribution UI MUST create, rename, populate, reorder, cover-select and delete exhibits.
- A virtual exhibit MAY span collections in different libraries on that repository and public collections on other repositories. Cross-repository members MUST be public references with explicit availability ([FE-003](./product-vision.md#independent-library), [F-009](#f-009-multiple-libraries-and-filesystem-administration)).
- Every member MUST be marked for public access, with no privacy restriction on that exhibited media. Hidden media, private collections and private shared libraries MUST NOT appear ([C-050](./product-vision.md#provenance-metadata-and-disclosure)).
- A virtual exhibit MUST NOT store file bytes. A file's bytes MUST continue to live in exactly one collection. Social-feed posts MUST become files in exactly one home collection on import ([F-041](#f-041-source-aware-web-clipping-and-http-evidence)), then MAY join an exhibit.
- A virtual exhibit MUST offer the same UI features as a physical collection (grid, order, cover, inspector). The FileSquid default UI MUST call it a virtual exhibit; ImageSquid UI copy MUST use **exhibit**.
- Acquisition HTTP evidence, credentials, cookies, keys and encrypted backup MUST remain in the private evidence envelope and MUST NOT be disclosed through an exhibit ([C-010](./product-vision.md#provenance-metadata-and-disclosure), [F-041](#f-041-source-aware-web-clipping-and-http-evidence)).
- Hosted public exhibit delivery MUST remain distinct from the private per-user mailbox root ([FE-034](./product-vision.md#portable-preservation), [F-042](#f-042-ocfl-preservation-and-portable-recovery)). Shared albums and hosted galleries remain [F-033](#f-033-shared-albums-and-hosted-galleries). Private shared libraries remain [F-034](#f-034-family-accounts-and-shared-libraries). Professional client websites remain [F-035](#f-035-optional-service-extensions).
- Newly generated virtual exhibit identifiers MUST be RFC 4122 / RFC 9562 UUID version 4 ([ADR 021](./architecture/021-uuid-v4-identifiers.md), [T-047](./product-vision.md#canonical-ocfl-state-and-evidence)).

## F-054: C2PA module

**Priority:** Mandatory for FileSquid provenance; the module crate MUST remain independently installable and publishable. **User Story:** As a developer, I want to add C2PA reading, validation, minting and editing to my own application by integrating one library, without installing the FileSquid app.

**Delivered by:** FileSquid (`c2pa` module, crate `filesquid-c2pa`).

### Acceptance Criteria

- `filesquid-c2pa` MUST read manifest stores embedded in assets, in sidecar `.c2pa` files and by remote reference, parsing JUMBF boxes and CBOR claims and assertions ([C-054](./product-vision.md#module-rules)).
- It MUST validate hard bindings (data hash, BMFF hash, box hash), COSE_Sign1 claim signatures, certificate chains against caller-supplied trust lists, RFC 3161 timestamps, available revocation status and ingredient chains, and MUST report results with the C2PA specification's validation status codes per manifest and ingredient. Untrusted or unchecked items MUST be reported, not hidden. The crate MUST ship no implicit trust store ([FileSquid U-68](./product-vision.md#c2pa-module), [FileSquid U-70](./product-vision.md#c2pa-module)).
- It MUST mint new manifests (actions, ingredients, hard bindings, AI-generation and training/mining assertions, custom assertions) and sign them through a caller-supplied `Signer` (local keys, hardware modules or remote/third-party services). It MUST NOT operate or imply a certificate authority ([FileSquid U-71](./product-vision.md#c2pa-module)).
- It MUST support edits as new manifests that reference the prior asset as a parent ingredient, including update manifests and assertion redaction as the specification allows. It MUST NOT modify a signed manifest in place ([FileSquid U-72](./product-vision.md#c2pa-module)).
- It MUST embed manifests into supported formats or write sidecars through a format-handler interface, starting with JPEG and PNG and extending to HEIF/AVIF, BMFF, WebP and TIFF/DNG. Unsupported formats MUST be reported as unsupported, not silently skipped.
- It MUST produce bounded, resumable collection scans and disclosure-safe scan reports. A free scan MUST check disclosable AI-action history against the selected share or publication route, and scans MUST NOT auto-publish. An optional, default-off feature MUST project C2PA actions and ingredients to the PROV-O JSON-LD profile ([FileSquid U-75](./product-vision.md#c2pa-module), [ADR 015](./architecture/015-prov-o-jsonld-and-c2pa-sdk.md)).
- C2PA logic MUST be implemented from scratch. Cryptographic primitives MUST come from established crates behind a crypto-backend trait and MUST NOT be hand-rolled; the chosen crates MUST be recorded in the module README. The official `c2pa` crate, `c2patool` and C2PA conformance files MUST verify interoperability in CI and MUST NOT be a runtime dependency. The module MUST pin a C2PA specification version and track its changes ([FileSquid U-69](./product-vision.md#c2pa-module), [FileSquid U-73](./product-vision.md#c2pa-module)). Each product that mints trusted credentials MUST pass the C2PA conformance programme before it obtains production claim-signing certificates ([constraint requirements](./product-vision.md#constraint-requirements)).
- Default dependencies MUST exclude `filesquid-ocfl`, every distribution, Axum, Tauri, SQLite, libvips, FFmpeg and other FileSquid crates. The engine core MUST have no server, filesystem or network access: it MUST read and write through caller-supplied async byte sources and sinks, and remote fetches MUST go through a caller-supplied fetcher ([FileSquid U-74](./product-vision.md#c2pa-module), [FileSquid U-76](./product-vision.md#c2pa-module)).
- The crate MUST build for `wasm32-unknown-unknown`. It MUST run natively in the desktop host and in `filesquid-server`, and as WebAssembly in the encrypted browser host. The ciphertext-only `*.filesquid.tech` service MUST NOT link it or process private content with it ([C-054](./product-vision.md#module-rules), [ADR 032](./architecture/032-hosting-and-privacy-model.md)).
- Its commands `read`, `verify`, `sign`, `edit`, `embed` and `report` MUST be available in the `fsquid` CLI ([C-057](./product-vision.md#module-rules)), usable without the FileSquid app. The crate lives in the FileSquid Cargo workspace (`apps/filesquid/crates/c2pa/`), MUST build, test and publish on its own under the MIT license, and FileSquid MUST pin the published version after its API is versioned ([FileSquid U-67](./product-vision.md#c2pa-module), [FileSquid U-76](./product-vision.md#c2pa-module), [T-052](./product-vision.md#api-and-hosts)).

## F-055: Default UI and panel composition

**Priority:** Mandatory for the FileSquid default distribution and for every distribution that shows file facts. **User Story:** As a user, I want to browse and manage my files like a familiar cloud drive, with metadata and credential panels beside each file; as an ImageSquid owner, I want the inspector to show the same trusted panels.

**Delivered by:** FileSquid (`@filesquid/ui`). The ImageSquid distribution composes the panels in its inspector.

### Acceptance Criteria

- File facts MUST have one owner. `filesquid-ocfl` MUST own storage facts only (object identifier, logical path and implied folders, versions, version `created`, `user` and `message`, digests and fixity, size, content location); an extension-based file-type label is a display hint only. Every other file fact (embedded EXIF/ID3/IPTC/XMP, format identification, original creation or capture dates, Dublin Core and Schema.org description, titles, captions, keywords, rights) MUST belong to `filesquid-metadata`. Content Credentials MUST belong to `filesquid-c2pa`. Application meaning (collections, people, favorites, search indexes, recipes) MUST belong to FileSquid's application layer and be presented by distributions, never by a module ([C-055](./product-vision.md#module-rules)).
- Each module MUST expose its UI view operations through the FileSquid API ([F-056](#f-056-single-filesquid-api)) as serializable requests and responses: folder listing by logical-path prefix at a pinned version with paging, file details, per-file version history, object versions, size lookup, fixity, ranged content reads and version-producing writes for `filesquid-ocfl`; the properties API for `filesquid-metadata`; credential summary and validation report for `filesquid-c2pa`.
- FileSquid MUST ship one UI package, `@filesquid/ui`, of standard custom elements written in vanilla TypeScript with no UI framework and built with Vite. Components MUST contain presentation only and MUST reach FileSquid only through the API client, which hides the transport (in-process scheme on desktop, HTTP plus the event channel on `filesquid-server`, later the encrypted browser host) ([T-051](./product-vision.md#api-and-hosts), [ADR 031](./architecture/031-vanilla-typescript-ui-and-white-label-theming.md)).
- The default UI MUST provide a Drive-style file browser: navigation tree, breadcrumb, list and grid views, preview, upload, download, create folder, rename or move, remove and restore, with a sidebar of named panel slots (in the default UI the file panel and its tabs, below). Every file operation that changes content or organisation MUST produce a new OCFL version through the change model in [F-051](#f-051-ocfl-module). OCFL cannot store an empty folder: the UI MAY show a pending empty folder until a file is added beneath it, but MUST NOT persist it. The browser MUST show storage facts only from the `ocfl` module ([FileSquid U-01](./product-vision.md#platform-ownership-and-access), [FE-073](./product-vision.md#independent-library)). The FileSquid default distribution MUST compose it as its app on the desktop host and on `filesquid-server`.
- Every UI surface (browser rows and tiles, tree, breadcrumb, the file panel and its tabs, the Versions view, search suggestions and results, dialogs, and the hidden, deleted and duplicate views) MUST show a file's display name, the owner's original file name or its rename ([F-002](#f-002-safe-file-folder-and-device-import)); the stored name `<file-uuid>.<ext>` and the metadata record's file name MUST NOT be shown to people. The display name is a `metadata` module fact delivered through the API, not an `ocfl` storage fact. The file panel MUST offer three tabs of the same look, feel and behaviour (one layout, a table of property/value rows, copy, search and filter within the tab, empty, loading and error states, the keyboard and accessibility contracts of NFR-006): **File info** (the file's `<file-uuid>.metadata.md` record: original file name, extension, format and MIME type, size, digest, import facts with time, source and OCFL object and version), **Description** (the Metadata panel: Dublin Core `dcterms:` and Schema.org `schema:` statements, viewable and editable) and **Provenance** (the file's PROV-O activities, entities and agents newest first as `prov:` attributes and values, exportable as JSON-LD, with a **View in Logs** link that opens the Logs view filtered to that file or operation) ([F-067](#f-067-prov-o-history-of-every-version-and-change), [F-061](#f-061-activity-journal-and-live-health-monitoring)). There is no per-file audit log tab: the Logs view is the single place to browse journal entries.
- In the FileSquid default UI the file's sidebar MUST be one file panel with three tabs, File info, Description and Provenance, described above; the Versions view MUST open from a button in the file panel's header, and the **Logs** view MUST be reached from a sidebar item placed directly below Settings (keeping the Journal item's icon) for the desktop owner. The Details and Metadata panels of earlier designs are its File info and Description tabs. Mockups: `ui-mockups/v2/25-panel-states.svg`, `28-panel-metadata.svg` and `29-metadata-add-property.svg` (Description tab), `35-panel-file-info.svg`, `36-panel-provenance.svg`, `38-logs-view.svg` and `38-logs-view-dark.svg`, and `39-logs-export.svg`, paths relative to `resources/`, pending in the mockups pull request #85.
- The Metadata panel (the Description tab) MUST present the `metadata` properties API, and the Content Credentials panel the `c2pa` credential summary and validation report. Each panel MUST be mountable in the default browser's sidebar or in any distribution's view. No panel MAY depend on another; distributions register the panels they want in named slots ([FileSquid U-24](./product-vision.md#one-api-hosts-and-distributions), [FileSquid U-65](./product-vision.md#metadata-module), [FileSquid U-77](./product-vision.md#c2pa-module)).
- Components MUST use CSS custom-property design tokens (light and dark), meet the accessibility contracts in [NFR-006](#nfr-006-accessibility-and-usability), and take visible labels from replaceable label tables keyed by internal names. A missing token or label in a distribution MUST fail a test ([FileSquid U-24](./product-vision.md#one-api-hosts-and-distributions)).
- The ImageSquid distribution MUST compose these panels in its inspector, whose tabs are **Metadata | C2PA | Archive** (ImageSquid screen2), instead of reimplementing them: the Metadata tab from the Metadata panel, the C2PA tab from the Content Credentials panel, and the Archive tab from the Details and Versions panels. The Archive tab contains a **Source** section, an ImageSquid view whose data comes from FileSquid's clipping module (ImageSquid SRS-UI-002). ImageSquid MUST reach this data only through the FileSquid API (ImageSquid U-06).
- Views, sort keys, selection, layout preferences and item actions are specified in [F-062](#f-062-drive-style-views-sorting-selection-and-layout-preferences); the search bar in [F-063](#f-063-file-search-query-language-and-filter-chips). The default UI's sidebar is one file panel whose named tab slots hold File info, Description (the Metadata panel) and Provenance ([F-067](#f-067-prov-o-history-of-every-version-and-change), [F-061](#f-061-activity-journal-and-live-health-monitoring)); the Versions view ([F-065](#f-065-file-version-history-and-retention)) opens from its header; Content Credentials, Activity ([F-064](#f-064-file-activity-comments-and-user-activity-stream)) and Sharing ([F-066](#f-066-file-and-folder-sharing-roles-links-and-shares-view)) join as further named slots when their features ship. A distribution MAY compose the same panels differently (for example the ImageSquid inspector). No panel MAY depend on another.

## F-056: Single FileSquid API

**Priority:** Mandatory. **User Story:** As a UI developer, I want one API for every FileSquid feature, and desktop and server to use the same operation contracts.

**Delivered by:** FileSquid (`filesquid-api`).

### Acceptance Criteria

- Every FileSquid feature MUST be exposed through one versioned FileSquid API served by every host. UIs MUST reach features only through that API. UIs MUST NOT call modules, the filesystem or the database directly ([FE-074](./product-vision.md#platform-api-and-hosts), [FileSquid U-16](./product-vision.md#one-api-hosts-and-distributions)).
- The API contract (operation schemas and errors) MUST be versioned, owned by `filesquid-api` and published in the repository (an OpenAPI 3.1 document is the candidate). TypeScript types MUST be generated from it. A compatibility test MUST fail when the contract, the hosts or the generated types drift ([T-048](./product-vision.md#api-and-hosts)).
- The API MUST provide an event channel for job progress and change notifications, so UIs update without refreshing ([FileSquid U-17](./product-vision.md#one-api-hosts-and-distributions)).
- HTTP and WebSocket transports MUST be evaluated against preview and ranged reads, streaming uploads, push events, the no-TCP-port desktop rule and Service Worker interception for the encrypted browser host. The outcome MUST be recorded in an ADR before the API is declared stable ([T-050](./product-vision.md#api-and-hosts), [ADR 030](./architecture/030-filesquid-api-and-transports.md)).
- Every API call MUST be authorised before files, SQL or keys are touched; access MUST be deny-by-default. Responses MUST carry opaque handles, never raw filesystem paths or SQL ([FileSquid U-18](./product-vision.md#one-api-hosts-and-distributions)).
- Module and API boundaries MUST return errors the UI can show. A host MUST NOT crash on a failed request ([FileSquid U-26](./product-vision.md#one-api-hosts-and-distributions)).
- A distribution MUST NOT add an API of its own. A capability a distribution needs MUST be added as a FileSquid module behind this API ([FE-077](./product-vision.md#platform-api-and-hosts)).

## F-057: Desktop and server hosts

**Priority:** Mandatory; the encrypted browser host is mandatory after the first release. **User Story:** As an owner, I want FileSquid on my own computer or on a server I run for my family or organisation, and I want to know who can read my files on each.

**Delivered by:** FileSquid (`filesquid-desktop`, `filesquid-server`; later the encrypted browser host).

### Acceptance Criteria

- FileSquid MUST ship a desktop host (`filesquid-desktop`, Tauri 2) from the MVP, a multi-user server host (`filesquid-server`, Axum) from the first consumer release, and an encrypted browser host afterwards ([FE-075](./product-vision.md#platform-api-and-hosts)). Desktop (macOS first, then Linux and Windows) and `filesquid-server` delivery and validation MUST come first ([FE-017](./product-vision.md#cross-platform-access)).
- The desktop host MUST serve the API in-process through a Tauri custom URI scheme without opening a TCP port, unless an ADR records otherwise. Native file access, workers and OS adapters MUST stay out of the untrusted webview ([T-049](./product-vision.md#api-and-hosts), [FileSquid U-18](./product-vision.md#one-api-hosts-and-distributions)).
- `filesquid-server` MUST serve the API and a selected distribution's UI from one process, using local or S3-compatible storage ([FileSquid U-19](./product-vision.md#one-api-hosts-and-distributions)). It MUST be multi-user: it MUST authenticate users, support groups, personal and group-owned libraries and collection-level access, and enforce group and collection permissions on every API call. Group membership MUST NOT grant ownership of another member's libraries. Billing administration MUST grant no access to private libraries. Operational data MUST live in a rebuildable or operational-only store ([FileSquid U-06](./product-vision.md#platform-ownership-and-access), [T-053](./product-vision.md#api-and-hosts)).
- Private-library processing MUST default to authorized endpoints: the desktop host, a `filesquid-server` that the owner or their organisation operates, or the encrypted browser host ([T-001](./product-vision.md#client-processing-and-hosted-mailbox)).
- Each host MUST state its privacy boundary in the UI: desktop (the owner's device only), self-run server (that server's operator can read files), encrypted hosting (ciphertext only; the operator holds no keys). The UI MUST make it obvious which host the user is connected to ([FileSquid U-20](./product-vision.md#one-api-hosts-and-distributions)).
- The encrypted browser host MUST run the FileSquid modules as WebAssembly against the encrypted hosted OCFL root and MUST work when every desktop is off. Native Rust, WASM and WebCrypto MUST share matching test vectors for SHA-256, AES-256-GCM, HKDF, Ed25519, X25519 and Argon2id; drift is a defect. The WASM graph MUST NOT depend on native processing, and compiling to WASM MUST NOT be treated as providing a local-disk OCFL root, libvips, FFmpeg or PhotoKit ([FileSquid U-22](./product-vision.md#one-api-hosts-and-distributions), [FileSquid U-23](./product-vision.md#one-api-hosts-and-distributions), [T-002](./product-vision.md#client-processing-and-hosted-mailbox)).
- There MUST be one binary per host for all distributions. The desktop installer for a distribution MUST be produced by a configuration overlay (product name, identifier, icons, UI bundle); `filesquid-server` MUST select its UI directory from configuration ([F-058](#f-058-white-label-distributions)).
- Both hosts MUST write unbuffered structured logs to stdout ([NFR-009](#nfr-009-observability-and-capacity-economics)).
- `filesquid-server` MUST NOT offer operator-key encryption at rest. At-rest protection of a self-run server's local or S3-compatible storage is the operator's disk or bucket encryption, outside FileSquid, and the privacy boundary statement MUST say so. Encrypted content remains the ciphertext-only hosted service ([F-059](#f-059-hosted-encrypted-backup-and-sync-service)) and the encrypted browser host. An administrator recovery key for users' content MUST NOT exist as a server feature ([F-035](#f-035-optional-service-extensions)).
- A server administrator MUST be able to switch sharing (new shares), public links, re-sharing and download on or off per server; every switch MUST be journaled and reflected in the UI ([F-066](#f-066-file-and-folder-sharing-roles-links-and-shares-view)).

## F-058: White-label distributions

**Priority:** Mandatory. **User Story:** As a distribution builder, I want to ship a branded product for my audience without writing backend code.

**Delivered by:** FileSquid. The FileSquid default distribution, ImageSquid and later Arkrim are its first distributions.

### Acceptance Criteria

- A distribution MUST be definable by design tokens (light and dark), a label table keyed by internal names, brand assets, view modules and a module set. A distribution MUST NOT contain backend code ([FE-076](./product-vision.md#platform-api-and-hosts), [FileSquid U-24](./product-vision.md#one-api-hosts-and-distributions)).
- Every distribution UI MUST be vanilla TypeScript with standard custom elements and no UI framework, styled only through design tokens (CSS custom properties, light and dark) and worded only through its label table ([T-051](./product-vision.md#api-and-hosts)). A distribution's label table or token file that lacks a key from the FileSquid default MUST fail a test ([FileSquid U-24](./product-vision.md#one-api-hosts-and-distributions)).
- Panels MUST plug into named sidebar slots and MUST NOT depend on each other ([F-055](#f-055-default-ui-and-panel-composition)).
- Every capability a distribution needs MUST be built as a FileSquid module behind the API and MUST be switchable per distribution through its module set ([FE-077](./product-vision.md#platform-api-and-hosts), [FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)). A FileSquid user MUST be able to enable such a module without installing that distribution.
- The FileSquid default distribution MUST use the internal names. The ImageSquid distribution MUST use its label table (repo, library, album, gallery, exhibit; photo, image or video by media kind) and MUST NOT contain backend code ([F-049](#f-049-product-component-terminology-and-item-type), ImageSquid U-05, ImageSquid U-06). Arkrim MUST use internal names and archival terms.
- A distribution MUST run on every FileSquid host through the same API ([F-057](#f-057-desktop-and-server-hosts)). A distribution MUST NOT introduce a third application language or a backend of its own ([C-026](./product-vision.md#release-sequence)).
- Each distribution MUST be extractable to its own repository, consuming FileSquid only through the published API and `@filesquid/ui` ([F-050](#f-050-filesquid-platform-and-distributions)).

## F-059: Hosted encrypted backup and sync service

**Priority:** Mandatory for the paid protection service; it MUST ship before the ImageSquid release. **User Story:** As an owner, I want encrypted backup and sync without running a server, and I want backup to rescue mistakes that sync has propagated.

**Delivered by:** FileSquid (hosted service at `*.filesquid.tech`; client behaviour in [F-029](#f-029-client-encryption-and-key-custody)–[F-032](#f-032-accounts-entitlements-billing-and-cancellation) and [F-042](#f-042-ocfl-preservation-and-portable-recovery)).

### Acceptance Criteria

- FileSquid MUST offer paid encrypted backup and sync servers at `*.filesquid.tech`, before the ImageSquid release ([FE-078](./product-vision.md#platform-api-and-hosts), [FileSquid U-21](./product-vision.md#one-api-hosts-and-distributions), [C-025](./product-vision.md#release-sequence)).
- The service MUST store and relay ciphertext only, authorise keys and fence OCFL head publication. It MUST NOT parse OCFL, hold library keys or hold private plaintext ([T-054](./product-vision.md#api-and-hosts)).
- The service MUST NOT link private application logic, module processing of private content, or native media processing. CI MUST fail its build if libvips, FFmpeg, or module code that processes private plaintext appears in its dependency graph. Cargo features MUST NOT be the only privacy boundary. Its Tokio runtimes MUST NOT run blocking native media work ([FileSquid U-20](./product-vision.md#one-api-hosts-and-distributions), [FileSquid U-30](./product-vision.md#one-api-hosts-and-distributions), [FileSquid U-31](./product-vision.md#one-api-hosts-and-distributions), [FileSquid U-32](./product-vision.md#one-api-hosts-and-distributions)).
- Backup and sync MUST be separate guarantees. Backup MUST keep independent retained recovery points with rehearsed restores. Sync MUST NOT be presented as backup ([FileSquid U-09](./product-vision.md#platform-ownership-and-access), [C-003](./product-vision.md#platforms-originals-and-service-limits)). Cloud encryption MUST NOT be claimed to eliminate service metadata, compromised endpoints or infrastructure-provider dependence ([C-004](./product-vision.md#platforms-originals-and-service-limits)).
- Hosted roots MUST be private per stable user ID. Export and clean-device recovery MUST NOT depend on a live service catalog ([FileSquid U-10](./product-vision.md#platform-ownership-and-access), [FE-034](./product-vision.md#portable-preservation)).
- The same account model MUST scale from personal to institutional quotas. ImageSquid offers the same service under its own brand as a white label of FileSquid's backup and sync features; other distributions MAY ([C-031](./product-vision.md#platforms-originals-and-service-limits)). Quotas, retention and export windows MUST be published before purchase ([FileSquid U-12](./product-vision.md#platform-ownership-and-access)).
- The service MUST NOT take paying users before an independent crypto-protocol audit, a named provider and region, a payment processor and a GDPR role analysis ([constraint requirements](./product-vision.md#constraint-requirements), [T-034](./product-vision.md#later-gates)).

## F-060: Archival curation jobs and datasets

**Priority:** Mandatory later scope (Arkrim); independently gated from the FileSquid and ImageSquid releases. **User Story:** As an archivist, I want versioned batch curation jobs on my collection and FAIR datasets and reports from it.

**Delivered by:** FileSquid modules; the Arkrim distribution is the primary UI.

### Acceptance Criteria

- Curation jobs (for example transcoding), Parquet projection and FAIR reporting MUST be FileSquid modules exposed through the API, with the Arkrim distribution as their primary UI. Any distribution that enables these modules MUST be able to use them ([FE-079](./product-vision.md#institutional-interoperability), [FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)).
- Curation jobs MUST run as durable, resumable jobs whose results are committed as new OCFL versions, never silent rewrites of accepted state ([FileSquid U-85](./product-vision.md#further-modules), [C-051](./product-vision.md#platforms-originals-and-service-limits)).
- Parquet projection MUST go through the `ocfl` module's `parquet` feature (default off) and the T-039 enumerator, using Apache Arrow record batches, to a caller-chosen location outside OCFL object and storage-root reserved paths. A dataset accepted for preservation MUST re-enter as ordinary OCFL object content. Parquet, DuckDB and Iceberg MUST remain derived representations; OCFL remains authoritative ([FileSquid U-86](./product-vision.md#further-modules), [T-041](./product-vision.md#canonical-ocfl-state-and-evidence), [C-045](./product-vision.md#module-rules), [C-046](./product-vision.md#module-rules)).
- FAIR reporting MUST be exposed through the API ([FileSquid U-86](./product-vision.md#further-modules)).
- After verified paid encrypted preservation, these jobs MAY be offered as a paid service. They MUST NOT replace the backup/sync subscription as the core paid package ([C-051](./product-vision.md#platforms-originals-and-service-limits)).
- The Arkrim distribution MUST provide an archives dashboard (ingest, batch curation jobs, dataset and FAIR reporting) over these modules, as a UI layer under [F-058](#f-058-white-label-distributions). Arkrim MUST offer Records in Contexts ([RiC-O](https://www.ica.org/standards/RiC/RiC-O_1-1.html)) descriptions through a registered `filesquid-metadata` extension namespace ([F-052](#f-052-metadata-module), [standards](./product-vision.md#standards-laws-and-regulations)). Institutional interoperability follows [F-047](#f-047-institutional-dam-and-iiif-interoperability). This feature MUST NOT become a prerequisite for the FileSquid or ImageSquid consumer release ([FE-063](./product-vision.md#institutional-interoperability)).

## F-061: Activity journal and live health monitoring

**Priority:** Mandatory for the FileSquid release. **User Story:** As an owner or administrator, I want a complete record of everything FileSquid did and a live view of whether the system and its processes are healthy, inside the app, on every host.

**Delivered by:** FileSquid (`filesquid-api`, both hosts, `@filesquid/ui`); every distribution MAY include the Monitor dashboard.

### Acceptance Criteria

- **100% logging.** Every domain operation, job and process state change, authorisation refusal, credential issue, warning and error MUST be recorded in the activity journal in the host's operational store, on desktop and server alike. Domain operations MUST be journaled in the same transaction as the change or before it is acknowledged; other events MUST NOT be dropped (backpressure). Hosts MUST NOT write text log files; stdout JSON continues ([FE-081](./product-vision.md#platform-api-and-hosts), [T-055](./product-vision.md#api-and-hosts)).
- Journal entries MUST be redacted (opaque IDs; no file contents, titles, captions, pixels, keys or credentials). A server administrator MUST NOT see another user's private metadata through the journal ([T-055](./product-vision.md#api-and-hosts)).
- The journal MUST be searchable (time, actor, operation, outcome, level, process) and exportable as JSON Lines. Retention MUST be configurable, defaulting to keep everything; archiving older segments as an OCFL object MAY be used ([FE-081](./product-vision.md#platform-api-and-hosts)).
- **Join with PROV-O.** Every journal entry MUST carry, besides its id and process or job id, the optional `operation_id` and `object_ref` (`urn:uuid:<object-id>@vN`) of what it concerns, so journal entries and PROV-O activities always join ([F-067](#f-067-prov-o-history-of-every-version-and-change)). Every event that does not become a PROV-O record (refusals, failures, health, process, IPC, authentication, warnings) MUST still be journaled.
- **Recoverable from disk.** The journal entries of a committed operation MUST be written into the same OCFL commit as the change, next to its PROV-O record, so nothing that changed files or metadata is lost when the operational database is lost. All other entries MUST be sealed as JSON Lines segments into journal OCFL objects (one inside each library's storage root for that library's events, so a copied library carries its own history; one per installation for host events) every hour, on app close and on library close. If the database is destroyed, at most one sealing interval (one hour) of entries that changed nothing MAY be lost; this is the accepted bound. Sealed segments are OCFL objects, not log files ([ADR 033](./architecture/033-activity-journal-and-live-health-monitoring.md)).
- **Rebuild.** Rebuilding a library from its files ([F-001](#f-001-independent-local-library-and-ownership)) MUST also rebuild the journal into a fresh operational store from the sealed segments and the per-commit entries, de-duplicated by journal id, so one command restores library, catalog, PROV-O history and journal.
- **Complete coverage.** Browser console warnings and errors MUST reach the journal through a bounded API route as `ui.console.warn` / `ui.console.error` with opaque IDs only, one entry per event: the UI MUST queue and retry, so the route's rate limits only delay events and never merge them, MUST spill a full queue into the page's IndexedDB store and persist undelivered events there at page close, and MUST deliver them on the next open before new events; only when IndexedDB is unavailable or refuses a write MUST the events it could not keep be counted, in one page-close `ui.console.overflow` entry (the one owner-recorded unavoidable gap); OCFL engine warnings (Git, copy and storage-hierarchy guards, lease takeovers) MUST be journaled with operation and object reference; recording a library intent MUST be journaled (`library.intent.recorded`); the `fsquid` CLI MUST journal through the same `filesquid-api` path; periodic `metrics.snapshot` entries MAY be enabled. Stdout keeps the redaction rule (no paths).
- **Logs view and export.** The desktop owner (server administrators later) MUST have a **Logs** view in the default UI, reached from a sidebar item placed directly below Settings (keeping the Journal item's icon), as the single place to browse the audit log: all journal entries newest first with paging, filters by time range, library, operation, outcome and level, and by file, object or operation id, one entry in detail, a link from an entry of a committed operation to its Provenance activity (by operation id and object reference), failed and refused entries, which have no activity, labelled journal-only, live updates through the event channel, and the accessibility contracts of [NFR-006](#nfr-006-accessibility-and-usability). The Provenance tab's **View in Logs** link MUST open the Logs view filtered to that file or operation. The owner MUST be able to export the whole journal or a filtered part to a file as JSON Lines or CSV through the API, journaled as `journal.export`, with the journal's redaction; the export is a convenience, not the recovery mechanism.
- **Live health.** Every long-running process MUST appear in the process registry with state, progress and heartbeat; missed heartbeats MUST mark it stalled. Health MUST be `ok`, `degraded` or `failing` with reasons, from the registry, host checks (storage roots, free space, operational store, queue depths, error rate) and OCFL validation errors in open libraries, and changes MUST be pushed live ([FE-080](./product-vision.md#platform-api-and-hosts), [T-057](./product-vision.md#api-and-hosts)).
- FileSquid MUST ship its own **Monitor** dashboard in `@filesquid/ui`: health, live processes and jobs, queues, throughput and error rates, storage, open OCFL validation findings, and the journal with live tail and export. Only the desktop owner or server administrators MAY open it. ImageSquid reaches it from its More menu ([F-058](#f-058-white-label-distributions)).
- Metrics MUST come from one counter set: JSON through the admin API on every host and Prometheus text at `/metrics` on `filesquid-server` ([T-056](./product-vision.md#api-and-hosts)).
- OTLP export to external sinks MAY be enabled; it MUST be off by default and store-and-forward from the journal so nothing is lost while a sink is down. No message broker, collector or dashboard server is required, and none MAY be bundled into the desktop app ([ADR 033](./architecture/033-activity-journal-and-live-health-monitoring.md)).
- The journal MUST be the single source for the user-facing activity stream and per-file activity in [F-064](#f-064-file-activity-comments-and-user-activity-stream). Journal rows keep opaque IDs; name resolution happens at read time within the caller's permissions. A user MUST NOT receive journal rows about files they may not read.

## F-062: Drive-style views, sorting, selection and layout preferences

**Priority:** Mandatory for the FileSquid default distribution; distributions MAY present the same capability with their own views. **User Story:** As a user coming from a mainstream cloud drive, I want the familiar views, sort orders, selection and layout choices, and I want them to stay as I set them.

**Delivered by:** FileSquid (`filesquid-api` view operations; `@filesquid/ui` default browser). Storage facts come from the `ocfl` module, format identification from the `metadata` module, and application meaning (favorites, recents, tags, preferences) from FileSquid's application layer ([F-055](#f-055-default-ui-and-panel-composition)).

### Acceptance Criteria

**FileSquid capability**

- The API MUST serve these views for the calling user, each as a paged, bounded query with stable ordering ([F-003](#f-003-browsing-selection-and-viewing)): **home** (recent activity on files the user may read, favorites and files shared with the user; no calendar or meeting integration), **all files** the user may read (default), the **personal library** root, **recent** (viewed or modified by the user), **favorites**, **shares** (shared with the user, by the user, and by public link, distinguishable in one view), **tags** (files by `dcterms:subject` value), and **deleted files** (recoverable items under [F-008](#f-008-deletion-hidden-files-and-duplicate-review)).
- Sort keys MUST include name, size, modified (latest OCFL version `created`), **last modified by me** and **last opened by me**, ascending and descending, with folders either on top or mixed with files. Per-user "by me" keys MUST derive from the user's own activity under [F-064](#f-064-file-activity-comments-and-user-activity-stream) and MUST NOT expose other users' activity.
- A query MUST accept a view-local text filter that narrows the current result set, distinct from library-wide search ([F-063](#f-063-file-search-query-language-and-filter-chips)).
- The API MUST report storage used per library and per collection for the calling user, and MUST support sorting a view by size so large files can be found.
- Copy MUST create a new file with its own FileId whose PROV-O history derives it from the source file ([F-067](#f-067-prov-o-history-of-every-version-and-change)), under a new stored name with the same display name; it MUST NOT create a second home for the same bytes ([FileSquid U-05](./product-vision.md#platform-ownership-and-access)). Move MUST change the logical path or collection as a new OCFL version ([F-051](#f-051-ocfl-module)).
- Multi-item download MUST stream one ZIP archive of at most 1,000 members and 10 GiB, applying the [F-046](#f-046-media-metadata-inspection-correction-and-privacy-exports) outgoing policy to each member and listing omitted or unavailable items in a manifest entry; it MUST NOT buffer the archive in memory ([NFR-003](#nfr-003-bounded-processing-and-migration-endurance)).
- Per-user UI preferences (start view, sort key and direction, folder placement, layout, hover card on or off) MUST be stored through the API and restored on every host the user signs in to.

**FileSquid default UI**

- The navigation sidebar MUST list the views above under the label table's names, with a view filter field at the top and the deleted-files entry, a storage-used indicator and settings at the bottom. Browser back and the breadcrumb MUST both return to the previous level.
- One **New** control MUST offer upload files, upload folder and new folder in one menu; drag and drop onto the list MUST upload into the current folder ([F-002](#f-002-safe-file-folder-and-device-import)). A new empty folder MAY appear as pending until a file is added and MUST NOT be persisted ([F-055](#f-055-default-ui-and-panel-composition)).
- The list layout MUST be the default and MUST show selection checkbox, name, size and modified columns, a per-row share control, a per-row actions menu and a footer summary (files, folders, total size). Clicking a column header MUST sort by it; clicking again MUST reverse. The grid layout MUST show a sort menu in place of headers and tiles with thumbnail or type icon, checkbox, name, relative modified time and the actions menu.
- A row checkbox MUST select the item; the header checkbox MUST select all items in the view. With a selection, a bulk action bar MUST show the count and offer at least favorite, move or copy, download and delete, and MUST state the effective selection scope before acting ([F-003](#f-003-browsing-selection-and-viewing)).
- The per-item actions menu MUST offer at least favorite, details, manage tags, rename, move or copy, download, share and delete; on the desktop host it MUST also offer reveal in file manager ([F-009](#f-009-multiple-libraries-and-filesystem-administration)). A right-click MUST open the same menu. Dragging an item onto a folder MUST move it there; move or copy from the menu MUST open a folder picker.
- Shared items MUST show a shared badge and public-link items a link badge on their icon or thumbnail ([F-066](#f-066-file-and-folder-sharing-roles-links-and-shares-view)).
- Clicking a name MUST open the in-app preview for images, video, audio, plain text and PDF; when no preview is supported the UI MUST offer download and say why ([F-003](#f-003-browsing-selection-and-viewing), [F-040](#f-040-compatibility-and-parity-register)).
- Hovering an icon MAY show a preview card with thumbnail, size, modified time and last editor; the card MUST be switchable off in settings and MUST NOT show on touch or when reduced-motion is requested ([NFR-006](#nfr-006-accessibility-and-usability)).
- The user MUST be able to choose the start view (home or personal library root). Home, recent and shares views MAY have a fixed order; the UI MUST say so rather than offer a disabled sort.

**Distributions**

- A distribution MAY present this capability with its own views and wording; those views MUST use only the FileSquid API and their UI criteria belong to that distribution's PRD.

## F-063: File search query language and filter chips

**Priority:** Mandatory. **User Story:** As a user, I want to find files with chips and a small query language the way I do in a familiar cloud drive, across everything I may read, without leaking what I may not.

**Delivered by:** FileSquid (`filesquid-api` search operation; indexes in the operational store, rebuildable from canonical OCFL state under [TC-001](#tc-001-immutable-originals-and-explicit-identity)). [F-006](#f-006-virtual-collections-and-metadata-search) keeps saved-query rules and media quick filters; this item owns the generic query contract both use.

### Acceptance Criteria

- One search operation MUST accept free text plus a documented operator grammar and return paged, bounded, permission-filtered results with stable ordering ([F-003](#f-003-browsing-selection-and-viewing), [NFR-004](#nfr-004-security-and-privacy)). Results MUST never include files the caller may not read, and suggestions MUST NOT reveal hidden or private names ([F-006](#f-006-virtual-collections-and-metadata-search)).
- Free text MUST match the file's display name (its original file name or its rename, never the stored name; [F-002](#f-002-safe-file-folder-and-device-import)), `dcterms:title`, `dcterms:description` and `dcterms:subject`. A quoted phrase MUST match exactly; a leading `-` MUST exclude a term. Content (full-text) matching MUST index PDF and plain text in the first release and Office documents later, each declared in the [F-040](#f-040-compatibility-and-parity-register) register; media files MUST NOT be content-indexed by this operator (OCR and semantic search remain [F-020](#f-020-semantic-search-ocr-and-visual-understanding)); indexing MUST NOT run outside the authorised endpoint ([TC-002](#tc-002-endpoint-processing-and-hosted-encryption)).
- Operators MUST include at least: `type:` (`folder` from logical-path semantics; for files, the format class identified by the `metadata` module: image, video, audio, pdf, document, text, archive, other), `in:` (a library or folder by logical path), `title:`, `owner:`, `creator:` (`dcterms:creator`), `tag:` (`dcterms:subject`), `is:favorite`, `is:deleted`, `is:unfiled` (files whose collection or folder was removed, [F-008](#f-008-deletion-hidden-files-and-duplicate-review)), `before:` / `after:` (latest version `created`, `YYYY-MM-DD`), `createdbefore:` / `createdafter:` (original creation date from the `metadata` module, else first OCFL version), `source:` (acquisition origin from [F-041](#f-041-source-aware-web-clipping-and-http-evidence)), `generator:` (C2PA claim generator from [F-043](#f-043-c2pa-records-and-action-disclosure); absent from the grammar, and so an unknown operator, in builds without the `c2pa` module; where the module is built but switched off, the disabled-module unavailable error applies), `size>` / `size<` (an integer with an optional unit `B`, `KB`, `MB` or `GB` in powers of 1024; no unit means bytes), and, on `filesquid-server`, `sharedwith:` (`me`, a user or group, `public`; the Google value `external` is not adopted because FileSquid has no cross-server recipients), `to:`, `from:` and `pendingowner:` ([F-066](#f-066-file-and-folder-sharing-roles-links-and-shares-view)). An unknown operator MUST return an explicit error, not silently match everything.
- Date boundaries, case folding and Unicode normalisation MUST be specified and tested; a capture date without zone MUST NOT acquire an invented instant ([F-005](#f-005-favorites-metadata-and-keywords)).
- The default ordering of a text search MUST be relevance; the caller MUST be able to switch to any [F-062](#f-062-drive-style-views-sorting-selection-and-layout-preferences) sort key and back to relevance.
- Natural-language questions and answer overviews MUST be available only through the user's own [F-045](#f-045-byo-ai-connections-and-per-task-model-choice) connection, opt-in per task, with the files used listed as sources; FileSquid MUST NOT run an answer model over private files by default ([F-039](#f-039-consent-notifications-and-activity-privacy)).

**FileSquid default UI**

- A global search bar MUST offer filter chips for at least Type, People (owner or sharer), Modified and Location (library or folder); typing part of a chip name MUST suggest the chip; chips MUST combine with text; each chip MUST have a remove control and the row a clear-all control. Selecting a result in the drop-down opens it; Enter shows the full result list with column-header sorting.
- The sidebar view filter MUST narrow the current view only, and the UI MUST show the active scope ("search everywhere" or the view name) and the result summary as chips above the list.
- Unsupported operators on a host or format MUST be reported in the UI as unsupported ([F-040](#f-040-compatibility-and-parity-register)).

## F-064: File activity, comments and user activity stream

**Priority:** Mandatory for `filesquid-server`; the per-file activity view is mandatory on every host. **User Story:** As a user, I want to see what happened to my files and who did it, comment on a file with the people who can see it, and follow my own activity, without becoming an administrator.

**Delivered by:** FileSquid (`filesquid-api` over the [F-061](#f-061-activity-journal-and-live-health-monitoring) journal; comments as application data in the operational store and in the file's OCFL object; `@filesquid/ui` Activity panel and activity view).

### Acceptance Criteria

- The activity journal MUST be the single source of activity ([F-061](#f-061-activity-journal-and-live-health-monitoring)). The API MUST derive a **user activity stream** from it that contains only events on files, collections and shares the caller may read, with opaque IDs resolved to names only within the caller's permissions. A user MUST NOT learn of a file through activity that they could not learn of through browsing ([NFR-004](#nfr-004-security-and-privacy)).
- The stream MUST be filterable by all, by me, by others, file changes (create, modify, delete, rename, move, restore), favorites, shares, comments and tags, and by every enabled module's event types; it MUST be paged, grouped by day and show actor, action, object and time.
- The API MUST serve a **per-file activity** list with the same rules, including versions created, metadata and tag changes, shares, downloads of public links and comments.
- **Comments** MUST be attachable to a file by its owner and by recipients whose share role is commenter or editor ([F-066](#f-066-file-and-folder-sharing-roles-links-and-shares-view)); viewers MUST be able to read comments but MUST NOT post. Comments MUST be visible to everyone who may read the file, editable and deletable by their author, and removable by the file's owner. Comments MUST be journaled, MUST be stored as application data recoverable with the file ([F-042](#f-042-ocfl-preservation-and-portable-recovery)), and MUST follow the [F-046](#f-046-media-metadata-inspection-correction-and-privacy-exports) disclosure policy: they MUST NOT leave FileSquid with a shared or published copy unless the owner selects them.
- Mentions in comments MAY notify the mentioned user under [F-039](#f-039-consent-notifications-and-activity-privacy); a mention MUST NOT grant access. Emoji and smart-picker insertion in comments are not adopted.
- FileSquid MUST NOT expose the activity stream as RSS, Atom or any bearer-link feed. Instead a user MAY connect their own AT Protocol (Bluesky) account and publish a chosen file or activity entry as a post, one explicit request at a time and never automatically. Only files marked public ([F-053](#f-053-virtual-exhibit)) or activity the user may already read are publishable; published media MUST pass the [F-046](#f-046-media-metadata-inspection-correction-and-privacy-exports) outgoing policy with its disclosure preview; connect, publish and disconnect MUST be journaled and recorded as a disclosure action under [F-043](#f-043-c2pa-records-and-action-disclosure); credentials MUST live in the host's secret store, never on the ciphertext-only service; the UI MUST say that the remote network strips metadata and that a published copy is outside FileSquid's control ([ADR 034](./architecture/034-at-protocol-publishing-connection.md)).
- Activity retention and visibility controls remain in [F-007](#f-007-automatic-and-activity-collections) and [F-039](#f-039-consent-notifications-and-activity-privacy); the administrator journal and Monitor dashboard remain in [F-061](#f-061-activity-journal-and-live-health-monitoring).

**FileSquid default UI**

- The sidebar panel slots MUST include an **Activity** panel showing per-file activity and comments with a comment box; the panel MUST NOT depend on other panels ([F-055](#f-055-default-ui-and-panel-composition)).
- An activity view reachable from the header MUST show the user stream with the filters above and relative times.

## F-065: File version history and retention

**Priority:** Mandatory. **User Story:** As a user, I want to see every version of a file, restore or download any of them and label the ones that matter, and I want to know that FileSquid never thins or deletes history behind my back.

**Delivered by:** FileSquid (`ocfl` module change model, [F-051](#f-051-ocfl-module); `filesquid-api` version operations; `@filesquid/ui` Versions panel). Edit recipes remain [F-011](#f-011-non-destructive-editing-workflow).

### Acceptance Criteria

- The API MUST list a file's versions from the OCFL inventory with version number, `created`, `user`, `message`, size and digest, and MUST serve any version's bytes with ranged reads ([F-056](#f-056-single-filesquid-api)).
- Restoring a version MUST be one operation that creates a new version through revert or reinstate ([F-051](#f-051-ocfl-module)); earlier versions MUST remain unchanged. Every content or organisation change MUST produce a version; FileSquid MUST NOT coalesce writes by time or skip versions.
- The user MUST be able to **label** a version. The label MUST be stored as the OCFL version `message` when set at commit time, or as an application-layer label recoverable with the file when set later; it MUST be searchable and shown in the panel.
- FileSquid MUST NOT expire, thin or delete individual versions and MUST NOT remove inventory-referenced content to free space ([F-042](#f-042-ocfl-preservation-and-portable-recovery), [F-001](#f-001-independent-local-library-and-ownership)). Storage used by history MUST be reported per file and per library ([F-062](#f-062-drive-style-views-sorting-selection-and-layout-preferences)); a capacity warning MUST explain hardware or quota limits without discarding data. Whole-object export and deletion remain the owner's explicit, journaled actions under [F-008](#f-008-deletion-hidden-files-and-duplicate-review) and [F-010](#f-010-export-and-portable-backup).
- The [F-040](#f-040-compatibility-and-parity-register) register MUST list "no version expiry, no version deletion, no free-space cap" as a documented difference from drives that thin history.

**FileSquid default UI**

- The Versions panel MUST list versions newest first with label, relative time, size and a per-version menu offering restore, download and edit label; the current version MUST be marked. Deletion MUST NOT appear as an option.

## F-066: File and folder sharing roles, links and shares view

**Priority:** Mandatory for `filesquid-server`; link sharing from the desktop host requires the hosted service and follows [F-033](#f-033-shared-albums-and-hosted-galleries). **User Story:** As a server user, I want to share a file or folder with people, groups or by link, with a clear role, and see in one place what is shared with me, by me and publicly.

**Delivered by:** FileSquid (`filesquid-server` authorisation and share records; `filesquid-api`; `@filesquid/ui` Sharing panel and Shares view). Album sharing and hosted galleries remain [F-033](#f-033-shared-albums-and-hosted-galleries); shared libraries remain [F-034](#f-034-family-accounts-and-shared-libraries).

### Acceptance Criteria

- A share MUST target a file, a folder (logical-path prefix) or a collection, and name a recipient (user, group or team) or a public link, with one role: **viewer**, **commenter** (viewer plus [F-064](#f-064-file-activity-comments-and-user-activity-stream) comments) or **editor** (new versions, rename, move within the share). Roles MUST be enforced on every API call, deny-by-default ([F-056](#f-056-single-filesquid-api), [F-057](#f-057-desktop-and-server-hosts)). No share role MAY delete a file from the library; deletion ([F-008](#f-008-deletion-hidden-files-and-duplicate-review)) is reserved to the file's owner. Re-sharing by a recipient MUST be a share option the owner controls.
- Public links MUST require an explicit owner action, MAY carry a password and expiry, MUST be revocable, MUST disclose bearer access, and MUST apply the [F-046](#f-046-media-metadata-inspection-correction-and-privacy-exports) outgoing policy to every delivered byte. Downloads through a public link MUST be journaled and visible in [F-064](#f-064-file-activity-comments-and-user-activity-stream). An **internal link** for people who already have access MUST grant no new access.
- The API MUST list, for a file, who has access and through which share, and serve the **Shares** view (with me, by me, by link) under [F-062](#f-062-drive-style-views-sorting-selection-and-layout-preferences).
- **Ownership transfer** of a file or collection to another user on the same server MUST be an offered-and-accepted operation, journaled, with a pending state the search operator `pendingowner:` can find ([F-063](#f-063-file-search-query-language-and-filter-chips)). Transfer MUST NOT move bytes out of their collection's storage location without a verified copy ([F-009](#f-009-multiple-libraries-and-filesystem-administration)).
- A server administrator MUST be able to switch off sharing itself (new shares), public links, re-sharing and download per server; the UI MUST hide the control and say the administrator disabled it ([F-057](#f-057-desktop-and-server-hosts)). Billing administration MUST grant no share or read access ([FileSquid U-06](./product-vision.md#platform-ownership-and-access)).
- Shares on encrypted hosts MUST use recipient keys ([F-029](#f-029-client-encryption-and-key-custody)); FileSquid MUST NOT require the owner to re-share after a key change, and MUST NOT use a server master key ([F-034](#f-034-family-accounts-and-shared-libraries)).

**FileSquid default UI**

- A **Sharing** panel in the sidebar slots MUST show internal shares (people or teams picker, others with access, internal link) and external shares (public link creation with password and expiry). Shared and link badges follow [F-062](#f-062-drive-style-views-sorting-selection-and-layout-preferences).

## F-067: PROV-O history of every version and change

**Priority:** Mandatory, from the first FileSquid build that commits OCFL versions; it MUST NOT depend on the `c2pa` module. **User Story:** As an owner, I want a standards-based record of everything that happened to every file and its metadata, who or what did it and when, kept with the files so it survives copying and recovery without FileSquid.

**Delivered by:** FileSquid (`prov` module, crate `filesquid-prov`; `filesquid-api`; every host's commit paths; `@filesquid/ui` Provenance tab of the file panel). The import record stays [F-002](#f-002-safe-file-folder-and-device-import); C2PA records stay [F-043](#f-043-c2pa-records-and-action-disclosure); the activity journal stays [F-061](#f-061-activity-journal-and-live-health-monitoring).

### Acceptance Criteria

- Every committed operation MUST produce W3C PROV-O records, one `prov:Activity` per committed OCFL object version (an operation that commits several objects, such as an import that commits the file object and the library snapshot, produces one activity per object version, all sharing its operation ID), for library snapshots, files, metadata edits and repairs alike. Covered changes: import (with upload and copy), rename, move, hide, unhide, delete, restore, purge, metadata edit (with version labels), export, scrub, publication, revert (with reinstate), repair, and catalog organisation (collections, virtual collections and memberships) ([FileSquid U-89](./product-vision.md#prov-module), [FE-082](./product-vision.md#prov-o-history)).
- A command that changes nothing (a no-op save) MUST commit no version and produce no record. Failed and refused attempts MUST be recorded in the activity journal only and MUST NOT appear in PROV-O, which describes what happened to files.
- Records MUST use a versioned JSON-LD profile of the W3C PROV-O vocabulary (namespace `http://www.w3.org/ns/prov#`) with an embedded context, so no network fetch is needed to read them. A later profile version MUST NOT stop earlier records from being read.
- Each committed version MUST have exactly one `prov:Activity`, carrying its operation kind, its operation ID (shared by every object version one operation commits), the OCFL object and version it committed, the IDs of its journal entries and `prov:startedAtTime` / `prov:endedAtTime` in RFC 3339 UTC, the end time equal to the version's inventory `created` time.
- Each file and metadata state a version used, generated or invalidated MUST be a `prov:Entity` identified by OCFL object ID, version and logical path and carrying its SHA-256 digest. Generated states MUST be `prov:wasGeneratedBy` the activity and `prov:wasDerivedFrom` the state they replace (a rename or move derives the new path from the old; a metadata edit derives the new metadata record from the previous one); deleted and purged states MUST be `prov:wasInvalidatedBy` it; inputs MUST be `prov:used`.
- Agents MUST be: for the owner, the installation agent: a `prov:Agent` identified `urn:uuid:<installation-uuid>`, where the installation UUID is a UUID v4 ([ADR 021](./architecture/021-uuid-v4-identifiers.md)) that each FileSquid installation generates once on first run, stores with the host's data and keeps the same across restarts; no person identity, account or display name; a `prov:SoftwareAgent` for the FileSquid host and its version (no host name), acting on the installation's behalf; and a tool or model `prov:SoftwareAgent`, reserved for later AI actions. The installation UUID MUST be generated with a cryptographic random source on the first run that finds none, MUST survive restarts and updates, and MUST be restored with a backup of the host's data; when it is lost, a new one is generated and journaled, and earlier records keep the old value. It MUST NOT be derived from hardware, account or host names. They MUST be linked by `prov:wasAssociatedWith`, `prov:actedOnBehalfOf` and `prov:wasAttributedTo`. An agent FileSquid did not observe MUST be stated as unrecorded, never guessed. On `filesquid-server`, each account MUST be its own `prov:Agent`, identified `urn:uuid:<account-uuid>` by a UUID v4 assigned when the account is created and carrying no user name, e-mail address or display name, and MUST be linked to the installation agent by `prov:actedOnBehalfOf`; desktop records name the installation agent as the owner and no `prov:Person`.
- Each activity MUST name, by opaque IDs, the activity-journal entries of its commit and its operation ID ([F-061](#f-061-activity-journal-and-live-health-monitoring)), and journal entries MUST carry the operation ID and the OCFL object and version they concern, so the history and the journal join without timestamps. Every commit MUST emit an `ocfl.commit` journal entry, MUST set the OCFL version `user`, and MUST carry a structured operation message, metadata edits included.
- The record of a version MUST be stored durably in OCFL in the same commit as the change it describes, so a change creates no extra version. An activity that changes no library object but produces new output (an export, a scrubbed export, a ZIP download, or a share or publication output) MUST be committed as a provenance-only version of the library's history object, and a version-inventory repair, which adds no version of its own, MUST be followed by a provenance-only version of the repaired object. A single-file download MUST NOT create a PROV-O record; it MUST be recorded in the activity journal only ([F-061](#f-061-activity-journal-and-live-health-monitoring)). An interrupted commit MUST leave neither a version without its record nor a record without its version.
- Records MUST be exportable with the library and recoverable with ordinary tools: an app-closed copy of the storage root MUST let a JSON parser read the full history without FileSquid, its database or the journal ([F-042](#f-042-ocfl-preservation-and-portable-recovery)).
- Records MUST be complete for the owner's own library, where names and descriptive content (the original file name, titles) MAY appear. Links to journal entries MUST carry opaque IDs only, as F-061 requires. PROV-O exported for sharing or publication MUST be redacted per the [F-046](#f-046-media-metadata-inspection-correction-and-privacy-exports) outgoing policy; an owner archival export MUST carry the records unchanged.
- Objects committed before this history existed MUST receive records from a one-time, bounded, resumable backfill from the existing OCFL inventories (and the journal where available). Backfilled records MUST say that they were reconstructed, carry their own recording time, never claim to have been written at the original commit and invent no agent that was not recorded.
- History MUST be queryable through `filesquid-api` by file, object version, operation, agent and time range, paged and within the caller's read permission; any index of it MUST be rebuildable from the OCFL records. The default UI MUST show a file's history in the Provenance tab of its file panel and let the owner export it as JSON-LD ([F-055](#f-055-default-ui-and-panel-composition), [F-065](#f-065-file-version-history-and-retention)).
- Ephemeral thumbnail or decoded-cache generation and transient computations MUST NOT create permanent history unless their result becomes a retained or disclosed output; promoted results then require lineage ([T-037](./product-vision.md#canonical-ocfl-state-and-evidence)).
- The history engine MUST be usable without the FileSquid app: a stand-alone module crate (`filesquid-prov`) that builds, tests and publishes on its own, takes no default dependency on another module (an optional `ocfl` feature is allowed), never depends on `filesquid-native` or `filesquid-c2pa`, and builds for WebAssembly ([T-058](./product-vision.md#canonical-ocfl-state-and-evidence), [T-052](./product-vision.md#api-and-hosts), [ADR 040](./architecture/040-prov-o-history.md)).

## Technology Stack

The recorded stack is the [ADR index](./architecture/README.md) table: Accepted ADRs 001–016 and 022–028 as amended, plus [ADR 029](./architecture/029-filesquid-unified-product-and-distributions.md) (FileSquid unified product and distributions), [ADR 030](./architecture/030-filesquid-api-and-transports.md) (single API and transports), [ADR 031](./architecture/031-vanilla-typescript-ui-and-white-label-theming.md) (vanilla TypeScript UI and white-label theming; supersedes the Svelte decision in ADR 003 and the three UI packages in ADR 028) [ADR 032](./architecture/032-hosting-and-privacy-model.md) (hosting and privacy model) and [ADR 033](./architecture/033-activity-journal-and-live-health-monitoring.md) (activity journal and live health monitoring) and [ADR 034](./architecture/034-at-protocol-publishing-connection.md) (opt-in AT Protocol publishing connection) and [ADR 040](./architecture/040-prov-o-history.md) (PROV-O history of every version, W3C PROV-O in a versioned JSON-LD profile, independent of C2PA). FileSquid is one Rust (stable, edition 2021) Cargo workspace in `apps/filesquid/` with the module crates `filesquid-core`, `filesquid-ocfl`, `filesquid-metadata`, `filesquid-prov` (created by its first task) and `filesquid-c2pa`, the `filesquid-api` crate that owns the one versioned API contract, and two hosts: `filesquid-desktop` (Tauri 2, API served in-process through a custom URI scheme, no TCP port) and `filesquid-server` (Axum on Tokio, multi-user, serving the API and the selected distribution's UI as static files). Every UI is vanilla TypeScript with standard custom elements and no UI framework, built with Vite; design tokens are CSS custom properties and label tables are JSON ([FileSquid technical stack](./product-vision.md#technical-stack)). The encrypted browser host runs the modules as WebAssembly. SQLite holds local operational catalogs/indexes (accessed through `rusqlite` with a bundled SQLite; SQLx is the planned PostgreSQL access library, [ADR 005](./architecture/005-sqlite-local-postgres-hosted.md) amendment 2026-10-18); PostgreSQL holds multi-user server and hosted-service operational data; S3-compatible object storage holds hosted ciphertext. Default canonical state MUST reside in documented owner-readable files/metadata and OCFL 1.1 versions/snapshots, with separately encrypted sensitive HTTP evidence; operational databases MUST be rebuildable. Hosted private content on `*.filesquid.tech` requires ciphertext object storage, a documented OCFL encryption profile, an operational account/device/transport catalog and versioned APIs. That service MUST NOT decrypt or return ordinary private file meaning, MUST compile hosted-safe code only (envelopes, opaque IDs, job protocol, ciphertext transport) and MUST NOT run private collection search or libvips/FFmpeg on private originals. A host MUST NOT server-render private pages. Stills processing uses libvips in Rust workers and MUST NOT co-install ImageMagick; video, RAW, HEIF and C2PA use dedicated libraries. OCFL digest is SHA-256. Newly generated portable domain identifiers MUST be RFC 4122 / RFC 9562 UUID version 4 ([ADR 021](./architecture/021-uuid-v4-identifiers.md)). Git MUST NOT be the OCFL library. Adopted standards include PROV-O/JSON-LD acquisition and retained-transformation records, C2PA claim/manifest lifecycle and linked watermark publication, OpenAI-compatible BYO connections, OpenAPI 3.1 for the API contract, and later named professional-metadata/IIIF profiles under F-047. Full HTTP recording requires a validated recorder/browser integration; no particular recorder or provider is installed by this specification.

High performance MUST govern implementation of this stack: prototypes MUST measure UI/rendering, catalog/query, worker/scheduler, decoder and cache approaches against the same NFR-002/003 fixtures on the initial macOS desktop profile, then the `filesquid-server` profile. Heavy work MUST stay off interactive rendering paths; acceleration MUST be measured for supported hardware, fidelity and peak decoded memory without implicit remote plaintext processing. Exact crate/library versions are lockfile concerns at implementation; Svelte, React and other UI frameworks, Electron, Phoenix, LiveView, FastAPI and Python, Elixir or other backend languages are not application-stack options. See [architecture technology decisions](./architecture.md#8-technology-decisions-summary) and the [ADR index](./architecture/README.md).

# Non-Functional Requirements

The following proposed budgets make release verification finite. They MUST be confirmed or deliberately revised in the future SRS before implementation promises. No benchmark was run against a FileSquid or ImageSquid application in this assignment.

## NFR-001: Preservation and transactional integrity

**Requirement:** Every verified imported original MUST retain its expected cryptographic hash. Crash/fault fixtures MUST produce zero silent loss, zero false-complete resources and zero unintended duplicate logical imports. All inventoried relationships MUST reconcile or remain explicit unresolved records. F-043 initial records MUST commit with completed imports, and F-042 canonical snapshots MUST durably reference complete verified asset versions and recoverable metadata/keys/evidence before an authoritative change is reported complete. Hosted protection additionally requires its deployed copy/verification policy. A fast UI acknowledgment MUST NOT masquerade as canonical durability or bypass integrity/provenance.

## NFR-002: Benchmark envelope and responsiveness

**Requirement:** High performance MUST be a defining free requirement from the first FileSquid release, on every host and distribution, maintained as workflows expand. Proposed test profiles are Small: 10,000 assets/20,000 resources/100 GB/100 albums; Medium: 100,000/200,000/1 TB/5,000 albums; Large: 1,000,000/2,000,000/10 TB/50,000 albums/10,000,000 memberships. Include duplicate bytes, pairs, Unicode, dense memberships, late sidecars, high decoded-pixel counts and varied supported formats. Support tests MUST include a 100 GB single resource and 250 GB archive part through streaming rather than memory-sized allocation; compressed bytes alone MUST NOT establish bounded decoding cost.

On a declared 4-core/16 GB/SSD desktop reference machine, warm viewport interaction-to-visible-feedback p95 MUST be ≤100 ms, indexed metadata search p95 ≤2 s, and first useful local viewport ≤3 s for the large catalog. Measure at least 100 queries/interactions after declared warm-up and report cold behavior separately. Mobile/browser tests MUST use a separately declared 4 GB device/network profile: warm cached control feedback p95 MUST be ≤100 ms, while completed common browse/search work retains its p95 ≤2 s budget excluding unavailable-original downloads. Excluded transfer waits MUST still appear in end-to-end reports.

For each supported desktop/browser/mobile profile, resident prepared thumbnail/viewer images and prepared next slides MUST paint the correct image at p95 ≤100 ms from the request or scheduled presentation time. A placeholder, stale image or input spinner MUST NOT pass. Preparation coverage across the exercised collection MUST be reported so a tiny prewarmed subset cannot establish performance. Input acknowledgment, first useful preview and final requested-quality readiness MUST have independent measurements and acceptance results.

Declared 60 Hz scrolling/presentation MUST use a proposed 16.7 ms frame budget. Before release acceptance, each profile MUST define permitted missed-frame rate, slideshow transition lateness, sufficiently large p95/p99 sample sizes, clock boundaries, warm-up/cache contents and error limits; other refresh rates require their own explicit budget. Reports MUST show stalls and p95/p99 timing rather than averages alone.

Before an affected feature ships, its nonempty format/device/local-or-remote matrix MUST define useful-preview and full-quality deadlines for cold, warm, evicted and missing-derivative cases, including supported large/high-resolution media. The first useful viewport, warm feedback and full-quality gates MUST pass independently; a fast spinner MUST NOT compensate for missed image-readiness targets. Tests MUST exercise idle operation and concurrent import/migration, thumbnail generation, indexing, provenance scans and batch metadata work, including navigation reversals/jumps, cache loss and resource pressure. Actual end-to-end wait, transfer/decode components, failures and unsupported coverage MUST remain visible; demanding cases MUST NOT disappear from evidence by labeling all relevant formats unsupported.

## NFR-003: Bounded processing and migration endurance

**Requirement:** Proposed desktop default staging cap is 10 GiB, with reserve max(2 GiB, 5% destination capacity) and preflight selection of a larger cap or segmented path when a resource cannot fit. Streamed large resources MUST not require a full extra library copy. Default active transfers ≤4 and CPU-heavy workers ≤2; all queues MUST have explicit finite limits/backpressure. Allow at most 5 total attempts per resource, including the initial attempt, before quarantine, using exponential retry backoff capped at 60 s plus jitter; manual retry starts a new recorded recovery cycle subject to the same cap.

Proposed migration process memory budget is ≤2 GiB excluding separately budgeted ML models. Restart MUST reach a resumable job state within 30 s on the reference desktop without rescanning completed media; measure repeated bytes per route and allow documented affected-resource/decompression replay. A 72-hour mixed/fault soak MUST show no unbounded growth; post-warm-up non-cache memory growth MUST be ≤10%. Throughput MUST be reported as verified bytes/time with hardware/network, small-file mix and sidecar reconstruction time; do not promise WAN speed independent of the provider.

Heavy decode, transform, index, scan, verification and transfer work MUST run outside the interactive rendering path through bounded CPU/GPU/I/O/transfer queues. Current controls/visible media MUST have priority, equivalent work MUST be deduplicated and speculative work MUST reduce under memory, disk, network, battery or thermal pressure. Existing default concurrency ceilings above remain in force until a reviewed profile changes them. Each scheduler profile MUST define a measurable background-service guarantee or a visible pause policy before implementation so interaction cannot silently starve preservation indefinitely; progress, pause cause and resumption MUST be observable.

Each device/browser profile MUST define finite memory/disk cache budgets and prefetch item/byte/time limits before implementation. Thumbnail/display/decoded-media and view/query caches MUST key identity, content/recipe/metadata revision, view/sort/filter and permission scope, with deterministic invalidation, pinning and eviction. Stale work MUST be canceled or its results rejected. Application eviction MUST NOT delete authoritative originals/history or pinned offline media; browser-imposed eviction/quota loss MUST expose availability and recovery limits. Peak decoded memory, cache occupancy, pending queues, hit/miss/eviction/redecode rates, speculative bytes and background progress MUST be measured with explicit bitmap/decoder/resource release.

Durable completed-item checkpoints MUST extend to supported import/upload, thumbnail/index, provenance-scan and metadata batches. Restart MUST revalidate source/version/authorization, reuse verified completed work and resume within-resource only where supported. Necessary affected-unit replay MUST be explicit and measured; faults MUST NOT cause entire-batch restart, duplicate provenance/events or false-complete media. Cache loss may require rebuilding missing cache work without pretending completed authoritative work was lost. Finite retry/backpressure policy and safe quarantine/recovery remain mandatory.

## NFR-004: Security and privacy

**Requirement:** Cross-account/private/shared authorization tests MUST show zero unauthorized reads/writes in the suite. Cryptographic design and key recovery MUST undergo independent review before hosted protection is offered, with no unresolved critical/high findings accepted for that release. Private media, keys, tokens, plaintext hashes and sensitive metadata MUST be absent from operational logs/analytics fixtures. Malformed media/archives, path traversal, decompression bombs, symlinks, oversized metadata and hostile extensions MUST be safely rejected or isolated.

Derivatives, cached views/searches and speculative prefetch MUST enforce the same authorization/disclosure boundaries as requested content. Tests MUST cover edits, hidden/access changes, lock/logout, membership revocation, sort/filter changes and browser storage loss; stale authorized results MUST NOT remain exposed after access changes. Cache keys/logs MUST NOT leak private identifiers or HTTP evidence. Hardware/worker acceleration MUST preserve supported rendering fidelity and MUST NOT authorize unselected remote plaintext processing.

## NFR-005: Recovery and service reliability

**Requirement:** Local recovery MUST be tested on clean installs and missing-drive/corruption scenarios. Proposed hosted pilot targets: monthly control-plane availability ≥99.5%; disaster RPO ≤24 h for acknowledged catalog/recovery manifests; control-plane RTO ≤24 h. These are internal pilot budgets, not an SLA. Restore transfer completion MUST be estimated from bytes and measured throughput, not conflated with control-plane RTO. An original MUST meet the selected durability/replication policy together with its recoverable catalog/manifest and wrapped-key associations before it is labeled protected. The proposed 24-hour control-plane RPO MUST NOT authorize loss of recovery associations for media already labeled protected; uncommitted coverage remains pending.

## NFR-006: Accessibility and usability

**Requirement:** Web interfaces MUST target WCAG 2.2 AA; desktop MUST support keyboard-only tasks, native screen readers, visible focus, scalable text and high-contrast/reduced-motion preferences. Drag-only workflows MUST have alternatives. Undo and Redo for the organization commands of [F-004](#f-004-collections-folders-and-navigation) MUST be reachable from the keyboard and the Edit menu on every host. Release testing MUST include import, search, edit, permission review, recovery and export on each supported platform; built UI MUST mount with rigorous DOM/state assertions and no SSR/hydration/browser errors.

## NFR-007: Compatibility, rendering and localization

**Requirement:** Publish the tested OS/browser/codec/model matrix per release. Preserve source color profiles and orientation; proposed normalized SDR golden-image tolerance is mean ΔE2000 ≤2 and p95 ≤5 against an approved same-recipe reference, with separately reviewed perceptual/HDR/video tests. Engine changes MUST preserve old approved renders and require explicit reprocessing. UI strings MUST be externalized, support Unicode/locale sorting, localized dates/numbers and right-to-left layout testing; the supported language list MUST be declared before release.

## NFR-008: Maintainability and verified engineering

**Requirement:** Application workspaces MUST be standalone/exportable, dependencies minimal and verified, interfaces documented, and all functions/classes/modules/complex blocks documented. Functions MUST be ≤60 lines; loops/retries/queues MUST be bounded; resource cleanup and error paths MUST be explicit. Lint/format precede appropriate unit, integration, BDD, built-production UI and load tests, with structured reports and raw task walkthrough evidence. Tests MUST track changed requirements and isolate/clean artifacts within each workspace's tests directory.

## NFR-009: Observability and capacity economics

**Requirement:** Processes MUST emit structured, unbuffered, redacted stdout events with UTC RFC 3339 time, severity, opaque job/request correlation and outcome. Central service operations MUST support buffered log ingestion, metrics, traces, dashboards, access control and retention. Local-only operation MUST not require cloud telemetry or a running observability cluster. Full source HTTP exchanges, including sensitive request/response bodies, MUST be stored as encrypted F-041 preservation artifacts, separate from these redacted operational events. Background work SHOULD stop when idle; storage/replication and essential service infrastructure costs MUST be modeled separately from scalable compute.

## NFR-010: Accurate public claims

**Requirement:** Public claims MUST describe the tested migration scope, encryption trust boundaries, hosting provider/geography and cancellation policy accurately. Claims MUST distinguish hypotheses from measured results and report denominators for measured results.

# Technical Constraints

## TC-001: Immutable originals and explicit identity

**Constraint:** Originals MUST be immutable; logical asset IDs, source IDs, storage hashes, versions, relationships and resource roles MUST remain distinct. Newly generated logical asset IDs and other portable domain identifiers (repository, library, collection, virtual collection, virtual exhibit, file, file version, file resource, folder) MUST be RFC 4122 / RFC 9562 UUID version 4 in canonical lowercase 8-4-4-4-12 form ([ADR 021](./architecture/021-uuid-v4-identifiers.md)). Documented F-001/042 local media, sidecars and OCFL versions/snapshots MUST be authoritative for committed managed-local state; operational catalogs/indexes MUST be rebuildable. Completed mutations MUST have durable canonical representation and startup reconciliation. Optional format conversions MUST add lineage-linked renditions without replacing originals; encrypted sensitive evidence and referenced/remote availability remain explicit exceptions. Performance optimizations MUST NOT weaken these rules.

## TC-002: Endpoint processing and hosted encryption

**Constraint:** Hosted private media MUST follow F-029/F-042. User-selected F-044 publication and F-045 BYO inference are adopted scoped disclosures; the source plan's unrestricted operator-side derivative/AI suggestions remain excluded from the default architecture. Browser-only operation MUST still work for accessible supported content; desktop assistance MUST not become a hidden prerequisite for basic browser use.

## TC-003: Platform and provider limits

**Constraint:** PhotoKit permissions/library coverage, Google export/API restrictions, Microsoft placeholders/range semantics, browser lifetime/filesystem constraints, media licensing and capture dependencies MUST be verified per supported version. Exact Apple proprietary history/rendering and undocumented APIs MUST not be assumed.

## TC-004: Decision status and repository alignment

**Constraint:** Technical stack candidates MUST become reviewed ADRs before implementation. Stack ADRs [001–016](./architecture/README.md) and [022](./architecture/022-dublin-core-descriptive-metadata.md)–[028](./architecture/028-filesquid-ui-packages-and-panel-composition.md), as amended or superseded by [ADR 029](./architecture/029-filesquid-unified-product-and-distributions.md)–[ADR 032](./architecture/032-hosting-and-privacy-model.md), are the implementation direction, including the `filesquid-ocfl` module ([ADR 016](./architecture/016-rust-ocfl-library.md), [ADR 023](./architecture/023-filesquid-ocfl-engine-and-parquet.md)). This repository is the FileSquid repository; ImageSquid is the distribution in `apps/imagesquid/`. It currently supplies scaffolded crates, hosts and UI shells, not the product features specified here. Named cloud region, payment, recorder implementation, job SQL, SPA-asset signing and independent crypto audit remain later work ([T-034](./product-vision.md#later-gates)). The hypothetical architecture MUST be labeled accordingly. Accepted stack direction is not evidence that NFR-002/003 or [FileSquid U-11](./product-vision.md#platform-ownership-and-access) have been measured. Rules specific to another product/stack MUST not silently override FileSquid's local-first requirements. Git MUST NOT be the OCFL library. SHA-256 is the OCFL digest ([ADR 009](./architecture/009-cryptography-webcrypto-compatible-suite.md)).

## TC-005: Shared Rust graph, hosted mailbox isolation and OCFL writer

**Constraint:** FileSquid MUST use Rust for backend logic and TypeScript (vanilla, standard custom elements, no UI framework) for every distribution's UI. MUST NOT add Python, Elixir, Phoenix, FastAPI, Django or another application backend ([FileSquid U-29](./product-vision.md#one-api-hosts-and-distributions)). A distribution MUST NOT add a backend of its own ([C-026](./product-vision.md#release-sequence)). MUST NOT add Celery, Oban or another second job stack beside durable Rust jobs ([FileSquid U-35](./product-vision.md#one-api-hosts-and-distributions)). MUST NOT require a nightly toolchain ([FileSquid U-37](./product-vision.md#one-api-hosts-and-distributions)). MUST NOT import a generic server or AI stack solely because it appears in the repository template ([T-033](./product-vision.md#later-gates)).

FileSquid MUST be one Cargo workspace of module crates. A module crate MUST NOT take a default dependency on another module crate; optional ones are allowed, namely the `metadata` module's optional `ocfl` store and the `prov` module's optional `ocfl` feature ([ADR 040](./architecture/040-prov-o-history.md)), and each MUST build and test on its own ([T-052](./product-vision.md#api-and-hosts)). Hosts link `filesquid-api`, which links the modules. The FileSquid modules (`filesquid-ocfl`, `filesquid-metadata`, `filesquid-prov`, `filesquid-c2pa`) and private application logic MAY run natively in `filesquid-desktop` and in `filesquid-server`, and as WebAssembly in the encrypted browser host; native media processing runs in out-of-process workers of the desktop host and of `filesquid-server` ([C-053](./product-vision.md#module-rules), [FileSquid U-26](./product-vision.md#one-api-hosts-and-distributions), [ADR 032](./architecture/032-hosting-and-privacy-model.md)). Portable contracts (envelopes, opaque IDs, job protocol, ciphertext transport) MAY link into every host and into the hosted service. The ciphertext-only `*.filesquid.tech` service MUST NOT link private application logic, module processing of private content, or native media processing ([FileSquid U-20](./product-vision.md#one-api-hosts-and-distributions), [T-001](./product-vision.md#client-processing-and-hosted-mailbox), [T-054](./product-vision.md#api-and-hosts)). CI MUST fail that service's build if libvips, FFmpeg, `filesquid-ocfl` (any backend), `filesquid-metadata`, `filesquid-prov`, `filesquid-c2pa` or other module code that processes private plaintext appears in its dependency graph ([FileSquid U-30](./product-vision.md#one-api-hosts-and-distributions)). Cargo features MUST NOT be the only privacy boundary ([FileSquid U-31](./product-vision.md#one-api-hosts-and-distributions)). Hosted Tokio runtimes MUST NOT run blocking native media work ([FileSquid U-32](./product-vision.md#one-api-hosts-and-distributions)). A `filesquid-server` operator can read the files on that server; this deployment type MUST be stated in its UI and documentation and MUST NOT weaken the ciphertext-only promise of `*.filesquid.tech` ([FileSquid U-20](./product-vision.md#one-api-hosts-and-distributions)).

Versioned operation schemas and errors MUST be owned by `filesquid-api` and published as a contract, with TypeScript types generated from it and compatibility tests so hosts and UIs cannot drift ([FileSquid U-16](./product-vision.md#one-api-hosts-and-distributions), [T-048](./product-vision.md#api-and-hosts)). Native Rust, WASM and WebCrypto MUST share matching test vectors for SHA-256, AES-256-GCM, HKDF, Ed25519, X25519 and Argon2id; drift is a defect ([FileSquid U-22](./product-vision.md#one-api-hosts-and-distributions), [FE-021](./product-vision.md#managed-protection-and-connectivity)). Release and CI MUST run `cargo audit` / `cargo deny` for crates.io dependencies and bundled C libraries ([FileSquid U-38](./product-vision.md#one-api-hosts-and-distributions)). Private-library processing MUST default to authorized endpoints: the desktop host, an owner- or organisation-operated `filesquid-server`, or the encrypted browser host ([T-001](./product-vision.md#client-processing-and-hosted-mailbox)). The browser host MUST read and commit the hosted OCFL root through the WASM build of `filesquid-ocfl` ([T-002](./product-vision.md#client-processing-and-hosted-mailbox)). The owner MAY authorize selected inputs to their own AI provider or the paid C2PA publisher ([T-003](./product-vision.md#client-processing-and-hosted-mailbox)). FileSquid hosts MUST compile `filesquid-ocfl` into their graph and MUST NOT shell out to an OCFL CLI for canonical writes ([T-043](./product-vision.md#canonical-ocfl-state-and-evidence)).

## Out of Scope (For Now)

Continuous bidirectional synchronization with source providers; automatic source deletion or subscription cancellation; guaranteed repair of irrecoverably corrupt originals; exact Apple AI/rendering or inaccessible adjustment history; Apple Music/print-service rights without agreements; a professional layers/masks compositor, panorama stitching, HDR exposure merging or multitrack video editor; legacy iPhoto Events and flags, and star ratings in any form, including the rating filter of the Photos 12 guide (favorites and keywords are the classification model; see [F-040](#f-040-compatibility-and-parity-register)); a guaranteed background mobile-web camera backup service; a working viewer, editor or metadata mapper for every file format ([C-027](./product-vision.md#release-sequence)). Native mobile capture, direct provider conveniences, collaborative migration review/Automerge, a self-hosted copy of the operated `*.filesquid.tech` service and F-035 extensions remain optional decisions. A self-run multi-user `filesquid-server` is in scope ([F-057](#f-057-desktop-and-server-hosts)). Ordinary reversible image adjustments and retained edit versions remain in scope for the ImageSquid distribution. F-041–046 source capture, OCFL preservation, C2PA records (with the `c2pa` module), paid watermark publishing, BYO AI and metadata privacy, and the F-067 PROV-O history, are adopted core requirements; they are not deferred F-035 optional extensions.

F-047 institutional DAM and selected IIIF publishing/hosting, and F-060 archival curation jobs, are committed later scope, deferred from the FileSquid and ImageSquid consumer releases. The adopted [future IIIF milestones](#future-iiif-milestones) retain task-fidelity, partnership and funded-maintenance gates for implementation readiness and profile selection. They MUST NOT be reopened as an optional-scope decision or made a consumer-release/Photos-parity dependency; RC-014 separately governs institutional completion.

## Success Criteria

A release succeeds only when its assigned requirements pass the relevant release gates, with no silent original loss, all migration inventory accounted for, honest capability states, independent canonical-file/OCFL recovery and high-performance evidence for its workflows. The FileSquid release succeeds when the default distribution, the single API, both hosts and the `ocfl` and `metadata` modules pass their gates, with any file type preserved byte-exact and free local access surviving hosted-service cancellation. The encrypted backup and sync release succeeds when F-059 passes with rehearsed restores and ciphertext-only evidence. Mature ImageSquid scope succeeds when every Photos baseline item is mapped and any remaining limits are explicit, migration routes work inside declared contracts, and free local access survives hosted-service cancellation. Both consumer releases also require F-041–046 and F-067 evidence for the routes they ship: complete supported source recording, recoverable OCFL packages, PROV-O history of every version and retained change (from the MVP on), every-import C2PA records (from the ImageSquid release, which ships the `c2pa` module), free collection-wide scans/reports, working in-app paid watermark publication, user-funded task/model choice, Dublin Core `/terms/` descriptive metadata and configurable EXIF/video scrubbing with verified private/public boundaries. Institutional completion additionally requires F-047, F-060 and RC-014 without blocking consumer completion.

# Release Criteria

## Release order and dependency gates

This section records the required release order and the dependency gates each release MUST pass. It sets no dates, durations or staffing. Execution phasing and task order live in the [implementation plan](./implementation-plan.md) ([decoupled execution](../.agents/rules/decoupled-execution.md)). The stable F/NFR/TC/RC IDs remain independent of scheduling. All applicable safety, accessibility and recovery gates apply at every release.

The release order is: the **FileSquid MVP** (macOS desktop early access, [C-056](./product-vision.md#release-sequence)), then **FileSquid** (first consumer release), then **encrypted backup and sync** at `*.filesquid.tech`, then **ImageSquid** (second consumer release), then **Arkrim** and the other later milestones in an owner-chosen order ([C-025](./product-vision.md#release-sequence), [C-052](./product-vision.md#release-sequence), [F-050](#f-050-filesquid-platform-and-distributions)).

| Release or gate | Prerequisites | Scope and objective | Exit gate |
|---|---|---|---|
| Foundation gate (FileSquid; not a release) | Review source conflicts; approve initial SRS/ADRs, including [ADR 029](./architecture/029-filesquid-unified-product-and-distributions.md)–[ADR 032](./architecture/032-hosting-and-privacy-model.md), the API transport evaluation and the capability test matrix | On the macOS desktop profile (the first desktop target, [ADR 035](./architecture/035-macos-first-desktop-mvp.md)), validate canonical local file/OCFL and rebuildable catalog/resource models, transactional durability, the in-process desktop API transport (the API stays transport-agnostic; the HTTP plus event channel binding is validated on `filesquid-server` after the MVP), webview behavior, immutable recipes, encryption/key design and future sync identity; define F-042 readable-local/encrypted-evidence/hosted boundaries, F-067 PROV-O history profile, F-043 C2PA record policy, F-041 recorder feasibility and F-044 in-app publisher contract; benchmark candidate render/query/worker/decoder/cache approaches against NFR-002/003; start RAW/HDR/depth/Cinematic research early because ImageSquid depends on it | RC-001/002/003 feasibility and fault prototypes, declared performance/readiness/resource profiles; no product readiness claim |
| FileSquid MVP (early access; not the consumer release) | Foundation gate | The default distribution on the macOS desktop host only ([C-056](./product-vision.md#release-sequence)): [F-001](#f-001-independent-local-library-and-ownership) local library create/open/copy/recover; [F-002](#f-002-safe-file-folder-and-device-import) file and folder import with durable jobs and an import record for every import; [F-042](#f-042-ocfl-preservation-and-portable-recovery) local OCFL create/version/copy/recover; [F-051](#f-051-ocfl-module) read, validate, write and change model; [F-052](#f-052-metadata-module) extraction, `metadata.md` through the `ocfl` store and registered namespaces; [F-055](#f-055-default-ui-and-panel-composition) Drive-style browser with the three-tab file panel (File info, Description, Provenance; Versions opens from a header button) and the Logs view; [F-056](#f-056-single-filesquid-api); [F-057](#f-057-desktop-and-server-hosts) desktop host only; [F-061](#f-061-activity-journal-and-live-health-monitoring); [F-062](#f-062-drive-style-views-sorting-selection-and-layout-preferences); [F-063](#f-063-file-search-query-language-and-filter-chips); [F-065](#f-065-file-version-history-and-retention); [F-067](#f-067-prov-o-history-of-every-version-and-change) PROV-O history of every version and change with the file panel's Provenance tab, the Logs view and journal export; original file names on every surface and for owner downloads (F-002). Out of scope: `filesquid-server`, Linux and Windows installers, accounts, groups and sharing ([F-066](#f-066-file-and-folder-sharing-roles-links-and-shares-view)), the activity stream and comments ([F-064](#f-064-file-activity-comments-and-user-activity-stream)), virtual exhibits ([F-053](#f-053-virtual-exhibit)), the distribution manifest, all C2PA ([F-043](#f-043-c2pa-records-and-action-disclosure), [F-054](#f-054-c2pa-module) and the Content Credentials panel; they ship with the ImageSquid release), JSON sidecars, published module crates, installer signing and the updater. Media, migration, AI and curation modules MUST NOT be prerequisites | Useful offline import → browse → search → restore a version → recover the library from OCFL alone → export on macOS desktop, with [RC-001](#rc-001-original-integrity-and-recovery), [RC-002](#rc-002-performance-and-resource-bounds) (macOS desktop profile), [RC-005](#rc-005-functional-and-accessible-ui-quality), [RC-010](#rc-010-complete-ocfl-preservation-and-independent-recovery) and [RC-013](#rc-013-metadata-fidelity-and-configurable-sharing-privacy); the [ADR 011](./architecture/011-native-media-adapters-and-ffmpeg-license.md) licence and patent review of the DMG's linked inventory has passed; an unsigned installer is labelled early access and its install guide states how to open it; no paid-service claim |
| FileSquid release (first consumer release) | FileSquid MVP | The default distribution on the desktop host (macOS first, then Linux and Windows) and the multi-user `filesquid-server`: [F-050](#f-050-filesquid-platform-and-distributions), [F-051](#f-051-ocfl-module), [F-052](#f-052-metadata-module), [F-055](#f-055-default-ui-and-panel-composition)–[F-058](#f-058-white-label-distributions) without the Content Credentials panel, F-049; F-001; F-002 files/folders/cards/selected uploads and watched folders; F-004 collections and F-006 virtual collections and metadata search in internal names; F-005, F-008, F-009, F-010 original export, F-037, F-038, F-039; the F-040 capability register; F-042 canonical local OCFL create/version/copy/export/recover; F-046 extraction/inspection, single/batch `dcterms:` / `schema:` sidecars, baseline search and the mandatory selected metadata policy for any sharing route; F-053; F-067 PROV-O history on both hosts; server users, groups, group-owned libraries and collection permissions. C2PA ([F-043](#f-043-c2pa-records-and-action-disclosure), [F-054](#f-054-c2pa-module)) is out of scope and ships with the ImageSquid release. Media, migration, AI and curation modules MUST NOT be prerequisites | Useful offline import → organize → recover → export on both hosts with applicable RC-001/002/003/005/010/013; independent feedback/useful-preview/canonical-recovery gates; any file type preserved byte-exact with disclosed limits; synthetic/new files acceptable; no whole-library migration claim |
| Encrypted backup and sync (FileSquid) | FileSquid release; reviewed crypto/key recovery, version/conflict model, independently verified backup; F-059 launch gates (crypto-protocol audit, provider and region, payment processor, GDPR role analysis) | [F-059](#f-059-hosted-encrypted-backup-and-sync-service) at `*.filesquid.tech`; F-029–034 and F-042 managed preservation, family/non-family groups, encrypted sharing and F-032 service controls. Encrypted backup MUST precede paid sync reliability claims | RC-003/006/007/010 and C-2: multi-device conflict/deletion/restore, revoked-recipient and concurrent-transfer responsiveness gates; ciphertext-only dependency-graph check |
| Encrypted browser host | Encrypted backup and sync; authorized browser decryption | F-036 encrypted browser host and installable mobile web; F-057 WASM modules against the encrypted hosted root ([T-002](./product-vision.md#client-processing-and-hosted-mailbox)) | RC-003 browser-with-desktop-off gate and NFR-002–004 browser profiles. It is not a prerequisite of the ImageSquid release unless the owner decides otherwise |
| ImageSquid release (second consumer release) | FileSquid release and encrypted backup and sync | The ImageSquid distribution ([F-058](#f-058-white-label-distributions)) with photo views and the FileSquid media, editing, people, migration, AI and publication modules it enables. FileSquid Content Credentials ship with it on every host that processes private content: [F-054](#f-054-c2pa-module) read, validate, mint, sign, edit, embed and scan; [F-043](#f-043-c2pa-records-and-action-disclosure) import-time records, PROV-O/disclosure coverage for shipped actions, free collection scans/reports and backfilled records for files imported earlier; the Content Credentials panel ([F-055](#f-055-default-ui-and-panel-composition)), with [RC-009](#rc-009-source-evidence-and-c2pa-lifecycle) as its gate. Its internal gates follow ImageSquid C-040 in this order: **(a) ownership and dependable organization:** F-003 progressive grid/viewer/basic-video/selection, F-004 albums and galleries, F-005, F-006 saved-query predicates (local metadata, `dcterms:` / `schema:`, camera, album, status), F-007 non-semantic media/activity views, F-016 burst review, migration package M-A; **(b) editing:** F-011–013 everyday photo editing/RAW and corresponding mobile basic edits; **(c) discovery:** F-019–021, F-045 compatible recognition/discovery task routes and intelligent F-006/007/039, including People/Pets/Trips/Months/Years and semantic collections; **(d) connectivity:** shared photo surfaces of F-033/034/036/039; **(e) storytelling:** F-022/023 storytelling, prepared slideshows, print/PDF and authorized project adapters; **(f) advanced media:** F-014–018 and remaining advanced F-036; **(g) parity audit:** complete F-024/028/037/040 integration, F-002 native camera/phone adapters, migration refinements, mature macOS/Windows/Linux/browser support, locale/accessibility and performance audit. Recognition-dependent predicates/curation and synchronized definitions wait for gates (c) and (d) | (a) large-library concurrent-work benchmark, RC-009 scan and RC-010/013 metadata restoration/privacy; (b) render/revert/golden-image and original-hash checks; (c) labeled search/recognition corpus with approved relevance/error thresholds, correction and permission/private-content exclusion tests; (d) RC-006 for shared photo surfaces; (e) reopen/edit/export presentations, correct prepared-slide/timing and print-layout verification, music/provider rights; (f) per-format preservation/edit/export contracts with capability-dependent gaps registered; (g) RC-001–013 and full item-level photo-manager parity/adopted-scope review; unresolved proprietary gaps remain disclosed |
| Arkrim and later milestones | ImageSquid release; owner-chosen order ([C-052](./product-vision.md#release-sequence)) | [F-060](#f-060-archival-curation-jobs-and-datasets) curation jobs, Parquet and FAIR reporting with the Arkrim dashboard; [F-047](#f-047-institutional-dam-and-iiif-interoperability) IIIF/GLAM; ProofMode integration; [F-048](#f-048-later-hosted-private-oss-inference) after verified paid encrypted preservation | RC-014 for institutional completion; curation jobs pass RC-001/010 versioning and recovery checks |

Gate scope applies to the named acceptance subsets, not every criterion in a referenced F block. Remaining F-003 motion/HDR/face overlays become available with the relevant F-016/F-017/F-019 capabilities; no early gate can claim those outcomes from basic viewer completion alone. Every mandatory ImageSquid criterion MUST be closed by its parity-audit gate; F-047 and F-060 close at their separate institutional gate. Applicable NFR-002/003 and RC-002/005 performance checks MUST pass from the FileSquid release and for every added workflow, with feedback, useful-preview, full-quality, frame/resource and recovery results assessed independently. macOS-first sequencing ([ADR 035](./architecture/035-macos-first-desktop-mvp.md)) does not remove mature Linux/Windows requirements, and PhotoKit remains a macOS-bound adapter rather than a portable route.

Dependencies permit parallel work: ImageSquid discovery and editing modules can progress independently after shared foundations; protection prototypes need not wait for AI tools; local presentations do not depend on hosted sharing. The Foundation gate MUST model advanced resources and shared identities even when their interfaces ship later.

## Preservation, provenance and AI delivery track

These adopted requirements are FileSquid modules integrated with the release order without waiting for every Photos feature. From the release that ships the `c2pa` module (the ImageSquid release, [C-025](./product-vision.md#release-sequence)), each new import/action route MUST satisfy F-043 when that route ships; a later release cannot defer its initial record. Files imported by earlier releases receive backfilled records under F-043.

| Capability | Prerequisites and release | Required gate |
|---|---|---|
| F-067 PROV-O history | Foundation identity and canonical durability; required for every OCFL version and retained change from the FileSquid MVP onward, with backfilled records for versions committed before it, and expanded with every later route | RC-010; same-commit records, journal links, clean recovery and honest backfill |
| F-043 import/action C2PA records | Foundation identity, canonical durability, the F-067 history and a versioned disclosure policy; required for every import from the ImageSquid release, with backfilled records for files imported earlier, and expanded with every retained metadata/editor/export/AI route; free collection scans/reports in the ImageSquid release | RC-009/010; free/offline use, linked prior credentials, scan resumption and pending-import recovery |
| F-041 source clipping and full HTTP evidence | Acquisition/metadata contracts from the FileSquid release, a protected local evidence store and a recorder that passes complete controlled exchanges; a FileSquid module that does not wait for ImageSquid | RC-009; PROV-O sidecars and F-042 portable package/export support for the captured evidence |
| F-042 local preservation and export | The Foundation gate settles canonical file layout/transaction contract and evidence-key recovery; the FileSquid release MUST deliver local OCFL create/version/copy/export/recover for its supported imports and metadata, independent of hosted services; recovery coverage expands with every later authoritative feature | RC-010 valid readable-local OCFL, app-closed filesystem copy and clean reconstruction without operational database; applicable encrypted-evidence checks |
| F-042 hosted preservation | Reviewed encryption, tenant access, serialized commits, independently protected snapshots/copies and C-2 restore evidence; delivered with the encrypted backup and sync release ([F-059](#f-059-hosted-encrypted-backup-and-sync-service)) | RC-003/006/010 and C-2; measure retained HTTP/history/replica costs before publishing quotas |
| F-044 paid third-party watermark publication | F-043 history, output derivatives, protected provider connection and entitlement/job billing; provider/format feasibility prototyped at the Foundation gate; the declared desktop/browser in-app workflow ships after RC-011/C-7 and after ownership, recovery and paid encrypted preservation ([C-024](./product-vision.md#ai-and-article-50)), without waiting for all editing or parity | RC-011 and C-7; real watermark-linked signed results, complete in-app flow, useful recipient retrieval and portable issued artifacts |
| F-046 metadata privacy and curation | Extraction/inspection, single/batch `dcterms:` / `schema:` authoring, progressive disclosure, baseline search and versioned corrections/sidecars in the FileSquid release; every share/publication route requires verified enabled/disabled and retained-field policy support before launch, including C-7 | RC-013, with F-043 history and F-042 recovery; no paid prerequisite |
| F-045 BYO task/model choice | Protected user-owned connections and validated task contracts; initial compatible organization tasks MAY precede the ImageSquid discovery gate, with dedicated recognition/embedding contracts added as their features arrive | RC-012 for each supported task; F-043 for disclosure-relevant actions and F-042 for accepted corrections |

Validate capture-to-export-to-restore with a compound photo, metadata-only changes and a source-clipped video before extending protection claims. Additional destinations, extended retention or institutional offerings follow measured recovery; they do not create a second basic preservation subscription.

## Migration delivery track — proposed placement

The [Migration Playbook](./references/filesquid-migration-playbook.md), §§1, 12, explicitly leaves migration scheduling unassigned. The following is this draft's recommendation, not an inherited commitment. The migration engine is a FileSquid module; the photo-platform routes are presented by the ImageSquid distribution.

| Package | Earliest recommended gate | Included routes and dependencies | Acceptance |
|---|---|---|---|
| M-A: file/export migration | After FileSquid release integrity and metadata/relationship contracts; within ImageSquid gate (a) | SRC-01/03/04/07/08; F-025–028, stable manifests/resources/albums/provenance, safe placeholder hydration and independent destination | RC-004 on authorized Apple exports, multipart Takeout and Windows/OneDrive fixtures; source unchanged |
| M-B: native Apple | After the M-A engine; independent of ImageSquid editing completion | SRC-02 PhotoKit permissions, macOS/version matrix, bounded iCloud extraction, rich resource association | Item/resource checkpoint and live-source rescan tests; no promise of all libraries/metadata |
| M-C: optional cloud conveniences | After validated route need and the encrypted backup and sync release; only if selected | Optional SRC-05/09; SRC-06 eligibility investigation. Client-side transfer/encryption, OAuth and version-safe ranges | Selected-item limitations, expired-link/source-change tests, no hosted plaintext staging |
| M-D: final migration coverage audit | ImageSquid gate (g) | All committed routes, preservation gaps, repeated-source/cross-route identity and reports | Every MIG/MS requirement and declared supported source version has evidence |

Bulk migration launch MUST record the Playbook §12 worksheet: route/version scope, destination/encryption, metadata mappings, paired formats, benchmarks, review scope, Automerge decision, staffing/dependencies and release placement. Migration requires media preservation before advanced editing, not feature parity in every source format. A migration utility MAY use the same engine without depending on the entire GUI product.

## Future IIIF milestones

| Milestone | Scope and dependencies | Decision and delivery gate |
|---|---|---|
| IIIF-1 — Near-term focus | Deliver the existing FileSquid and ImageSquid consumer, migration, preservation, recovery, metadata-privacy and C2PA requirements. Defer IIIF publishing/hosting; preserve F-038's replaceable source/extension contracts. | Release scope MUST NOT introduce IIIF servers, hosting commitments or IIIF support as a prerequisite for the adopted core. |
| IIIF-2 — Later archive import | Validate, then deliver F-047's bounded import/viewing adapter for explicitly selected archive items from named, openly accessible collections and declared manifest/image profiles. Reuse F-002/038/040–043 and applicable F-046 behavior. | Before implementation, recurring user tasks MUST show useful source fidelity or saved effort beyond ordinary downloads/external viewers, with justified maintenance scope. Before delivery, fixtures MUST verify exact acquired bytes/hash, preserved manifest/attribution/acquisition context, explicit partial/derived-resource status, bounded retrieval and independent export/recovery; applicable RC-001/002/003/009/010/013 and the RC-014 import/interoperability subset MUST pass. Bounded import delivery does not require the later publishing/annual-service subset or establish full institutional completion. |
| IIIF-3 — Institutional partnership | Deliver F-047 selected IIIF publishing for a concrete institutional partner. Define the external consumer, supported interoperability profile and delivery scope separately from import support; include the annual institutional offer and collection-system/metadata workflows. | Before implementation, a partner use case and credible funded maintenance MUST be established. Before delivery, the agreed external consumer MUST demonstrate interoperability under RC-014, with applicable RC-002/003/007/013 and any RC-011 credential/watermark claims verified. Storage, derivative/cache, transfer, access, withdrawal/retention and support responsibilities MUST be specified and costed. |

When IIIF-2 proceeds, the adapter MUST declare finite traversal, request, byte and time limits, expose unsupported/partial results, and avoid default recursive collection harvesting. A manifest, rights statement or attribution MUST remain an attributed source observation. A remote link, partial tile cache or provider-limited rendition MUST NOT be labeled an independently preserved archival master. Completed imports MUST initialize F-043 records, satisfy F-041 recording/completeness requirements and preserve retained source/HTTP evidence under F-042; any capture gaps remain governed by F-041.

When IIIF-3 proceeds, publication MUST use a separate authorized delivery projection of selected media and reviewed metadata; private encrypted OCFL roots MUST NOT become public endpoints. F-046's enabled/disabled scrubbing and retained-field policy MUST govern delivered files, thumbnails, manifests and accompanying metadata, without implicit disclosure of keys or private HTTP records. Transformed images MUST NOT inherit claims of valid C2PA binding or watermark recovery without validation of that rendition; issued artifacts MUST remain separately identifiable. Import support MUST NOT imply adoption of every IIIF API, an annotation platform or a public repository.

These milestones and F-047's collection-system, professional-metadata and batch-publishing obligations MUST remain visible in future implementation planning, with unmet gates recorded as pending institutional work. They MUST NOT be silently promoted to consumer launch commitments or reclassified as optional F-035 extensions. No partner agreement or IIIF-specific price is assumed.

## Service rollout gates

| Gate | Required evidence | Product dependency |
|---|---|---|
| C-1 Local validation | Repeated primary-library use and successful export/recovery | FileSquid release; migration optional |
| C-2 Protection validation | Key recovery, clean-device full restore, security review and verified coverage | F-029/031/042/059 prototypes, independent-copy recovery and RC-010 before offering hosted protection |
| C-7 Provenance publication readiness | Third-party watermark/signing/retrieval contract, supported outputs/destinations, scoped disclosure comprehension, recipient acceptance, billing/cancellation terms | F-043/044/046 and RC-011/013; may launch independently of full backup/sync and final photo-manager parity, with applicable RC-003/007 gates |

Gate IDs C-3 to C-6 are retired. F-033/034 hosted sharing follows its own applicable security gates, and each F-035 option is gated by its own scope and terms definition.

Source: [V](./product-vision.md), [C-042](./product-vision.md#parity-preservation-and-sequence). Later annual institutional services additionally require F-032/047, NFR-010, applicable RC-007 evidence and independent RC-014/IIIF-3 readiness; they are not an F-035 optional-extension decision. No dates, staffing levels or permanent prices are implied.

## RC-001: Original integrity and recovery

**Criteria:** NFR-001 passes every import/edit/export/migration fault fixture; clean restore verifies originals and catalog relationships; deletion/duplicate recovery follows stated policy.

**Verification:** Cryptographic manifests before/after, injected failures at each persistence boundary, restart/retry, original round-trip and independent destination restore. Any silent data loss blocks release.

## RC-002: Performance and resource bounds

**Criteria:** Assigned NFR-002/003 profiles pass from the FileSquid release and at every added workflow gate, on every host and distribution, with declared hardware/browser/codec/cache/network conditions and complete structured reports. Feedback, useful-preview/prepared-image, final-quality, frame/transition, resource and recovery thresholds MUST pass independently. Profile-specific cold-readiness, missed-frame/lateness, tail-sample and background-progress limits MUST be defined before acceptance; absent limits or an all-unsupported difficult-media profile do not pass.

**Verification:** Measure p95/p99 interaction and image readiness, frame stalls, missed-frame rate, slide lateness and preparation coverage in cold/warm/evicted/missing-cache states under idle and concurrent import/migration/thumbnail/index/scan/metadata work. Test direction changes, sort/filter/permission changes, high decoded-pixel counts, missing derivatives, browser quota loss and resource pressure. Record peak decoded/cache/process memory, staging, queue/concurrency caps, cache hit/miss/eviction/redecode, speculative bytes, background progress, resume time/completed-work reuse/repeated bytes, throughput and 72-hour soak. Relevant cloud pilots additionally load-test proposed 100 concurrently active sessions with 20 concurrent resumable transfers and disclose infrastructure sizing. This concurrency is a proposed pilot test, not a capacity promise; no measured app performance is asserted by this document.

## RC-003: Security, privacy and independent browser operation

**Criteria:** F-029 key lifecycle and NFR-004 isolation/privacy pass; the encrypted browser host can unlock, browse, organize and export uploaded supported files with desktops off. Initial acceptance MUST cover the macOS desktop host and `filesquid-server` (including multi-user authorization and the stated operator privacy boundary); the encrypted browser host and later Linux/Windows support require their own declared evidence. The `*.filesquid.tech` service MUST pass the ciphertext-only dependency-graph check ([F-059](#f-059-hosted-encrypted-backup-and-sync-service)).

**Verification:** Stolen token/revoked device/shared-member tests, locked browser/logout/cache tests including prefetched media and cached search/view results, malicious archive/media tests, recovery drill and independent crypto review. Verify operational logs and ordinary private backup objects contain no private plaintext, including HTTP secrets. Test approved F-044/F-045 disclosures separately: only selected content reaches its authorized destination, without implicit library keys or private traces. Default owner-readable local content and separately encrypted evidence MUST match the documented F-042 boundary.

## RC-004: Migration fidelity and accountability

**Criteria:** All applicable MIG/MS requirements and route versions pass the declared corpus; 100% of inventoried items/relationships have outcomes; full success requires zero unresolved entries within the selected supported scope. Any unavailable, failed or ambiguous in-scope item means completion with issues; approved exclusions remain separately reported.

**Verification:** Apple exports/PhotoKit, multipart/late-sidecar Takeout, Windows local and OneDrive placeholder fixtures, fault injection and source hashes. Run the self-service inventory/transfer/recovery/report workflow unsubscribed, verify completed-work reuse, and distinguish paid capacity/transport from free tools. Explicit exclusions are reported separately and never counted as transferred originals.

## RC-005: Functional and accessible UI quality

**Criteria:** Assigned F requirements and NFR-002/006/007 pass on released platforms; no uncaught browser, hydration or SSR errors occur during built-artifact scenarios. macOS desktop is the initial target (the FileSquid MVP, [ADR 035](./architecture/035-macos-first-desktop-mvp.md)); `filesquid-server` and its web UI, Linux and Windows desktop, and the encrypted browser host are each verified separately at their own gates.

**Verification:** Unit/BDD/integration plus production-build UI tests with strict DOM/state assertions for the correct image/resource/edit version, useful quality, ordering, focus/selection, progressive replacement and progress/error/unavailability states. Timing/resource evidence under RC-002 MUST accompany concurrent-work browsing/slideshow cases rather than replacing DOM correctness with a responsive spinner. Include keyboard/screen-reader review and media/render golden comparisons; human review covers appearance, accessibility and permissions comprehension beyond automated assertions.

## RC-006: Backup, sync and collaboration correctness

**Criteria:** Concurrent/offline edit, delete-vs-edit, long-offline replay, quota exhaustion, key/device revocation and shared departure/closure produce explicit recoverable states.

**Verification:** Multi-device convergence and independent backup restore; terminated/mutated resumable batches; restored keys and client-side hashes; unauthorized family/group payer/removed-member attempts rejected. Exercise non-family membership under the same invitation/departure/permission rules and proposed organizer-plus-five minimum, without assuming enterprise identity administration.

## RC-007: Operational readiness

**Criteria:** Applicable C-2 protection evidence or C-7 publication evidence, license/component scope, provider/geography, retention/cancellation/support policies and incident/recovery runbooks are approved before hosted-service claims.

**Verification:** Cancellation/restore exercises, security review, dependency/license review and service recovery drills. No fixed price or SLA inferred from this draft.

## RC-008: Complete parity and documentation traceability

**Criteria:** Every granular Photos inventory item has F-040 status/evidence; all 60 F, 10 NFR and 5 TC requirements (F-001–F-060, NFR-001–010, TC-001–005) MUST map to system specifications and tests before their applicable implementation completion gate. Each F requirement MUST state whether FileSquid or the ImageSquid distribution delivers it. FileSquid release completion covers the subset assigned in [Release order and dependency gates](#release-order-and-dependency-gates); consumer completion across the FileSquid, encrypted backup and sync, and ImageSquid releases covers F-001–046, F-049–F-059 and RC-001–013; later institutional F-047/F-060/RC-014 and later optional F-048 MUST retain traceability and explicit pending status without blocking consumer parity. Mandatory gaps cannot be hidden by aggregate coverage percentages.

**Verification:** Source inventory checklist, architecture allocation, future bidirectional SRS/BDD links, supported-platform/format register and stakeholder acceptance of explicit proprietary limitations. This document is currently a draft input to that verification, not evidence it already passed.

## RC-009: Source evidence and C2PA lifecycle

**Criteria:** F-041's nonempty supported recorder matrix passes full controlled request/response capture; where the `c2pa` module is enabled, F-043 initializes every completed import, links prior credentials and records required C2PA actions linked to the F-067 PROV-O activities of the same operations. Applicable free collection-wide scan/report workflows MUST pass; free/offline records and scans never depend on paid signing.

**Verification:** GET/POST/form/API/file-upload fixtures with binary bodies, repeated headers, credentials/Cookie/Set-Cookie, redirects, errors, cache/service-worker responses, raw/decoded encodings and absent/empty/unavailable/truncated components. Compare exact captured bytes and associations, require partial status for gaps, and verify protected evidence is absent from operational logs and unintended disclosures. Import camera-roll, desktop, migration and web fixtures (as their routes ship), existing credentials and unsupported embedding; inject crashes between media/record commits and check retry idempotency. Edit/metadata/AI/export/undo/redaction/watermark fixtures verify action policy, immutable prior manifests, original hashes and honest unsigned/validation states.

Verify that each C2PA action links to the F-067 PROV-O activity of its operation (the PROV-O history itself is verified under RC-010). Scan mixed libraries offline and unsubscribed, interrupt/resume, change resources and compare item-level/aggregate scope, coverage, incomplete status, validation age/policy and absent/unsigned/binding/trust outcomes. Confirm scans do not sign or rewrite media and report disclosure excludes private evidence unless explicitly authorized for owner archival export.

## RC-010: Complete OCFL preservation and independent recovery

**Criteria:** F-042's default readable-local OCFL structure and applicable hosted/encrypted package structure validate against their actual bytes. An authorized clean client MUST reconstruct the complete declared recovery set without the original operational database, running FileSquid service or active subscription. Local durability and hosted protection MUST have distinct evidence; protection status is justified by the deployed copy/key/verification policy. Local OCFL acceptance applies from the FileSquid MVP for its supported imports/metadata, including the [F-067](#f-067-prov-o-history-of-every-version-and-change) PROV-O history and the recovery of original file names from each file's metadata record.

**Verification:** Independent standard validation plus client decryption/fixity checks; compound resources, metadata-only versions, albums/identities/rejections/recipes, full HTTP and PROV-O evidence, source-clipped video, C2PA histories and published derivatives. Exercise root isolation, parallel/interrupted writes and inventory/digest races, missing/corrupt keys/content, lost catalog, independently controlled recovery copies and checkpoints, valid purge/locked retention, and encrypted/plaintext exports with matching inventories. Measure actual retained bytes, copies, verification reads and restoration work; no synthetic cost estimate is accepted as measured production cost.

Close the app, copy the documented canonical collection with ordinary filesystem tools, inspect supported media/sidecars and rebuild supported organization without first exporting or retaining the original database. Verify metadata/`dcterms:`-only commits, crash boundaries and snapshot references, separate encrypted HTTP evidence, explicitly unavailable/referenced-resource coverage and unknown fields. Verify that every committed version carries its PROV-O record in the same commit (import, rename, move, hide/delete/restore, metadata edit, export/scrub, revert, repair, backfill), that activities, entities, agents, UTC times and journal links survive clean recovery and are readable with a JSON parser, that original file names are recovered from `<file-uuid>.metadata.md` files alone, and that ephemeral cache work creates no permanent history. No change reported complete may exist only in the operational catalog; altered signed bytes/inventories MUST be detected, and optional conversions MUST retain exact originals plus lineage.

## RC-011: Paid third-party publishing and watermark delivery

**Criteria:** F-044 returns a correctly watermarked derivative and bound signed credential through at least one declared provider/format/destination profile using its supported desktop/browser in-app workflow. Purchase remains optional for the owner; consumer product completion requires this working capability.

**Verification:** Selected/saved-scope authorization; original unchanged; provider-visible payload inspection; watermark-before-final-binding behavior; signature/ingredient/trust evaluation; watermark/locator recovery and declared transformation/destination tests. Inject timeout, duplicate submission, cancellation, quota, provider/lookup outage and corrupted/mismatched results; require honest state and idempotent billing. Verify retained artifacts and recovery after cancellation, separate signer/library keys, published retention/locator terms and C-7 cost/recipient evidence. A decorative logo, signature-only result or universal unsupported state fails this gate.

On every declared desktop/browser surface, verify in-app selection, price/disclosure review, submission, progress, permitted cancellation/retry, result inspection and retained-output retrieval. Provider authentication/payment handoffs MUST restore the appropriate app job/session state; an external website-only path fails the workflow requirement.

## RC-012: User-funded AI connections and task outcomes

**Criteria:** F-045 connects supported user-owned local and frontier accounts, applies per-task model choices and preserves confirmed collection knowledge without a FileSquid-funded inference path.

**Verification:** OpenAI-compatible profile plus required native/runtime adapters, endpoint-bound secrets, authentication/entitlement errors, model capability changes, remote-disclosure scopes, local failure without silent remote fallback, budgets/cancellation and bounded retries. The AI Connections surface MUST show the standing disclosure and inspectable **How AI Connections work** explanation; a newly authorized remote job MUST show destination, extra hop, file count, image size, metadata policy, payer, Confirm and Cancel. Exercise at least one supported local route and one supported frontier route; validate each enabled task independently, including specialist face/pet contracts where offered, prompt-injection resistance, preview/undo, disclosure history and correction-preserving index rebuild. Provider usage/account receipts establish user billing; no user inference bill is booked to FileSquid or a distribution.

## RC-013: Metadata fidelity and configurable sharing privacy

**Criteria:** F-046's nonempty photo/video metadata and `dcterms:` / `schema:` type/operator matrix passes, including free single/batch authoring, populated-only progressive disclosure, namespace prefixes, baseline search and supported virtual-collection predicates. All enabled sharing/publication routes honor user-selected enabled/disabled mode, retained fields and outgoing filename policy, while originals/private metadata remain intact and recoverable. The default is enabled with no retained fields and FileId (UUID version 4) plus extension as the outgoing name; metadata/privacy controls require no payment.

**Verification:** Fixtures include nested/duplicate EXIF, GPS, MakerNotes, thumbnails, EXIF-derived XMP, IPTC, proprietary/read-only metadata, malformed values, conflicting dates/zones, Unicode, MOV/MP4 native and timed location data, compound resources and signed originals. Verify attributed extraction, reversible batch correction, album/search updates and exact binding after OCFL export/clean recovery. Exercise enabled/empty-list full scrubbing, enabled/selective camera/date/GPS retention, and explicitly disabled scrubbing; independently inspect actual outputs for both absent and retained populated fields. Verify the privacy-on default outgoing filename is FileId plus extension, that **Use original filename** is an explicit previewed choice, and that owner archival/original export still records original names in its manifest. Verify saved-policy/one-job overrides, namespace collisions and mirrored values, preview/status accuracy, media appearance/playback, companions and original hashes. Inject unsupported writers/scanners and ensure no silent raw fallback or automatic disabling. Test shared-library/download/partner routes and the publisher payload before upload and after final watermark/signature output; require reviewed provenance and no implicit private evidence. These are future acceptance requirements, not tests executed by this documentation task.

Exercise `dcterms:` statement creation, inspection, edit, removal, Schema.org overflow suggestion, extension-namespace registration and statements (including rejection of unregistered prefixes), unknown-field preservation and type validation on files, including batch preview/undo/interruption, duplicate filenames and binding after restart, export/reimport and metadata-only OCFL recovery. Verify rejection of unknown `dcterms:` local names and of collection-domain `accrual*` on a file. Verify search/virtual-collection invalidation, original-vs-owner attribution and `dcterms:`/`schema:`/unknown field disclosure in files, sidecars and reports; unconstrained custom-field namespaces MUST NOT be accepted as consumer descriptive metadata and MUST NOT gain unsupported professional-conformance claims.

## RC-014: Institutional interoperability and service readiness

**Criteria:** F-047's nonempty named collection-system/metadata/IIIF matrix passes its supported profiles, with defined institutional tasks, concrete consumer/partner evidence, annual service terms and credible funded maintenance. Institutional completion requires this full gate plus applicable shared F/NFR/TC and RC gates; consumer RC-001–013 and photo-manager parity remain independent. A partial IIIF-2 import release MUST pass its declared import/interoperability subset without waiting for IIIF-3 publishing or annual-service evidence, and MUST retain those unmet subsets as pending rather than claim institutional completion.

**Verification:** Exercise declared collection-system identifier/relationship and professional metadata mappings, unknown/conflicting/lossy fields, supported batch watermark/publication with per-item failure/resume outcomes, and bounded IIIF import/export against the agreed external consumer. Validate exact originals, PROV-O/C2PA lineage, readable-local/OCFL recovery, selected disclosure and private-root isolation, access withdrawal and retained-output portability. Require F-044/RC-011 evidence for C2PA-linked watermark claims and NFR-002/003/RC-002 evidence for institutional workload profiles. Review annual billing/capacity/transfer/support/retention/cancellation terms, measured costs and partnership/funding evidence without inventing prices, agreements or superiority over other systems. Pending institutional gates MUST stay explicitly pending and MUST NOT be counted as completed consumer features.

## Vision allocation

Every requirement row in the [FileSquid vision](./product-vision.md) (U-01–U-89 and every FE-, C- and T- row) is allocated here to at least one PRD item. The ImageSquid vision is allocated in the ImageSquid PRD. FE-, C- and T- numbers are stable across both visions; U-numbers are per vision. T-059 is the next unused T-ID, not a requirement. [check_prd_allocation.py](../scripts/docs/check_prd_allocation.py) verifies this table against the FileSquid vision, and the ImageSquid PRD's table against the ImageSquid vision.

### FileSquid vision

| Vision ID | PRD allocation |
|---|---|
| FE-001, FE-065 | [F-001](#f-001-independent-local-library-and-ownership) |
| FE-002, FE-003 | [F-009](#f-009-multiple-libraries-and-filesystem-administration), [F-049](#f-049-product-component-terminology-and-item-type) |
| FE-004 | [F-002](#f-002-safe-file-folder-and-device-import), [F-008](#f-008-deletion-hidden-files-and-duplicate-review), [F-010](#f-010-export-and-portable-backup) |
| FE-011, FE-049, FE-050, FE-051, FE-052, FE-053, FE-067 | [F-045](#f-045-byo-ai-connections-and-per-task-model-choice) |
| FE-017, FE-019 | [F-036](#f-036-browser-and-mobile-web-experience), [F-037](#f-037-setup-updates-and-diagnostics), [F-040](#f-040-compatibility-and-parity-register), [F-057](#f-057-desktop-and-server-hosts) |
| FE-020, FE-021 | [F-029](#f-029-client-encryption-and-key-custody)–[F-031](#f-031-encrypted-backup-and-verified-restore), [F-059](#f-059-hosted-encrypted-backup-and-sync-service), [TC-005](#tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer) |
| FE-022 | [F-033](#f-033-shared-albums-and-hosted-galleries), [F-034](#f-034-family-accounts-and-shared-libraries), [F-057](#f-057-desktop-and-server-hosts) |
| FE-029, FE-030 | [F-025](#f-025-migration-selection-inventory-and-pilot)–[F-027](#f-027-migration-execution-recovery-and-reconciliation) |
| FE-031, FE-032 | [F-041](#f-041-source-aware-web-clipping-and-http-evidence) |
| FE-033, FE-034, FE-035 | [F-042](#f-042-ocfl-preservation-and-portable-recovery), [F-059](#f-059-hosted-encrypted-backup-and-sync-service) |
| FE-036, FE-037, FE-038, FE-039, FE-040, FE-041 | [F-043](#f-043-c2pa-records-and-action-disclosure) |
| FE-082 | [F-067](#f-067-prov-o-history-of-every-version-and-change) |
| FE-042, FE-043, FE-044, FE-045, FE-046, FE-047, FE-048, FE-066 | [F-044](#f-044-paid-c2pa-publishing-and-watermarking) |
| FE-054, FE-055, FE-056, FE-057, FE-068 | [F-046](#f-046-media-metadata-inspection-correction-and-privacy-exports) |
| FE-058, FE-059 | [F-003](#f-003-browsing-selection-and-viewing), [NFR-002](#nfr-002-benchmark-envelope-and-responsiveness), [NFR-003](#nfr-003-bounded-processing-and-migration-endurance) |
| FE-060, FE-061, FE-062 | [F-047](#f-047-institutional-dam-and-iiif-interoperability) |
| FE-063 | [F-047](#f-047-institutional-dam-and-iiif-interoperability), [F-060](#f-060-archival-curation-jobs-and-datasets) |
| FE-064 | [F-037](#f-037-setup-updates-and-diagnostics), [F-057](#f-057-desktop-and-server-hosts), [TC-005](#tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer) |
| FE-069 | [F-051](#f-051-ocfl-module), [F-042](#f-042-ocfl-preservation-and-portable-recovery) |
| FE-070 | [F-052](#f-052-metadata-module), [F-046](#f-046-media-metadata-inspection-correction-and-privacy-exports) |
| FE-071 | [F-053](#f-053-virtual-exhibit) |
| FE-072 | [F-054](#f-054-c2pa-module), [F-043](#f-043-c2pa-records-and-action-disclosure) |
| FE-073 | [F-055](#f-055-default-ui-and-panel-composition) |
| FE-074 | [F-056](#f-056-single-filesquid-api) |
| FE-075 | [F-057](#f-057-desktop-and-server-hosts) |
| FE-076, FE-077 | [F-058](#f-058-white-label-distributions), [F-050](#f-050-filesquid-platform-and-distributions) |
| FE-078 | [F-059](#f-059-hosted-encrypted-backup-and-sync-service) |
| FE-079 | [F-060](#f-060-archival-curation-jobs-and-datasets) |
| U-01 | [F-055](#f-055-default-ui-and-panel-composition) |
| U-02 | [F-001](#f-001-independent-local-library-and-ownership), [F-032](#f-032-accounts-entitlements-billing-and-cancellation) |
| U-03 | [F-001](#f-001-independent-local-library-and-ownership), [F-042](#f-042-ocfl-preservation-and-portable-recovery) |
| U-04 | [F-049](#f-049-product-component-terminology-and-item-type) |
| U-05 | [F-004](#f-004-collections-folders-and-navigation), [F-009](#f-009-multiple-libraries-and-filesystem-administration), [F-049](#f-049-product-component-terminology-and-item-type) |
| U-06 | [F-057](#f-057-desktop-and-server-hosts), [F-033](#f-033-shared-albums-and-hosted-galleries), [F-034](#f-034-family-accounts-and-shared-libraries) |
| U-07 | [F-053](#f-053-virtual-exhibit) |
| U-08 | [F-002](#f-002-safe-file-folder-and-device-import), [F-025](#f-025-migration-selection-inventory-and-pilot)–[F-028](#f-028-supported-migration-routes) |
| U-09 | [F-031](#f-031-encrypted-backup-and-verified-restore), [F-059](#f-059-hosted-encrypted-backup-and-sync-service) |
| U-10 | [F-038](#f-038-open-interfaces-and-product-exit), [F-042](#f-042-ocfl-preservation-and-portable-recovery), [F-059](#f-059-hosted-encrypted-backup-and-sync-service) |
| U-11 | [NFR-002](#nfr-002-benchmark-envelope-and-responsiveness), [NFR-003](#nfr-003-bounded-processing-and-migration-endurance), [RC-002](#rc-002-performance-and-resource-bounds) |
| U-12 | [F-032](#f-032-accounts-entitlements-billing-and-cancellation), [F-059](#f-059-hosted-encrypted-backup-and-sync-service) |
| U-13 | [F-038](#f-038-open-interfaces-and-product-exit), [NFR-008](#nfr-008-maintainability-and-verified-engineering) |
| U-14 | [F-036](#f-036-browser-and-mobile-web-experience), [F-040](#f-040-compatibility-and-parity-register) |
| U-15 | [F-050](#f-050-filesquid-platform-and-distributions) |
| U-16, U-17 | [F-056](#f-056-single-filesquid-api), [TC-005](#tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer) |
| U-18 | [F-056](#f-056-single-filesquid-api), [F-057](#f-057-desktop-and-server-hosts), [F-037](#f-037-setup-updates-and-diagnostics) |
| U-19 | [F-057](#f-057-desktop-and-server-hosts) |
| U-20 | [F-057](#f-057-desktop-and-server-hosts), [F-059](#f-059-hosted-encrypted-backup-and-sync-service), [TC-005](#tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer) |
| U-21 | [F-059](#f-059-hosted-encrypted-backup-and-sync-service) |
| U-22, U-23 | [F-057](#f-057-desktop-and-server-hosts), [F-036](#f-036-browser-and-mobile-web-experience), [TC-005](#tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer) |
| U-24 | [F-058](#f-058-white-label-distributions), [F-055](#f-055-default-ui-and-panel-composition) |
| U-25 | [F-058](#f-058-white-label-distributions), [F-056](#f-056-single-filesquid-api) |
| U-26 | [F-037](#f-037-setup-updates-and-diagnostics), [F-056](#f-056-single-filesquid-api), [TC-005](#tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer) |
| U-27, U-28 | [F-037](#f-037-setup-updates-and-diagnostics), [NFR-003](#nfr-003-bounded-processing-and-migration-endurance) |
| U-29 | [TC-005](#tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer), [F-050](#f-050-filesquid-platform-and-distributions) |
| U-30, U-31, U-32 | [TC-005](#tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer), [F-059](#f-059-hosted-encrypted-backup-and-sync-service) |
| U-33, U-34 | [F-037](#f-037-setup-updates-and-diagnostics) |
| U-35, U-37, U-38 | [TC-005](#tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer) |
| U-36 | [F-036](#f-036-browser-and-mobile-web-experience) |
| U-39 | [F-047](#f-047-institutional-dam-and-iiif-interoperability), [F-060](#f-060-archival-curation-jobs-and-datasets) |
| U-40, U-43, U-44, U-46, U-47, U-48, U-49, U-51, U-52, U-53 | [F-051](#f-051-ocfl-module) |
| U-41, U-42, U-45, U-50 | [F-051](#f-051-ocfl-module), [F-042](#f-042-ocfl-preservation-and-portable-recovery) |
| U-54, U-56, U-57, U-58, U-59, U-60, U-61, U-62, U-63, U-64 | [F-052](#f-052-metadata-module) |
| U-55 | [F-052](#f-052-metadata-module), [F-046](#f-046-media-metadata-inspection-correction-and-privacy-exports) |
| U-65 | [F-052](#f-052-metadata-module), [F-055](#f-055-default-ui-and-panel-composition) |
| U-66 | [F-046](#f-046-media-metadata-inspection-correction-and-privacy-exports) |
| U-67, U-68, U-69, U-70, U-71, U-72, U-73, U-74, U-76 | [F-054](#f-054-c2pa-module) |
| U-75 | [F-054](#f-054-c2pa-module), [F-043](#f-043-c2pa-records-and-action-disclosure) |
| U-77 | [F-055](#f-055-default-ui-and-panel-composition), [F-054](#f-054-c2pa-module) |
| U-78 | [F-043](#f-043-c2pa-records-and-action-disclosure), [F-067](#f-067-prov-o-history-of-every-version-and-change), [F-002](#f-002-safe-file-folder-and-device-import) |
| U-89 | [F-067](#f-067-prov-o-history-of-every-version-and-change) |
| U-79 | [F-011](#f-011-non-destructive-editing-workflow)–[F-018](#f-018-markup-and-companion-annotation) |
| U-80 | [F-041](#f-041-source-aware-web-clipping-and-http-evidence) |
| U-81 | [F-045](#f-045-byo-ai-connections-and-per-task-model-choice) |
| U-82 | [F-022](#f-022-memories-and-storytelling), [F-045](#f-045-byo-ai-connections-and-per-task-model-choice) |
| U-83 | [F-044](#f-044-paid-c2pa-publishing-and-watermarking) |
| U-84 | [F-044](#f-044-paid-c2pa-publishing-and-watermarking), [F-047](#f-047-institutional-dam-and-iiif-interoperability) |
| U-85, U-86 | [F-060](#f-060-archival-curation-jobs-and-datasets) |
| C-001 | [F-036](#f-036-browser-and-mobile-web-experience) |
| C-002 | [F-001](#f-001-independent-local-library-and-ownership), [NFR-002](#nfr-002-benchmark-envelope-and-responsiveness) |
| C-003, C-004 | [F-030](#f-030-personal-synchronization-and-availability), [F-031](#f-031-encrypted-backup-and-verified-restore), [F-059](#f-059-hosted-encrypted-backup-and-sync-service) |
| C-006, C-007 | [F-032](#f-032-accounts-entitlements-billing-and-cancellation), [F-001](#f-001-independent-local-library-and-ownership) |
| C-008, C-009 | [NFR-002](#nfr-002-benchmark-envelope-and-responsiveness), [NFR-003](#nfr-003-bounded-processing-and-migration-endurance) |
| C-010 | [F-041](#f-041-source-aware-web-clipping-and-http-evidence) |
| C-011, C-035 | [F-043](#f-043-c2pa-records-and-action-disclosure) |
| C-012 | [F-044](#f-044-paid-c2pa-publishing-and-watermarking) |
| C-013, C-014, C-033, C-034, C-043 | [F-046](#f-046-media-metadata-inspection-correction-and-privacy-exports) |
| C-015 | [F-031](#f-031-encrypted-backup-and-verified-restore), [F-042](#f-042-ocfl-preservation-and-portable-recovery) |
| C-016, C-018, C-021, C-022, C-023, C-024 | [F-044](#f-044-paid-c2pa-publishing-and-watermarking) |
| C-017 | [F-011](#f-011-non-destructive-editing-workflow) |
| C-019 | [F-045](#f-045-byo-ai-connections-and-per-task-model-choice), [F-048](#f-048-later-hosted-private-oss-inference) |
| C-020 | [F-019](#f-019-people-pets-and-groups), [F-022](#f-022-memories-and-storytelling) |
| C-025, C-026, C-027, C-052, C-056 | [F-050](#f-050-filesquid-platform-and-distributions), [F-058](#f-058-white-label-distributions) |
| C-028, C-029, C-030 | [F-001](#f-001-independent-local-library-and-ownership), [F-009](#f-009-multiple-libraries-and-filesystem-administration), [F-042](#f-042-ocfl-preservation-and-portable-recovery) |
| C-031 | [F-032](#f-032-accounts-entitlements-billing-and-cancellation), [F-059](#f-059-hosted-encrypted-backup-and-sync-service) |
| C-032, C-042 | [F-032](#f-032-accounts-entitlements-billing-and-cancellation) |
| C-038 | [F-040](#f-040-compatibility-and-parity-register) |
| C-041 | [F-028](#f-028-supported-migration-routes) |
| C-044, C-047 | [F-051](#f-051-ocfl-module) |
| C-057 | [F-050](#f-050-filesquid-platform-and-distributions), [F-051](#f-051-ocfl-module), [F-052](#f-052-metadata-module), [F-054](#f-054-c2pa-module) |
| C-045, C-046 | [F-051](#f-051-ocfl-module), [F-060](#f-060-archival-curation-jobs-and-datasets) |
| C-048, C-049 | [F-052](#f-052-metadata-module) |
| C-050 | [F-053](#f-053-virtual-exhibit), [F-041](#f-041-source-aware-web-clipping-and-http-evidence) |
| C-051 | [F-032](#f-032-accounts-entitlements-billing-and-cancellation), [F-060](#f-060-archival-curation-jobs-and-datasets) |
| C-053 | [F-051](#f-051-ocfl-module), [F-052](#f-052-metadata-module), [F-054](#f-054-c2pa-module), [F-057](#f-057-desktop-and-server-hosts), [TC-005](#tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer) |
| C-054 | [F-054](#f-054-c2pa-module), [F-043](#f-043-c2pa-records-and-action-disclosure) |
| C-055 | [F-055](#f-055-default-ui-and-panel-composition), [F-046](#f-046-media-metadata-inspection-correction-and-privacy-exports) |
| T-001 | [TC-005](#tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer), [TC-002](#tc-002-endpoint-processing-and-hosted-encryption), [F-057](#f-057-desktop-and-server-hosts) |
| T-002 | [F-036](#f-036-browser-and-mobile-web-experience), [F-057](#f-057-desktop-and-server-hosts), [TC-005](#tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer) |
| T-003 | [TC-005](#tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer), [TC-002](#tc-002-endpoint-processing-and-hosted-encryption) |
| T-033, T-034 | [TC-005](#tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer), [TC-004](#tc-004-decision-status-and-repository-alignment) |
| T-004, T-022 | [F-036](#f-036-browser-and-mobile-web-experience) |
| T-005, T-007, T-029, T-035, T-036 | [F-001](#f-001-independent-local-library-and-ownership), [F-042](#f-042-ocfl-preservation-and-portable-recovery) |
| T-006, T-008 | [F-041](#f-041-source-aware-web-clipping-and-http-evidence) |
| T-009, T-010, T-011, T-012, T-013, T-014, T-015, T-017, T-018, T-019, T-020, T-021, T-023, T-024, T-025, T-026, T-027, T-028, T-030, T-031, T-032 | [NFR-002](#nfr-002-benchmark-envelope-and-responsiveness), [NFR-003](#nfr-003-bounded-processing-and-migration-endurance) |
| T-016 | [F-001](#f-001-independent-local-library-and-ownership), [NFR-002](#nfr-002-benchmark-envelope-and-responsiveness) |
| T-037 | [F-011](#f-011-non-destructive-editing-workflow), [F-067](#f-067-prov-o-history-of-every-version-and-change), [F-043](#f-043-c2pa-records-and-action-disclosure) |
| T-058 | [F-067](#f-067-prov-o-history-of-every-version-and-change), [F-042](#f-042-ocfl-preservation-and-portable-recovery), [F-061](#f-061-activity-journal-and-live-health-monitoring) |
| T-038, T-044, T-045, T-046 | [F-052](#f-052-metadata-module), [F-046](#f-046-media-metadata-inspection-correction-and-privacy-exports) |
| T-039, T-040, T-042, T-043 | [F-051](#f-051-ocfl-module) |
| T-041 | [F-051](#f-051-ocfl-module), [F-060](#f-060-archival-curation-jobs-and-datasets) |
| T-047 | [F-053](#f-053-virtual-exhibit) |
| T-048, T-050 | [F-056](#f-056-single-filesquid-api) |
| T-049, T-053 | [F-057](#f-057-desktop-and-server-hosts) |
| T-051 | [F-058](#f-058-white-label-distributions), [F-055](#f-055-default-ui-and-panel-composition) |
| T-052 | [F-051](#f-051-ocfl-module), [F-052](#f-052-metadata-module), [F-054](#f-054-c2pa-module), [TC-005](#tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer) |
| T-054 | [F-059](#f-059-hosted-encrypted-backup-and-sync-service) |
| [Terminology](./product-vision.md#terminology) | [F-049](#f-049-product-component-terminology-and-item-type), [F-058](#f-058-white-label-distributions) |
| [Module specifications](./product-vision.md#module-specifications) and distributions table | [F-051](#f-051-ocfl-module), [F-052](#f-052-metadata-module), [F-054](#f-054-c2pa-module), [F-055](#f-055-default-ui-and-panel-composition), [F-058](#f-058-white-label-distributions), [F-060](#f-060-archival-curation-jobs-and-datasets) |
| [Technical Stack](./product-vision.md#technical-stack) | [Technology Stack](#technology-stack), [TC-005](#tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer) |
| [Constraint requirements](./product-vision.md#constraint-requirements) | [F-050](#f-050-filesquid-platform-and-distributions), [F-054](#f-054-c2pa-module), [F-059](#f-059-hosted-encrypted-backup-and-sync-service) |
| Standards: RFC 2119, RFC 3339, WCAG 2.2 AA, GDPR review, Article 50, OCFL, C2PA, PROV-O, ISO 15836-2 / DCMI Terms, RiC-O, RFC 4180, OpenAPI 3.1, RFC 9562, FAIR/Parquet | This document's RFC 2119 language; [NFR-006](#nfr-006-accessibility-and-usability); [F-044](#f-044-paid-c2pa-publishing-and-watermarking); [F-042](#f-042-ocfl-preservation-and-portable-recovery); [F-043](#f-043-c2pa-records-and-action-disclosure); [F-046](#f-046-media-metadata-inspection-correction-and-privacy-exports); [F-052](#f-052-metadata-module); [F-056](#f-056-single-filesquid-api); [F-060](#f-060-archival-curation-jobs-and-datasets); [TC-001](#tc-001-immutable-originals-and-explicit-identity) |
| IIIF-1–3 (PRD milestones) | [Future IIIF milestones](#future-iiif-milestones), [F-047](#f-047-institutional-dam-and-iiif-interoperability) |
| FE-080, FE-081, T-055, T-056, T-057, U-87, U-88 | [F-061](#f-061-activity-journal-and-live-health-monitoring) |
