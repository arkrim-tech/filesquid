# FileSquid: Product Vision

## Product Vision

**FileSquid is a free, open-source file platform that keeps your files in an archival standard you own, with descriptions and provenance attached, on your own computer, on your own server, or in an encrypted cloud you can leave at any time.**

Everyday cloud drives are convenient, but they keep versions, descriptions and sharing history in their own databases. Leave the service and you get the bytes back, not the history and context. Digital-preservation systems (Archivematica, Preservica) do keep that context, using open standards, but they are built for institutions with archivists and budgets, not for a family, a researcher or a small organisation.

FileSquid closes that gap. It looks and works like a familiar cloud drive: folders, previews, a details sidebar, version history, sharing. After the macOS desktop MVP, encrypted backup and sync is the primary product. The Drive stays the interface for day-to-day file work. OCFL stays behind that Drive. Underneath, every file is stored the way archives store them:

- **[OCFL 1.1](https://ocfl.io/1.1/spec/) objects on disk or S3-compatible storage.** Versioned, checksummed and readable with ordinary file tools, without FileSquid. If the application's database is lost, the whole application can be rebuilt from those files.
- **Metadata in a human-readable record** (`metadata.md`: Dublin Core, with Schema.org for anything Dublin Core cannot say), next to the embedded EXIF, ID3, IPTC and XMP the file already carries. This includes the ability to add any additional arbitrary schemas (e.g. RiC-O for archivists).
- **A [W3C PROV-O](https://www.w3.org/TR/prov-o/) history** of every version and change, stored with the files from the first import: what was done to a file and its metadata, by whom or by which tool, and when, readable without FileSquid.
- **[C2PA](https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html) Content Credentials** for media, from the second consumer release (the `c2pa` module): signed claims about what was done to a file and whether AI was involved. Files imported earlier receive honestly dated backfilled records ([C-025](#release-sequence)).

### One platform, many audiences

FileSquid is one Rust backend with **one API** for all of its features. Every user interface is plain HTML, CSS and TypeScript that calls that API, and nothing else. A **distribution** puts a UI, a theme, a vocabulary and a set of enabled modules on top of the same backend:

| Distribution | Audience | What it adds | Release |
|---|---|---|---|
| **FileSquid** (default) | Anyone who wants a private, durable cloud drive; developers; small organisations | Drive-style file browser with one file panel containing File info, Description and Provenance tabs, a Versions view, a Logs view, the PROV-O history (`prov` module) and, from the second consumer release, the Content Credentials panel; internal vocabulary (repository, library, collection, file) | The Drive people use every day. A macOS desktop MVP comes first. Encrypted backup and sync is the primary product after that MVP. The first consumer release (Linux, Windows and `filesquid-server`) follows backup. Content Credentials (the `c2pa` module and its panel) ship with the second consumer release ([C-025](#release-sequence)) |
| **ImageSquid** | Consumers and families managing photos and videos (ImageSquid vision, imagesquid.app; screenshots in ImageSquid ui-mockups) | Photo-album UI: library, albums, galleries, viewer, studio, exhibits; ImageSquid vocabulary (repo, album, gallery, exhibit) | Second consumer release |
| **Arkrim** | Digital archivists and records managers in memory institutions and organisations | Archives dashboard: batch processing, curation jobs (for example transcoding), Parquet datasets, FAIR reporting | Later |

**Design rule:** distributions are UI layers. Every capability a distribution needs (photo thumbnails and editing recipes for ImageSquid, transcoding jobs and Parquet projection for Arkrim) is built into FileSquid as a module and exposed through the API. A distribution contains no backend code. So a FileSquid user can get thumbnails or a transcoding job without installing ImageSquid or Arkrim.

### Modules

| Module (crate) | Owns | Default dependency on other modules |
|---|---|---|
| `filesquid-core` | Shared vocabulary and contracts: opaque IDs, repository, library, collection, virtual collection, virtual exhibit, file, jobs, crypto envelopes | none |
| `filesquid-ocfl` | **Storage facts:** OCFL 1.1 objects, versions, digests, fixity, sizes, logical paths; the git-like change model | none |
| `filesquid-metadata` | **Every other file fact:** embedded technical metadata, format identification, capture dates, Dublin Core and Schema.org description | none (optional `ocfl` store) |
| `filesquid-c2pa` | **Content Credentials:** C2PA read, validate, mint, edit, embed, report | none |
| `filesquid-prov` | **PROV-O history:** W3C PROV-O records of OCFL versions and changes in a versioned JSON-LD profile; build, validate, read, query and backfill | none (optional `ocfl` feature) |
| `filesquid-api` | The single API, auth, users and groups, events | all of the above |
| Later modules | Media derivatives and editing recipes, import and migration routes, web clipping, curation jobs, Parquet projection | as needed |
| Hosts | `filesquid-desktop` (Tauri 2), `filesquid-server` (Axum), later an encrypted browser host | `filesquid-api` |

Each module crate MUST build, test and publish on its own, so the OCFL, metadata, PROV-O history and C2PA engines stay usable by programs that never run FileSquid.

### Where FileSquid runs

| Host | For | Who can read your files |
|---|---|---|
| Desktop app (Linux, macOS, Windows) | One person or household, offline-first | Only you, on your device |
| `filesquid-server` you run | Families, teams and institutions that self-host, multi-user | You and whoever operates that server |
| Encrypted hosting at `*.filesquid.tech` (paid) | Backup and sync without running a server | Only you: the service stores ciphertext and never has your keys |

## Target Users & Customer Profile

| Segment | Jobs to be done | Pains | Gains |
|---|---|---|---|
| Privacy-conscious individuals and families | Keep a lifetime of documents, photos and records safe, organised and shareable within the family | Cloud drives lock history and context in; self-hosting is hard; backups are untested | A familiar drive on their own computer, files in an open archival layout, paid encrypted backup/sync they can leave |
| Self-hosters and small organisations | Run their own shared drive for a team, club or small business | Self-hosted file-sharing platforms are heavy to run, and their versions live in a database | One Rust server, multi-user, a storage layout readable without the software |
| Researchers and creators | Keep sources, versions and provenance with their files | Provenance is lost between tools; AI involvement is invisible | PROV-O history of every change from import, Dublin Core descriptions, verifiable exports; C2PA Content Credentials with the `c2pa` module |
| Digital archivists and records managers (via Arkrim) | Ingest, describe, preserve and process collections at scale | Institutional systems are expensive; open-source ones need specialist ops | Standard OCFL, versioned batch jobs, Parquet and FAIR outputs on the same platform |
| Developers and preservation programmes | Embed OCFL, metadata or C2PA in their own software | Tools are CLI-only, language-locked, or lack S3 and crash safety | Rust engines with native and WebAssembly builds, one `fsquid` command line, and a documented API |
| Distribution builders (ImageSquid, Arkrim, third parties) | Ship a branded product for an audience without building a backend | A file backend with preservation and provenance is years of work | Theme, vocabulary, views and modules on one maintained platform |

## Core Features

- **Drive-style file browser (default UI):** navigation tree, breadcrumb, list and grid views, previews, upload and download, folders, rename and move, remove and restore, a sidebar with one file panel (File info, Description and Provenance tabs, Versions opening from its header) and a Logs view reached from the sidebar below Settings; the Content Credentials panel joins with the `c2pa` module.
- **Repositories, libraries and collections:** multi-user repositories; personal and group-owned libraries; collections with one storage location and one permission set; virtual collections (saved queries); virtual exhibits (public curated queries).
- **Sharing and permissions:** users, groups, collection-level access, transparent departure and deletion rules.
- **OCFL preservation (`ocfl` module):** OCFL 1.1 storage roots on local disk or S3-compatible storage; crash-safe version publication; a git-like change model (stage, status, diff, commit, log, checkout, revert, reinstate) with automatic delta computation; validation; streaming; enumeration of committed state; commands in the `fsquid` CLI.
- **Metadata (`metadata` module):** extraction of EXIF, ID3, IPTC and XMP; a closed Dublin Core register with Schema.org overflow and registered extension namespaces (for example RiC-O, or an owner-declared custom namespace for free key:value pairs); canonical `metadata.md`; pluggable stores; CSV, JSON sidecar and XMP interchange; Obsidian-style properties panel; privacy scrubbing on share; commands in the `fsquid` CLI.
- **PROV-O history (`prov` module):** every OCFL version and retained change (import, rename, move, hide, delete, restore, metadata edit, export and scrub, revert, repair) recorded as W3C PROV-O in the same OCFL commit, linked to the activity journal, shown in the Provenance tab of the file panel, exportable as JSON-LD and queryable through the API; no-op saves and failed attempts produce no record; no C2PA module needed.
- **Content Credentials (`c2pa` module, second consumer release):** read, validate, mint, edit, embed and scan; credentials started at import once the module is enabled, backfilled for files imported earlier; disclosure-safe collection reports; optional PROV-O projection; commands in the `fsquid` CLI.
- **One API, two hosts now:** desktop (API served in-process, no network port) and `filesquid-server` (HTTP plus an event channel). A WebSocket transport is being evaluated alongside HTTP.
- **White-label distributions:** design tokens (light and dark), replaceable label tables, brand assets, view modules and a module set per distribution; one binary per host.
- **Encrypted backup and sync (paid hosting at `*.filesquid.tech`):** end-to-end encrypted backup recovery points and multi-device sync, as distinct guarantees, scaling from personal to institutional quotas.
- **Durable jobs:** imports, uploads, migrations and batch processing that checkpoint and resume, and that commit their results as new OCFL versions.
- **Later modules, exposed through the API:** media derivatives and non-destructive editing recipes (for ImageSquid); import and migration routes; source-aware web clipping with HTTP evidence; BYO AI connections; paid C2PA publication and watermarking; curation jobs such as transcoding, Parquet projection and FAIR reporting (for Arkrim); the encrypted browser host.

### Module specifications

These details come from the module designs and bind each module in addition to the requirement tables.

**`filesquid-ocfl`**

- Storage roots and objects: open a storage root, discover objects, load NAMASTE declarations, inventories and digest sidecars; support [OCFL community extensions](https://ocfl.github.io/extensions/) where adopted.
- View API: folder listing by logical-path prefix at a pinned version with paging, file details, per-file version history, object versions, size lookup, fixity check, ranged content reads for preview and download, and write operations that each produce a new OCFL version (add or upload, create folder, rename or move, remove, restore a previous version). OCFL cannot store an empty folder: creating a folder means adding a file beneath the new path prefix; the UI MAY show a pending empty folder until then, but it is not persisted.
- The file browser shows **storage facts only** (logical paths and implied folders, versions with timestamps, agents and messages, digests and fixity, sizes); a file-type label from the extension is a display hint.
- Commands in the one FileSquid CLI, `fsquid` ([C-057](#module-rules)): stage, status, diff, commit, log, checkout, revert, reinstate, list, cat, and `fsquid ocfl validate`.
- Constraints: strictly OCFL 1.1, improvements are in implementation quality, not a new layout; the default build MUST stay small enough for a desktop installer and a browser bundle; validation against community tools is mandatory before each release; the public API MUST be documented as if another team were the caller; default features MUST exclude Arrow, Parquet, DuckDB, Iceberg, Axum, Tauri, libvips, FFmpeg and SQLite.
- Default dependencies kept minimal (`thiserror` today; SHA-256 and async traits as the engine grows). Tests use community OCFL fixtures and validators (`rocfl`, `ocfl-py`) as oracles.

**`filesquid-metadata`**

- Extract EXIF and ID3 today, IPTC and embedded XMP next. Descriptive facts map to `dcterms:` or `schema:`; technical tags stay as `exif:` (or the packet's own prefix).
- Create, read, update and delete namespaced statements `{namespace, term, value}`; legacy `dc:` names map to `dcterms:`; collection-only terms (`accrual*`) are rejected on files.
- Canonical `metadata.md`: YAML front matter grouped by namespace plus an optional Markdown body.
- Must provide ability to add arbitrary schemas or key:value pairs
- Stores: `sidecar` (default on native; one `<filename>.metadata.md` next to each file), `ocfl` (feature; native and WebAssembly; the record as a logical file inside the OCFL object, which FileSquid names `<file-uuid>.metadata.md` beside the stored original `<file-uuid>.<ext>`), `sqlite` (feature, native only), or a caller's own `MetadataStore`; plus an optional rebuildable SQLite index. In the browser, callers use `ocfl` or their own store.
- Interchange: CSV (one row per file, `prefix:term` headers, RFC 4180 quoting), XMP sidecar and JSON sidecar read and write. A JSON sidecar is one UTF-8 document per OCFL object that binds `{namespace, term, value}` statements to the OCFL object identifier and the SHA-256 digest of the described logical file. No MODS or SQL interchange.
- Properties API and Metadata panel: populated key/value rows, namespace prefixes, and an Add property search across `/terms/` then Schema.org.
- Constraints: the module maps a deliberate subset of tags and reports the rest as native observations or unsupported; writing metadata back into original bytes is out of scope for the canonical path (originals stay untouched; records live beside them); in WebAssembly there is no filesystem and memory is bounded, so extraction streams bytes and the `sqlite` store is native-only.
- Default dependencies: `kamadak-exif`, `id3`, `serde`, `serde_json`, `sha2`, `thiserror`.

**`filesquid-c2pa`**

- Read manifest stores embedded in assets, in sidecar `.c2pa` files, or by remote reference (fetched through a caller-supplied fetcher); JUMBF and CBOR parsing.
- Validate hard bindings (data, box and BMFF hashes), COSE_Sign1 signatures, certificate chains against caller-supplied trust lists, RFC 3161 timestamps, revocation where available, and ingredient chains, reporting the specification's status codes.
- Mint claims with actions, ingredients, AI-generation and training/mining assertions and custom assertions; sign through a `Signer`.
- Edit: new manifests reference the prior asset as a parent ingredient; update manifests and assertion redaction as the specification allows; never rewrite a signed manifest.
- Embed: JPEG and PNG first, then HEIF/AVIF, BMFF (MP4/MOV), WebP and TIFF/DNG through a format-handler interface; sidecar output for anything else.
- Report: bounded, resumable collection scans and disclosure-safe reports. Optional PROV-O projection to JSON-LD.
- Commands in `fsquid` ([C-057](#module-rules)): `read`, `verify`, `sign`, `edit`, `embed`, `report`. The core has no server, filesystem or network access.
- Cryptographic primitives (SHA-2, ECDSA, Ed25519, RSA-PSS, X.509, CMS/RFC 3161) come from established crates chosen in the first C2PA task and recorded in the module README.
- Constraints: the C2PA specification is large and security-critical (about 184 hours for the first module plan); each product that mints trusted credentials must pass the C2PA conformance programme to obtain production claim-signing certificates; the module pins a specification version and tracks changes; the engine and crypto backend must stay small enough for WebAssembly delivery.

**`filesquid-prov`**

- PROV-O model: `prov:Entity` (OCFL object, version, logical path, SHA-256), `prov:Activity` (one per committed version, with operation kind, operation ID and journal entry IDs), `prov:Agent` (the installation UUID for the owner, host software, tool or model; no person identity), and the `used`, `wasGeneratedBy`, `wasDerivedFrom`, `wasInvalidatedBy`, `wasAssociatedWith`, `wasAttributedTo` and `actedOnBehalfOf` relations, with RFC 3339 UTC times.
- A versioned JSON-LD profile with an embedded context, so records read offline with any JSON parser; bounded parsing and validation; an in-memory history index.
- Optional `ocfl` feature: read and write `provenance/<version>.jsonld` records in OCFL objects through `filesquid-ocfl`, and derive backfill records from inventories.
- Commands in `fsquid` ([C-057](#module-rules)): `prov show`, `prov log`, `prov validate`, `prov export`. Default dependencies exclude every other FileSquid crate, and it never depends on `filesquid-native` or `filesquid-c2pa`; builds for WebAssembly ([ADR 040](./architecture/040-prov-o-history.md)).

**Distributions**

| Distribution | Views | Vocabulary | Primary modules |
|---|---|---|---|
| FileSquid (default) | Drive-style browser with Details, Versions, Metadata and Content Credentials panels | Internal names | `ocfl`, `metadata`, `c2pa` |
| ImageSquid | Photo library (Years, Months, Days, All Photos), albums, galleries, viewer, inspector (Metadata, C2PA, Archive tabs), studio, exhibits | repo, library, album, gallery, exhibit, photo/image/video | plus media, editing, people, migration, AI connections, publication |
| Arkrim | Archives dashboard: ingest, batch curation jobs, dataset and FAIR reporting | Internal names and archival terms | plus curation jobs, `parquet`, IIIF |

### Feature requirements

Feature requirements use `FE-` IDs. Numbers are stable identifiers, not an order; gaps are IDs held by the ImageSquid distribution.

#### Independent library

| ID | Requirement | Related |
|---|---|---|
| FE-001 | Managed local libraries MUST keep owner-readable canonical media and metadata and consistent OCFL snapshots. | U-02, U-03 |
| FE-002 | A user or group MAY own more than one library on a repository. A repository MAY host several libraries. | U-05 |
| FE-003 | Referenced originals MUST have explicit availability. Collections MUST share one storage location and one access-permission set. | U-05 |
| FE-004 | The product MUST provide safe import, backup/restore, repair, duplicate review and unrestricted full-resolution export. | U-02, U-08, U-09 |
| FE-065 | Owners MUST be able to inspect and copy a completed OCFL snapshot with ordinary local file tools and reconstruct supported organization without the original operational database, a running FileSquid service, or a prior app-driven export. | U-03; T-007 |
| FE-069 | FileSquid MUST preserve and recover through its `filesquid-ocfl` module. That crate MUST remain independently installable as a FOSS OCFL 1.1 library, with its commands in the `fsquid` CLI ([C-057](#module-rules)), that digital preservation programmes and other users can use without the FileSquid app, without FileSquid application types, catalogs or UI nouns. FileSquid packaged releases MUST pin the module version they link. | U-40, U-40; C-044, C-047, C-057; T-039, T-040, T-042 |
| FE-070 | FileSquid MUST route all metadata handling through its `filesquid-metadata` module. It MUST extract, create, read, update and delete metadata for files stored through `filesquid-ocfl`, including EXIF, ID3, XMP and other embedded packets mapped onto Dublin Core or Schema.org. Canonical metadata MUST be a Markdown file with YAML front matter in the OCFL object. The Metadata panel MUST present Obsidian-style populated properties with Add property. The module MUST ship CSV import/export and a CLI. | U-55, U-03, U-63; C-043, C-048, C-049; T-038, T-044, T-045, T-046 |
| FE-073 | FileSquid MUST ship a default Drive-style UI with named sidebar panel slots and the Details, Versions, Metadata and Content Credentials panels, as standard custom elements in vanilla TypeScript over the FileSquid API. Distributions MUST compose these panels in their own views (for example the ImageSquid inspector's Metadata, C2PA and Archive tabs). | U-01; C-055 |
| FE-072 | FileSquid MUST perform all C2PA processing (read, validate, mint, edit, embed and scan) through its `filesquid-c2pa` module, implemented from scratch and usable by any developer through its API and the `fsquid` CLI without the FileSquid app. | U-67; C-054, C-057 |

#### Everyday organization

| ID | Requirement | Related |
|---|---|---|
| FE-071 | The product MUST provide exhibits (internal virtual exhibits): public curated queries that MAY span collections across libraries on one repository and public collections on other repositories. Every member MUST be marked for public access. A virtual collection (gallery) remains a saved query or a hand-picked member list inside one library. A virtual exhibit MUST NOT be a second store and MUST offer the same UI features as a physical collection. | U-07, U-05; C-050; T-047 |

#### AI the user chooses

| ID | Requirement | Related |
|---|---|---|
| FE-011 | BYO inference MUST remain the user’s provider relationship. FileSquid MUST NOT treat the user’s frontier account as making FileSquid the GPAI provider. | U-81; FE-049 |
| FE-049 | FileSquid MUST connect existing local runners or the user's own supported frontier-provider account through an OpenAI-compatible baseline and task-aware adapters. | U-81 |
| FE-050 | Users MUST be able to choose compatible models per task and pay their provider directly. FileSquid MUST NOT purchase, subsidize or resell frontier inference. | U-81; C-019 |
| FE-051 | FileSquid MUST NOT place a general-purpose AI model on the market and MUST NOT mark every BYO output as FileSquid-generated. | C-019 |
| FE-052 | FileSquid MUST preserve the provider’s marks and MUST record FileSquid’s own actions honestly. | FE-010, FE-011 |
| FE-053 | Direct AI interaction MUST disclose that the person is interacting with AI as in FE-025. | U-82; FE-025 |
| FE-067 | The AI Connections surface MUST show standing disclosure of how a connection works: who is paid, that encrypted backup does not receive this traffic, that a local failure does not send the library elsewhere, and that remote copies cannot be recalled. A new remote job MUST preview destination, selected inputs, and any extra hop before send. | U-81; T-003 |

#### Cross-platform access

| ID | Requirement | Related |
|---|---|---|
| FE-017 | Desktop (macOS first, then Linux and Windows) and `filesquid-server` delivery and validation MUST come first. The FileSquid MVP MUST target the macOS desktop host only ([C-056](#release-sequence)). The encrypted browser host follows. An installable mobile-web interface MUST show visible capability limits. | U-14; C-001 |
| FE-019 | Per-OS installers, WASM native-processing limits and WASM worker rules remain U-23, U-34 and U-36. | U-23, U-34, U-36 |

#### Managed protection and connectivity

| ID | Requirement | Related |
|---|---|---|
| FE-020 | The product MUST provide end-to-end encrypted originals, previews, sensitive metadata, backup recovery points, resumable personal synchronization and explicit conflicts. | U-06, U-09 |
| FE-021 | Matching native/WASM/WebCrypto test vectors remain U-22. Drift is a defect. | U-22 |

#### Selective collaboration

| ID | Requirement | Related |
|---|---|---|
| FE-022 | The product MUST provide shared collections, comments, contributions, group-owned libraries, recipient access controls and transparent departure and deletion policies, on the multi-user `filesquid-server` and through encrypted sync. | U-06 |

#### Migration with evidence

| ID | Requirement | Related |
|---|---|---|
| FE-029 | Every inventoried item MUST receive an outcome, a preservation explanation and a recovery path. | U-08 |
| FE-030 | Large batches MUST retain verified checkpoints across interruption and reuse completed work. Within-file resume is required where supported; any necessary replay MUST be limited to the affected resource or processing unit and explained to the user. | T-027, T-028 |

#### Source-aware clipping

| ID | Requirement | Related |
|---|---|---|
| FE-031 | User-invoked image/video capture MUST retain source URLs and context, observed versus source-claimed metadata, full recorded HTTP requests/responses and a versioned PROV-O/JSON-LD sidecar. | U-80 |
| FE-032 | Request bodies, authorization credentials and cookies retained by the recorder MUST belong in encrypted private evidence, with clear capture completeness and separate sanitized views and publication outputs. | U-80; T-006; C-010 |

#### Portable preservation

| ID | Requirement | Related |
|---|---|---|
| FE-033 | Local OCFL MUST be present from the first FileSquid release, with complete objects for supported coherent assets and versioned collection snapshots, including metadata-only changes, compound resources, encrypted HTTP evidence, source sidecars and the PROV-O history of every version; C2PA history joins wherever the `c2pa` module is enabled. Each original is stored under its FileId plus extension, with its metadata record `<file-uuid>.metadata.md` (original file name, extension, format, import facts, description) beside it. | U-10, U-03; T-005 |
| FE-034 | Hosted roots MUST be private per stable user ID. Export and clean-device recovery MUST NOT depend on a live service catalog. | U-10 |
| FE-035 | Git-in-OCFL, unpublished `vN` and community-tool rules remain U-50, U-41 and U-42. | U-50, U-41, U-42 |

#### PROV-O history

| ID | Requirement | Related |
|---|---|---|
| FE-082 | Every OCFL version FileSquid commits and every retained file or metadata change MUST produce a W3C PROV-O record in a versioned JSON-LD profile, stored in OCFL in the same commit, linked to its activity-journal entries by opaque IDs, exportable, recoverable with ordinary tools, queryable through the API and shown in the default UI, without the `c2pa` module. | U-89, U-78, U-88; T-037, T-058 |

#### C2PA from import onward

| ID | Requirement | Related |
|---|---|---|
| FE-036 | Where the `c2pa` module is enabled, every camera-roll, desktop, migration and web import MUST start a durable resource-bound C2PA lifecycle, even without payment or connectivity; files imported before then MUST receive honestly dated backfilled records. | U-78; C-011 |
| FE-037 | Prior credentials, generator marks and source bytes MUST remain immutable. FileSquid MUST link its import/action history to existing ingredients and manifests rather than modifying signed records in place. | U-78 |
| FE-038 | Prepared/unsigned records, signed manifests and externally trusted results MUST have distinct statuses. | U-78 |
| FE-039 | When the provenance/AI-mark retain-class is selected, share and publication MUST NOT strip those marks. When it is not selected, stripping MUST be an explicit policy outcome, previewed and verified, not an accidental side-effect of empty EXIF retain-lists. | U-66; C-014 |
| FE-040 | Free bounded collection scans MUST report item-level and aggregate outcomes, progress or incomplete coverage, validation time and trust-policy context, including missing, unsigned and unavailable states, and MUST warn when an item has disclosable AI-action history and the selected share or publication route has no human-perceivable AI disclosure. | U-75 |
| FE-041 | Scans MUST remain free and MUST NOT silently sign, rewrite or publish media. | U-75 |

#### Paid C2PA publication, watermarking and AI disclosure

| ID | Requirement | Related |
|---|---|---|
| FE-042 | An in-app workflow MUST call a selected third-party publisher and return a watermarked derivative with correctly bound signed Content Credentials and retrieval/validation evidence. | U-83; C-012 |
| FE-043 | The professional publication profile MUST, at first public exposure of AI-generated or materially AI-manipulated image, audio or video that would falsely appear authentic, apply a human-perceivable disclosure and keep machine-readable provenance/AI marks that the selected policy did not strip. | U-84 |
| FE-044 | The default visible labelling set MUST be the [EU icons for labelling AI-generated content](https://digital-strategy.ec.europa.eu/en/policies/eu-icons-labelling-ai-generated-content). Another clear, distinguishable, accessible visible or audible form MAY be offered in addition where icons are unsuitable for the medium. | U-84; C-016 |
| FE-045 | Where the owner identifies the content as part of an evidently artistic, creative, satirical, fictional or analogous work, the profile MAY apply a limited appropriate disclosure that does not hamper display or enjoyment. That choice MUST be explicit and previewed. | U-84 |
| FE-046 | Selection, price/disclosure review, job status, permitted cancellation/retry and retained-result access MUST be available on the supported desktop and browser surfaces. | U-83 |
| FE-047 | Original media MUST remain unchanged. Publication MUST remain optional for the user and required as a product capability. | U-83 |
| FE-048 | FileSquid MUST NOT market this workflow as EU AI Act certification. | C-018 |
| FE-066 | Supported provider authentication and payment handoffs MAY return to the app after a paid C2PA publication job. Provider selection, supported profiles and prices remain implementation and commercial decisions. | U-83; FE-046 |

#### Metadata ownership and sharing privacy

| ID | Requirement | Related |
|---|---|---|
| FE-054 | The product MUST extract and inspect EXIF, IPTC, XMP and format-specific video metadata locally, and MUST allow correction of chronology, tags, location and owner-authored descriptive metadata with single and batch edits and supported search. | U-66, U-55 |
| FE-055 | Original observations, owner assertions and unknown extracted fields MUST be preserved separately in versioned, resource-bound OCFL sidecars. Owner-authored descriptive statements MUST use `dcterms:` keys, confirmed `schema:` overflow, or terms in a registered extension namespace. Export, reimport and recovery MUST preserve their identity and history. | U-66, U-55 |
| FE-056 | Scrubbing MUST be enabled or disabled by the user, with selected retained fields and clear outgoing disclosure covering `exif:`, `dcterms:`, `schema:` and registered extension-namespace sidecars. An empty retained-field list in enabled mode MUST give full EXIF removal of ordinary metadata. A disabled setting MUST remain visibly unscrubbed. | U-66, U-66 |
| FE-057 | Professional publish MUST offer the provenance/AI-mark retain-class. The product MUST verify actual outgoing files, including the paid publisher's input and final output, and on professional AI-disclosure routes the visible label and retained marks. | U-66; C-014 |
| FE-068 | Owner-authored descriptive metadata MUST use ISO 15836-2 `/terms/` properties (`dcterms:`), confirmed Schema.org overflow, or terms in a registered extension namespace. Create, read, update and delete MUST match the EXIF technical editor. The UI MUST list populated keys only (Markdown-style progressive disclosure). Unmatched intent MUST suggest the closest Schema.org term (`schema:`). Every displayed pair MUST show its prefix (`exif:`, `dcterms:`, `schema:` or the registered extension prefix). | U-55; C-043; T-038 |

#### Performance at collection scale

| ID | Requirement | Related |
|---|---|---|
| FE-058 | The free core MUST prioritize visible content and user actions, with progressive display-sized previews, bounded prefetch, reusable caches and adaptive background scheduling during background work. Long-running supported batch jobs MUST checkpoint and resume instead of restarting completed work. | U-11; T-009–T-032 |
| FE-059 | Out-of-process workers, last-window checkpoints, keep-running-after-quit, 4-core/16 GB/SSD default bounds and the 30-second restart bound remain U-26, U-27, U-28 and U-27. | U-26, U-27, U-28, U-27 |

#### Institutional interoperability

| ID | Requirement | Related |
|---|---|---|
| FE-060 | Later DAM/IIIF workflows MUST include additional digital-curation features for memory institutions, named collection-system and metadata profiles, bounded import, authorized publication, batch watermarking with per-item outcomes and an annual institutional service option. | U-39 |
| FE-061 | Publishing of AI-assisted restoration or other materially AI-manipulated collection items MUST include visible first-exposure disclosure (EU icons as the default labelling set), preserved originals and valid C2PA/provenance for delivered derivatives. | U-84, U-84 |
| FE-062 | Nonempty interoperability contracts and funded delivery gates MUST precede implementation. Private OCFL roots MUST NOT become public IIIF endpoints. | U-39 |
| FE-063 | Institutional Article 50 work MUST NOT become a prerequisite for the FileSquid consumer release. Institutional workflows are delivered as FileSquid modules, with the Arkrim distribution as their primary UI. | U-39; C-024, C-025 |
| FE-079 | Curation jobs (for example transcoding), Parquet projection and FAIR reporting MUST be FileSquid modules exposed through the API, with the Arkrim distribution as their primary UI. | U-39, U-85, U-86 |

#### Desktop host isolation

| ID | Requirement | Related |
|---|---|---|
| FE-064 | Desktop host isolation MUST satisfy the desktop user requirements: the API served in-process with no TCP listener unless an ADR records otherwise, deny-by-default authorisation of every API call, opaque handles, no raw paths or SQL in the UI, out-of-process media workers in the installer, and UI-visible errors without crashing the desktop host. | U-18, U-18, U-26, U-26 |

#### Platform, API and hosts

| ID | Requirement | Related |
|---|---|---|
| FE-074 | Every FileSquid feature MUST be exposed through one versioned FileSquid API served by every host. UIs MUST reach features only through that API. | U-16, U-17 |
| FE-075 | FileSquid MUST ship a desktop host (Tauri 2, API in-process) from the MVP, a multi-user `filesquid-server` host (Axum) from the first consumer release, and an encrypted browser host afterwards. | U-18, U-19, U-22 |
| FE-076 | FileSquid MUST support white-label distributions (FileSquid default, ImageSquid, Arkrim, third parties), each defined by views, design tokens, a label table, brand assets and a module set, without backend code. | U-24 |
| FE-077 | Capabilities that a distribution needs (for example media derivatives and editing recipes for ImageSquid, curation jobs and Parquet for Arkrim) MUST be implemented as FileSquid modules behind the API and enabled per distribution. | U-25 |
| FE-078 | FileSquid MUST offer paid ciphertext-only backup and sync servers at `*.filesquid.tech` immediately after the MVP, before the first consumer release and before the ImageSquid release. | U-21 |
| FE-080 | Every host MUST provide **live health monitoring** of the system and its processes through the FileSquid API: a process registry (every job, worker, import, upload, migration, scan, sync session and host task with state, progress and heartbeat; stalled detection), host checks (storage roots, free space, operational store, queue depths, error rate) and an overall `ok` / `degraded` / `failing` status with reasons, pushed live on the event channel. `@filesquid/ui` MUST ship FileSquid's own **Monitor** dashboard showing this, which every distribution can include. Only the owner (desktop) or administrators (`filesquid-server`) MAY see it. | U-87; T-056, T-057; [ADR 033](./architecture/033-activity-journal-and-live-health-monitoring.md) |
| FE-081 | FileSquid MUST record **100% of its activity** in an append-only activity journal in its operational store: every domain operation, job and process state change, authorisation refusal, credential issue, warning and error, on every host. The journal MUST be queryable and exportable (JSON Lines) through the API and the Monitor dashboard, with configurable retention (default: keep everything; archiving older segments as an OCFL object allowed). External telemetry stacks are optional OTLP sinks and MUST NOT be bundled into the desktop app. | U-88; T-055, T-056; [ADR 033](./architecture/033-activity-journal-and-live-health-monitoring.md) |

## User Requirements

IDs are sequential (`U-01`, `U-02`, …).

### Platform, ownership and access

| ID | Statement | Requirement |
|---|---|---|
| U-01 | I want to browse and manage my files like a familiar cloud drive. | FileSquid MUST ship a default Drive-style web UI with navigation, breadcrumb, list and grid views, preview, upload, create folder, rename or move, remove, restore, and a sidebar with Details, Versions, Metadata and Content Credentials panels in named slots. Every file operation that changes content or organisation MUST produce a new OCFL version. |
| U-02 | I need my files to remain mine and usable offline, and I want to decide whether to pay after using the app. | No up-front subscription. The desktop app MUST work fully offline with no account. Originals MUST stay immutable, byte-exact and owner-readable on disk, with open import and independent reconstruction and export, and explicit availability limits for referenced or encrypted files. Free local access and export MUST survive cancellation of any paid service. Local capacity MUST be limited by hardware, not by a paywall. |
| U-03 | I want the files on disk to be the real source, so I can rebuild the app if its catalog or indexes are corrupted or removed. | OCFL objects MUST be the canonical data source for accepted state: on the owner's disk for the desktop, on the server's storage root for `filesquid-server`, and in the encrypted hosted OCFL root for the encrypted browser host. Catalogs, indexes and other application state MUST be fully rebuildable from those objects and MUST NOT be the only copy of accepted user data. If they are corrupted or removed, the user MUST have the option to rebuild the application layer in full from the OCFL objects. |
| U-04 | I want the same names everywhere for an installation, my files, a storage group, a saved view and a public exhibit, and I want the engine to treat every item as a file. | Internal names MUST be repository, library, collection, virtual collection, virtual exhibit and file, with the meanings and cardinality in [Terminology](#terminology). Internal code, the PRD, the SRS and architecture documents MUST use those names. The item-level type MUST be **file** at all backend, schema and API levels; Photo, Image or Video MUST NOT be minted as domain types. The default FileSquid UI MUST use the internal names; a distribution MAY replace them through its label table. |
| U-05 | I want more than one library when my work should stay separate, as many collections as I need, and saved views that gather files from several collections without copying them. | A user or group MAY own more than one library on a repository. A library MAY contain many collections. A collection belongs to exactly one library. A file's bytes MUST live in exactly one collection. A virtual collection MUST be a query, not a second store, and MUST offer the same UI features as a physical collection. |
| U-06 | I need several people to use one FileSquid server, and to understand who can see my files or my group's files. | `filesquid-server` MUST support multiple users and groups, personal and group-owned libraries, and collection-level access. Key custody, private accounts, shared-library membership, sharing authorisation and activity controls MUST be explicit. Group membership MUST NOT grant ownership of another member's libraries. Billing administration MUST grant no access to private libraries. |
| U-07 | I want to curate a public exhibit of files that are already public, without copying them into a second store. | A virtual exhibit belongs to a user on a repository. It MAY span collections in different libraries on that repository and public collections on other repositories. Every member MUST be marked for public access. Hidden files and private collections MUST NOT appear. A virtual exhibit is a query, not a second store, and MUST offer the same UI features as a physical collection. Social-feed posts become files in exactly one home collection on import, then MAY join an exhibit. Acquisition HTTP evidence, credentials and cookies MUST remain in the private evidence envelope. Hosted public exhibit delivery MUST remain distinct from the private per-user hosted root. |
| U-08 | I want to know what moved and what did not when I import or migrate. | Imports and migrations MUST produce durable inventories with verification, per-item outcomes, preservation limits, reconciliation and retry. |
| U-09 | I want backup to rescue mistakes that synchronisation has propagated. | Encrypted backup MUST keep independent retained recovery points with rehearsed restores. Backup and sync MUST be separate guarantees; sync MUST NOT be presented as backup. |
| U-10 | I want my private archive to survive software, provider and account changes. | Storage roots MUST be per user (OCFL), with complete collection and key recovery, declared retention and independently usable export. Export and clean-device recovery MUST NOT depend on a live service catalog. |
| U-11 | I want a large collection of large files to feel as quick to browse and present as a small one, even while the app is processing other work. | High performance is mandatory in the free core: prompt useful previews, responsive controls, smooth continuous navigation and presentation, bounded nearby prefetch and caching, fair task scheduling and durable batch resumption, verified on declared scale and device profiles for every host and distribution. |
| U-12 | I want costs and cancellation to be predictable. | Paid services MUST publish quotas, retention and export windows, on sustainable service terms. |
| U-13 | I need confidence that FileSquid will remain maintainable and portable. | Formats MUST be published, licensing MUST be deliberately open source, the code MUST be modular, and releases MUST be verified. |
| U-14 | I want familiar workflows on each device without false promises. | FileSquid MUST publish a platform capability matrix, show offline and availability states, and offer accessible alternatives where a platform cannot support a feature. |
| U-15 | I want one platform that can preserve and retrieve any file type, not only photographs and video. | FileSquid MUST upload, store and give access to any file type. Exact-byte preservation of unknown types is required; a working viewer, editor or metadata mapper for every format is not, and interpretation limits MUST be disclosed. |

### One API, hosts and distributions

| ID | Statement | Requirement |
|---|---|---|
| U-16 | As a UI developer, I want one API for every FileSquid feature, and desktop and server to use the same operation contracts. | Every feature MUST be reachable through one versioned FileSquid API. UIs MUST NOT call modules, the filesystem or the database directly. Versioned operation schemas and errors MUST be owned by `filesquid-api` and published as a contract; TypeScript types MUST be generated from it, with compatibility tests so hosts and UIs cannot drift. |
| U-17 | I want live progress and change updates without refreshing. | The API MUST provide an event channel for job progress and change notifications. HTTP and WebSocket transports MUST be evaluated and the result recorded in an ADR. |
| U-18 | I do not want the desktop app to open a network port, hold my disk paths or database queries, or grant privileges before a call is explicitly allowed. | The desktop host MUST serve the API in-process without a TCP listener, unless an ADR records otherwise. The UI MUST receive opaque handles, never raw filesystem paths or SQL. Every API call MUST be authorised before files, SQL or keys are touched; access MUST be deny-by-default. Native file access, workers and OS adapters MUST stay out of the untrusted webview. |
| U-19 | I want to run FileSquid on my own server for my family or organisation. | `filesquid-server` MUST serve the API and a selected distribution's UI from one process, using local or S3-compatible storage. |
| U-20 | I want to know who can read my files on each host, and I want private meaning never compiled into the encrypted hosted service. | Each host MUST state its privacy boundary in the UI: desktop (the owner's device only), self-run server (that server's operator), encrypted hosting (ciphertext only; the operator holds no keys). The ciphertext-only hosted service MUST NOT link private application logic, module processing of private content, or native media processing. |
| U-21 | I want encrypted backup and sync without running a server. | FileSquid MUST offer paid encrypted backup and sync at `*.filesquid.tech`. The service MUST store ciphertext only and MUST NOT hold library keys or private plaintext. The same account model MUST scale from personal to institutional quotas. |
| U-22 | I want to open the encrypted backup my desktop sealed, in a browser, when every desktop is off. | FileSquid MUST provide an encrypted browser host that runs the FileSquid modules as WebAssembly against the encrypted hosted root. Native Rust, WASM and WebCrypto MUST share matching test vectors for SHA-256, AES-256-GCM, HKDF, Ed25519, X25519 and Argon2id. Drift is a defect. |
| U-23 | I want the browser host to be honest about what it cannot do. | The WASM graph MUST NOT depend on native processing. It MUST include the `filesquid-ocfl` and `filesquid-metadata` modules working against the encrypted hosted OCFL root. Compiling to WASM MUST NOT be treated as providing a local-disk OCFL root, libvips, FFmpeg or PhotoKit. |
| U-24 | As a distribution builder, I want to ship a branded product for my audience without writing backend code. | A distribution MUST be definable by design tokens (light and dark), a label table keyed by internal names, brand assets, view modules and a module set. A distribution MUST NOT contain backend code. Panels MUST plug into named sidebar slots and MUST NOT depend on each other. Missing tokens or labels MUST fail a test. |
| U-25 | As a distribution builder, I want photo, archive and other capabilities available to any distribution. | Every capability a distribution needs MUST be built as a FileSquid module behind the API, and MUST be switchable per distribution. |
| U-26 | I want a corrupt or huge file not to kill the application, and failures explained in the app rather than a crashed window. | Native decode, FFmpeg and libvips MUST run in out-of-process workers shipped in the installer, so a decoder crash cannot take down the catalog or UI. Module and API boundaries MUST return errors the UI can show. Hosts MUST NOT crash on a failed request. |
| U-27 | I want closing the window to keep import progress, and no hidden work after I quit unless I asked; I want restart to resume quickly without redoing finished work. | Closing the last window MUST NOT destroy job checkpoints. Keep-running after quit MUST be an explicit setting. Default: checkpoint and resume on next launch; heavy decode or upload MUST NOT continue silently after quit. Ordinary restart MUST reach a resumable job state within 30 seconds without rescanning completed files. |
| U-28 | I want declared limits so a large import cannot take over the machine. | Declared 4-core / 16 GB / SSD desktop profile defaults: at most 4 active transfers; at most 2 CPU-heavy workers; 10 GiB staging cap; migration process memory at most 2 GiB excluding separately budgeted ML packs. |
| U-29 | I want one systems language and one UI language, not a third backend. | Rust for backend logic and TypeScript (no UI framework) for every distribution's UI. Python, Elixir, Phoenix, FastAPI, Django or another application backend MUST NOT be added. |
| U-30 | I want a hosted build that pulled in private processing or native media tools to fail CI. | CI MUST fail the build of the ciphertext-only hosted service if libvips, FFmpeg, or module code that processes private plaintext appears in its dependency graph. |
| U-31 | I do not want a compiler feature flag to be the only privacy wall. | Cargo features MUST NOT be the only privacy boundary; they are additive capabilities, not a switch that can enable private processing on the hosted service. |
| U-32 | I want the hosted services not to freeze while decoding media. | Hosted Tokio runtimes MUST NOT run blocking native media work. |
| U-33 | I want a default installer whose media stack is licence-reviewed. | The default public installer MUST remain LGPL-FFmpeg-clean (`--enable-gpl` and `--enable-nonfree` forbidden). The whole bundled native inventory MUST pass licence and patent review before a public desktop binary. FileSquid's own licence MUST NOT be treated as covering that inventory. |
| U-34 | I want a normal per-OS install that already includes the media tools. | Per-OS installers (macOS first, then Linux and Windows). A single binary for all operating systems is not required. Bundled native libraries MUST ship in the installer; the product MUST NOT require ImageMagick or FFmpeg on PATH. |
| U-35 | I do not want a second background-job product beside FileSquid's own jobs. | Celery, Oban or another second job stack MUST NOT be added beside durable Rust jobs. |
| U-36 | I want the browser interface to stay responsive while crypto and replica work runs. | Expensive WASM MUST run on workers, incrementally, not on the UI thread. |
| U-37 | I want FileSquid to build on ordinary stable Rust. | Current stable Rust is preferred. A nightly toolchain MUST NOT be required. |
| U-38 | I want shipped libraries checked for licences and known vulnerabilities. | Release and CI MUST run licence and advisory checks (`cargo audit` / `cargo deny`) for crates.io dependencies and bundled C libraries. |
| U-39 | I need professional collection workflows that fit my institution's systems and standards. | DAM and IIIF interoperability and additional digital-curation features for memory institutions, named metadata and collection-system profiles, efficient batch publishing and an annual service option MUST be delivered as FileSquid modules (primarily through the Arkrim distribution), with demand, partnership, funding and independent interoperability gates. |

### OCFL module

| ID | Statement | Requirement |
|---|---|---|
| U-40 | As a preservation programme, I want to use the OCFL engine without the FileSquid app. | `filesquid-ocfl` MUST be installable and usable on its own as a FOSS library, with its commands in the `fsquid` CLI, and with a generic OCFL 1.1 API free of FileSquid application types. Collection snapshots, recipes, C2PA layout, catalog tables and dual-profile encryption MUST remain callers of that API. Existing community OCFL programs were insufficient as an embeddable engine; this module fills those gaps with crash-safe OCFL 1.1 version publication, S3-compatible storage and a later Parquet projection. |
| U-41 | As an archivist, I want what I store to be readable by other OCFL tools. | Emitted objects MUST validate with community OCFL validators and fixtures. Community tools (`rocfl`, `ocfl-py` and similar) MAY be CI oracles; they MUST NOT be the writer, a bundled sidecar or a server dependency. |
| U-42 | As an operator, I want a crash never to corrupt a version, and unpublished versions never treated as saved. | Writes MUST reserve `vN` exclusively before content (local lease or fenced S3 reservation), write the root inventory sidecar last, treat an incomplete `vN` as unpublished, and reconcile cleanly after a crash. S3 heads MUST use compare-and-swap. |
| U-43 | As a recovery user, I want every file of a version, including unchanged ones. | Logical state MUST be resolved from inventory `state` and `manifest`, never by directory names. |
| U-44 | As an integrator, I want local disks and object storage behind one API. | All byte access MUST go through one async, runtime-agnostic storage backend interface with local filesystem, S3-compatible and in-memory backends. Logical files MUST stream in bounded chunks; no API may need a whole object in memory. |
| U-45 | As a hosted operator, I want two writers never to overwrite each other. | On S3-compatible storage a writer MUST obtain a fenced `vN` reservation before uploading any content for that version, and head publication MUST then use compare-and-swap. A second writer MUST NOT upload into the same `vN` prefix; a writer that fails to reserve or loses the head race MUST rebase or report a conflict. |
| U-46 | As a browser developer, I want the same engine in the browser. | The crate MUST build for `wasm32-unknown-unknown`; browser storage MUST NOT be treated as a canonical root. |
| U-47 | As a data engineer, I want to build datasets from preserved objects. | The crate MUST enumerate committed logical state; a `parquet` feature (default off) MAY project it without changing the core API. |
| U-48 | As a security reviewer, I want encrypted packages handled like any others. | The engine MUST store opaque caller-supplied bytes and MUST NOT require plaintext. |
| U-49 | As an adopter, I want independent releases. | The crate MUST remain independently publishable under the MIT license; callers MUST pin published versions. |
| U-50 | I want my archive to stay valid OCFL, not a Git working tree. | A `.git` directory MUST NOT appear in an OCFL object root or storage root. Git is this project's version control, not the file store. |
| U-51 | As an integrator, I want to commit a changed folder without working out the delta myself. | Given a working state and a base version, the engine MUST compute the change set by digest (added, changed, renamed, deleted, reinstated). It MUST write only bytes whose digest is not already in the object's manifest, and MUST record renames, deletes and reinstatements in the new `state` without copying bytes. |
| U-52 | As a developer, I want git-style version operations from code, not only from a command line. | The library MUST expose stage, status, diff (staged against `head`, and between any two versions), commit, log, checkout, revert and reinstate as a documented public API. Each commit MUST create exactly one new OCFL version. Revert and reinstate MUST create a new version and MUST NOT modify earlier versions. Staging and checkout MUST NOT write inside the storage root. `fsquid` MUST use only this public API. |
| U-53 | As an operator, I want staged changes from two writers never to overwrite each other, on any platform. | Every commit MUST go through the `vN` reservation and head compare-and-swap on every backend. A commit whose base `head` is no longer current MUST rebase or report a conflict, and MUST NOT publish. Native builds, WebAssembly builds and the UI's version-producing actions MUST use this same change model. |

### Metadata module

| ID | Statement | Requirement |
|---|---|---|
| U-54 | As a developer, I want to read the metadata a file already carries without shelling out to another tool. | The module MUST extract supported embedded metadata from bytes or a reader, without a subprocess or filesystem path. Unknown formats MUST return an explicit unsupported result. |
| U-55 | As an archivist, I want descriptions to use a recognised standard, and to see which namespace each value belongs to. | Owner-authored descriptive statements MUST use the ISO 15836-2 `/terms/` register (`dcterms:`), confirmed `schema:` overflow, or a registered extension namespace; unknown `dcterms:` names and undeclared namespaces MUST be rejected. The UI MUST show prefixes (`exif:`, `dcterms:`, `schema:` or the registered extension prefix) and list populated keys only. |
| U-56 | As a cataloguer, I want to say things Dublin Core cannot. | The module MUST suggest the closest Schema.org term and MUST store it only after confirmation. Close matches MUST NOT silently alias. |
| U-57 | As an owner, I want my metadata in a file I can read without the software. | The canonical record MUST be a `metadata.md` file (YAML front matter plus Markdown body) in the `sidecar` and `ocfl` stores; the `sidecar` store names it `<filename>.metadata.md`, one per file, and FileSquid's `ocfl` store names it `<file-uuid>.metadata.md` beside the stored original, holding the original file name, extension, format, import facts and description. The `sqlite` store keeps rows instead and SHOULD export `metadata.md` on request. |
| U-58 | As a developer, I want to choose where records live. | Storage MUST be a pluggable interface; the core MUST NOT depend on a particular store. |
| U-59 | As a preservation programme, I want metadata versioned with the object. | The optional `ocfl` store MUST save `metadata.md` through `filesquid-ocfl`, so each edit is a new OCFL version. FileSquid MUST use the `ocfl` store as canonical. |
| U-60 | As a spreadsheet user, I want to edit records in a spreadsheet. | CSV import/export MUST use `prefix:term` headers and RFC 4180 quoting, and MUST reject unknown `dcterms:` columns and columns whose prefix is not a registered namespace. |
| U-61 | As an app developer, I want fast search without giving up the readable record. | An optional SQLite index MUST be rebuildable from the canonical store and MUST NOT be the only copy, except when the caller explicitly chooses the `sqlite` store. |
| U-62 | As a browser developer, I want the same metadata engine in the browser. | The crate MUST build for `wasm32-unknown-unknown`; the `fsquid` command logic and file-path helpers are native-only. |
| U-63 | As a scripter or spreadsheet user, I want a command line, and I want to use the metadata module without the FileSquid app. | Its commands `extract`, `get`, `set`, `export`, `import`, `namespace` (`register`, `list`) and `fsquid metadata validate` (which names every unregistered prefix), and CSV import/export, MUST be available in the `fsquid` CLI, usable without the FileSquid app. It MUST work without OCFL through pluggable stores (`sidecar`, optional `ocfl`, optional `sqlite`); with the `ocfl` store it MUST extract from and persist through `filesquid-ocfl`. |
| U-64 | As an adopter, I want no lock-in to one application. | The crate MUST NOT depend on a distribution, Axum, Tauri or SQLite in default features, and MUST remain independently publishable under the MIT license. |
| U-65 | As a developer, I want a ready-made metadata panel for my file UI. | FileSquid MUST ship a Metadata panel over the properties API, mountable in the default browser's sidebar or any distribution's view without depending on other panels. |
| U-66 | I want full private metadata, my own descriptive fields, and control over the metadata that accompanies a share; household scrubbing aggressive by default, professional publication keeping AI and provenance marks on purpose. | Local extraction and correction; single and batch `dcterms:` / `schema:` authoring and search; portable versioned OCFL sidecars. Scrubbing enabled or disabled with retained-field choices and verified outgoing disclosure, including sidecars. The default ordinary retain-list is empty; a named provenance/AI-mark retain-class exists; stripping marks happens only as an explicit, previewed policy, verified on the actual outgoing files. |

### C2PA module

| ID | Statement | Requirement |
|---|---|---|
| U-67 | As a developer, I want to add C2PA to my app with one library, without installing the FileSquid app. | `filesquid-c2pa` MUST provide read, validate, mint, edit, embed and report through a documented Rust API and `fsquid` commands (`read`, `verify`, `sign`, `edit`, `embed`, `report`). It MUST NOT require OCFL, distribution types or a specific signer or trust store. |
| U-68 | As a verifier, I want results I can explain to users. | Validation MUST report the specification's status codes per manifest and ingredient, and MUST report untrusted or unchecked items rather than hide them. |
| U-69 | As a security reviewer, I want proven cryptography. | Cryptographic primitives MUST come from established crates behind a crypto-backend trait and MUST NOT be hand-rolled. |
| U-70 | As an integrator, I want to decide whom to trust. | The crate MUST ship no implicit trust store; trust lists MUST be supplied by the caller. |
| U-71 | As a capture-tool maker, I want to sign with my own keys or service. | Signing MUST go through a caller-supplied `Signer` (local keys, hardware modules, remote services). The crate MUST NOT act as a certificate authority. |
| U-72 | As an editor, I want edits recorded without breaking existing credentials. | Edits MUST create new manifests referencing the prior asset as a parent ingredient; signed manifests MUST NOT change in place. |
| U-73 | As an adopter, I want my output to work with other C2PA tools. | Output and verdicts MUST match the official `c2pa` crate and `c2patool` on C2PA conformance files in CI; those tools MUST NOT be runtime dependencies. |
| U-74 | As a browser developer, I want verification in the browser. | The crate MUST build for `wasm32-unknown-unknown`; the core MUST use caller-supplied byte sources, sinks and fetchers. |
| U-75 | As an archivist, I want to audit a whole collection, and a free warning before I share AI-touched files without visible disclosure. | The crate MUST provide bounded, resumable scans and disclosure-safe reports. A free scan MUST check disclosable AI-action history against the selected share or publication route. Scans MUST NOT auto-publish. |
| U-76 | As an adopter, I want no lock-in. | Default dependencies MUST exclude `filesquid-ocfl`, distributions, Axum and Tauri; the crate MUST remain independently publishable under the MIT license. |
| U-77 | As a developer, I want a ready-made credentials panel for my file UI. | FileSquid MUST ship a Content Credentials panel, mountable in the default browser's sidebar or any distribution's view without depending on other panels. |
| U-78 | I want provenance to begin when I import files, follow FileSquid's actions and be inspectable across my collection. | From the first release every import is automatically tracked (verified OCFL object and version, SHA-256 digest, import facts in the file's metadata record) and described (extracted `dcterms:` / `schema:` statements, a new OCFL version per metadata edit), and every version and retained change carries PROV-O history (U-89). Where the `c2pa` module is enabled (second consumer release): C2PA records for new imports, honestly dated backfilled records for earlier files, linked immutable prior credentials, disclosure policy, and bounded collection scans with exportable reports and honest signing and trust status. |

### PROV module

| ID | Statement | Requirement |
|---|---|---|
| U-89 | As an owner or archivist, I want a standards-based history of everything that happened to every file and its metadata, kept with the files. | FileSquid MUST record every OCFL version and retained change as W3C PROV-O (activity, entities, agents, times, journal links) in the same OCFL commit, show it beside each version and let me export it; `filesquid-prov` MUST be usable without the FileSquid app. |

### Further modules

| ID | Statement | Requirement |
|---|---|---|
| U-79 | I want to improve an image or video without losing the original. | A media module MUST provide derivatives (thumbnails, display sizes) and versioned non-destructive edits with faithful export. A preview MUST NOT be treated as an original. |
| U-80 | I want saved web files to retain the complete evidence available about their source. | User-invoked capture, full recorded HTTP exchanges, attributed source claims and PROV-O sidecars in encrypted preservation packages. |
| U-81 | I want to select my own AI model and use the account or runner I already have. | Per-task capability-aware connections, OpenAI-compatible APIs, supported authentication and direct user-to-provider inference billing. |
| U-82 | I want to know when I am talking to an AI, not a person. | First-interaction AI disclosure for a collection assistant and prompt-assisted model conversations, unless obvious to a reasonably well-informed person; accessible at first exposure. |
| U-83 | I want to purchase C2PA publication and watermarking when I need it. | Required third-party publisher integration with scoped disclosure, watermark and manifest validation, transparent billing and retained artifacts. |
| U-84 | I want to publish AI-generated or materially AI-manipulated files with a visible first-exposure disclosure using the EU AI-content icons, and durable machine-readable marks, without buying a legal opinion. | Professional publication profile on paid C2PA/watermarking: a human-perceivable label at first exposure, with the EU icons for labelling AI-generated content as the default visible set (another accessible visible or audible form MAY be added; icons MUST NOT be replaced by C2PA, watermarks or tool-only badges); retained generator and C2PA marks; outgoing-file verification. Household private use stays unlabelled by default. This is not an EU AI Act conformity assessment. |
| U-85 | As an archivist, I want versioned batch curation jobs on my collection. | Curation jobs (for example transcoding) MUST run as durable, resumable jobs whose results are committed as new OCFL versions, never silent rewrites of accepted state. |
| U-86 | As an archivist, I want FAIR datasets and reports from my collection. | Parquet projection (through the `ocfl` module's `parquet` feature) and FAIR reporting MUST be exposed through the API, with the Arkrim distribution as their primary UI. |

### Administration and monitoring

| ID | Statement | Requirement |
|---|---|---|
| U-87 | As an owner or administrator, I want to see live what FileSquid is doing and whether the system and its processes are healthy, inside the app, without installing a monitoring stack. | Every host MUST show live health and the state of every FileSquid process (jobs, imports, uploads, scans, sync) in FileSquid's own Monitor dashboard, served by the FileSquid API, on desktop as well as on the server. |
| U-88 | As an owner or administrator, I want a complete record of everything FileSquid did, on every host. | FileSquid MUST log 100% of its activity in its activity journal, never dropping events, and MUST let me search, browse (a Logs view) and export it (JSON Lines or CSV). The journal MUST be recoverable from disk: a committed operation's entries in the same OCFL commit as the change, all other entries sealed hourly and on close into journal OCFL objects, rebuilt with the library; at most one hour of entries that changed nothing MAY be lost with the database. |

## Terminology

FileSquid and every distribution MUST share one internal vocabulary for durable product components. These names are **mandatory** in internal code, schemas, APIs, tests, `prd.md`, `srs.md`, `architecture.md` and architecture records. They are not optional synonyms. The FileSquid default UI uses them as its nouns. A distribution MAY replace them in its UI through its label table (for example ImageSquid's repo, album, gallery and exhibit, defined in the ImageSquid vision).

| Internal name | Meaning | Rules |
|---|---|---|
| **repository** | A single FileSquid installation | One installation is one repository |
| **library** | A named set of files owned by a single repository user, or by a group of users | A user or group MAY own more than one library on a repository. A repository MAY host libraries from many owners. A group-owned library is the group's library; membership MUST NOT automatically grant ownership of another member's libraries. |
| **collection** | A physical and logical grouping of files in a library. Every file in a collection shares one storage location and one access-permission set | A library MAY contain one or many collections. A collection belongs to exactly one library. A user MAY have one or many collections. A file's bytes live in exactly one collection. |
| **file** | A stored bytestream; the item in a collection | The item-level type MUST be **file** at all levels: backends, schemas, APIs, `prd.md`, `architecture.md` and code. Distributions MAY label a given file by media kind (for example photo, image or video); those labels are presentation, not a second object model. |
| **virtual collection** | A filtered query (rule-based) **or an explicit, hand-picked member list** over one or many physical collections the user can access in one library, presented as one comprehensive collection with the same UI features as a physical collection, including manual order and a cover | A user MAY have zero or many virtual collections. A virtual collection belongs to exactly one library. A virtual collection MUST NOT be a second storage location. |
| **virtual exhibit** | A public curated query over collections whose files are marked for public access. It MAY span collections in different libraries on one repository and public collections on other repositories | A virtual exhibit belongs to a user on a repository. A user MAY have zero or many virtual exhibits. Every member MUST be public. Hidden files and private collections MUST NOT appear. A virtual exhibit MUST NOT be a second storage location. Social-feed posts become files in exactly one home collection on import, then MAY join an exhibit. Acquisition HTTP evidence MUST remain private. |

A **distribution** is a UI layer (views, design tokens, label table, brand assets and a module set) over the FileSquid API. A **module** is a FileSquid Rust crate that provides a capability behind the API. A **host** is the process that serves the API: the desktop app, `filesquid-server`, or the encrypted browser host.

Homonyms that remain in other senses and MUST be distinguished in prose:

- Git / this project's source-control repository is not a product repository.
- A software crate, including the `filesquid-ocfl` module, is not a product library.
- GLAM "Galleries" names the institutional sector; it is not a gallery in any distribution.
- A physical museum exhibition hang is not a virtual exhibit.
- An **OCFL snapshot** (also **collection snapshot**) is the versioned OCFL object that records a collection's organization (order, relationships, pointers to file-object heads). Owners copy a completed snapshot with ordinary file tools. It is not a ZFS/APFS copy-on-write snapshot.
- A job **checkpoint** is durable in-progress job state (import, upload, migration). It is not an OCFL snapshot.
- SQLite WAL **checkpoint** remains SQLite's term for flushing the write-ahead log.
- External "collections-management systems" keep their ordinary professional meaning.
- A product **file** is the item. Sidecars, inventories, lockfiles and source-code files remain ordinary files in other senses. An OCFL object is the preservation package for a file, not a different item type.

**Downstream documents.** `prd.md` MUST allocate these rules to functional, data-model and technical requirements using the internal names, and MUST record a distribution's UI labels only where that distribution's user story or usability NFR describes a screen. `architecture.md` MUST use the internal names for components, data and storage. `srs.md` MUST require each distribution's labels on that distribution's user-visible surfaces, while keeping the internal names in system, data and traceability text.

## Technical Stack

> | Layer | Choice |
> |---|---|
> | Backend language | Rust (stable, edition 2021), one Cargo workspace in `apps/filesquid/` |
> | Modules | `filesquid-core`, `-ocfl`, `-metadata`, `-c2pa`; later media, import, clipping, AI connections, jobs, sync |
> | API | `filesquid-api`: one versioned contract (OpenAPI 3.1 candidate) with generated TypeScript types; transport HTTP plus an event channel, WebSocket under evaluation |
> | Desktop host | Tauri 2; API served in-process via a custom URI scheme; no TCP port |
> | Server host | Axum on Tokio; multi-user; serves the API and the selected UI as static files |
> | UI | Vanilla TypeScript (no UI framework), standard custom elements, Vite build; design tokens as CSS custom properties; label tables in JSON |
> | Canonical store | OCFL 1.1 on the local filesystem or S3-compatible object storage |
> | Operational data | SQLite on desktop (rebuildable); PostgreSQL for multi-user servers and hosted services (rebuildable or operational only) |
> | Hosted service | `*.filesquid.tech`: ciphertext-only backup and sync on S3-compatible storage; providers and regions not yet chosen; each Drive's backup keeps copies in more than one geography; the Drive owner enrolls a device key for uploads into that Drive |
> | Native media (later) | libvips, LibRaw, libheif, LGPL FFmpeg in out-of-process workers |
> | Crypto | AES-256-GCM, HKDF-SHA-256, SHA-256, Ed25519/X25519, Argon2id, with matching native, WASM and WebCrypto vectors |
> | Quality | rustfmt, Clippy, ESLint, Prettier; `cargo test`, Vitest, Playwright, cucumber BDD; Allure/JUnit reports; `cargo deny` |
> | Logs and monitoring | Unbuffered stdout JSON; FileSquid's own activity journal (100% of activity, in the operational store), process registry and Monitor dashboard on every host; optional OTLP export to sinks such as VictoriaLogs and VictoriaMetrics; no Grafana, Loki, Prometheus or Redpanda ([ADR 033](./architecture/033-activity-journal-and-live-health-monitoring.md)) |
> | Licence | MIT for the whole platform: modules, API, hosts, `@filesquid/ui` and the FileSquid and ImageSquid distributions |
>
> Not used: Svelte, React or other UI frameworks; Electron; Python, Elixir or other backend languages; Git or community OCFL CLIs as the store.

## Technical requirements

`T-###` IDs; do not reuse a number. Next free ID is T-059. Live stack choices remain the ADRs in [resources/architecture/](./architecture/README.md).

### Client processing and hosted mailbox

| ID | Requirement | Related |
|---|---|---|
| T-001 | Private-library processing MUST default to authorized endpoints: the desktop host, a `filesquid-server` that the owner or their organisation operates, or the encrypted browser host. The ciphertext-only hosted service MUST NOT process private library content except as T-003 authorises. | U-20; [ADR 032](./architecture/032-hosting-and-privacy-model.md) |
| T-002 | Browser access to the encrypted hosted root MUST work when every desktop is off, using browser-side decryption: the encrypted browser host reads and commits the hosted OCFL root through the WASM build of `filesquid-ocfl`. | U-22; ADR 008, 009, 025, 032 |
| T-003 | The owner MAY authorize selected inputs to their own AI provider or the paid C2PA publisher, and MAY save scoped permissions for recurring jobs. Those boundaries MUST NOT grant ordinary backup operators access to library keys or unrelated private content. | U-81; F-044; F-045; FE-067 |
| T-004 | A desktop MAY generate advanced derivatives for later browser use. Unavailable work MUST have a visible unsupported or waiting state. | FE-017 |

### Canonical OCFL state and evidence

| ID | Requirement | Related |
|---|---|---|
| T-005 | A completed authoritative change MUST produce durable OCFL asset versions and a consistent collection snapshot. A database write alone is not sufficient. | U-03; ADR 005, 010 |
| T-006 | Sensitive HTTP evidence MUST be stored as separate encrypted resources. | F-041; ADR 009 |
| T-007 | Essential state and independently unlockable recovery envelopes MUST survive loss of the operational database. | U-10, U-03 |
| T-008 | HTTP evidence recording MUST use a capability-validated recorder. FileSquid MUST NOT assume an ordinary browser extension exposes every body and header. The recorder binary remains a later implementation ADR. | F-041 |
| T-035 | Changes MUST remain visibly pending until their authoritative files and a consistent OCFL snapshot are durable. | T-005, T-029 |
| T-036 | Direct inspection and copy MUST NOT authorize arbitrary in-place edits of OCFL inventories or signed originals. | FE-037, FE-065 |
| T-037 | Ephemeral cache generation MUST NOT become permanent PROV-O history unless the output is retained or disclosed. | U-78, U-89 |
| T-058 | The PROV-O record of an OCFL version MUST be the logical file `provenance/<version>.jsonld` of the same object, written in the same commit as the change (an activity that changes no object, a repair and a backfill use a provenance-only version), in the versioned profile `filesquid-prov/1` with an embedded context and the PROV namespace `http://www.w3.org/ns/prov#`. Every commit MUST set the OCFL version `user`, carry a structured `filesquid.operation.v1` message and emit an `ocfl.commit` journal entry; journal entries MUST carry the operation ID and object version. | U-89; FE-082; [ADR 040](./architecture/040-prov-o-history.md) |
| T-039 | `filesquid-ocfl` MUST resolve logical state through inventory `state` and `manifest` at a caller-pinned object version, stream logical files, and enumerate committed logical state (object id, version, logical path, digest, content locator) without FileSquid catalog types. Directory-name inference MUST NOT be the resolver. | U-40; FE-069; [ADR 023](./architecture/023-filesquid-ocfl-engine-and-parquet.md) |
| T-040 | Each FileSquid packaged release MUST declare the exact `filesquid-ocfl` crate version in its lockfile and release notes. | FE-069; [ADR 023](./architecture/023-filesquid-ocfl-engine-and-parquet.md) |
| T-041 | The later `parquet` Cargo feature MUST project through the T-039 enumerator to a caller-chosen analytical location using Apache Arrow record batches. Default engine dependencies MUST remain free of Arrow, Parquet, DuckDB and Iceberg. | C-045, C-046; [ADR 023](./architecture/023-filesquid-ocfl-engine-and-parquet.md) |
| T-042 | `filesquid-ocfl` MUST be designed as its own tool for independent install (FOSS library, with its commands in the `fsquid` CLI), including by digital preservation programmes that never run FileSquid. The crate MUST build and publish without FileSquid application crates. It MAY carry `publish = false` until its API is versioned; after that, crates.io publication SHOULD be the distribution and FileSquid hosts MUST pin the version they link. Independent crates.io distribution and the `fsquid` CLI remain the product intent and MUST NOT be designed away. | U-40; C-047, C-057; [ADR 023](./architecture/023-filesquid-ocfl-engine-and-parquet.md) |
| T-043 | FileSquid hosts MUST compile the `filesquid-ocfl` library into their native graph and the browser WASM bundle. FileSquid MUST NOT shell out to `fsquid` or download a separate OCFL executable for canonical writes. GitHub Release `fsquid` binaries are for independent install by preservation programmes. | T-040, T-042; [ADR 016](./architecture/016-rust-ocfl-library.md) |
| T-044 | `filesquid-metadata` MUST extract metadata from logical files stored through `filesquid-ocfl`, including EXIF, ID3, XMP and other supported embedded packets, map them onto `dcterms:` or `schema:`, persist namespaced statements through that same API, and expose create/enhance operations for Dublin Core `/terms/` and Schema.org overflow. It MUST be designed as its own tool. It MAY carry `publish = false` until its API is versioned; after that, crates.io publication SHOULD be the distribution. FileSquid MUST NOT shell out to a metadata CLI for canonical writes. | U-63; C-048; FE-070; [ADR 024](./architecture/024-filesquid-metadata-engine.md) |
| T-045 | The stand-alone UI of `filesquid-metadata` MUST be its commands in the `fsquid` CLI: `extract`, `get`, `set`, `export`, `import`, `namespace` (`register`, `list`), and `fsquid metadata validate` (which names every unregistered prefix). CSV import and export MUST be available in Foundation (one row per object; headers `prefix:term`). XMP sidecar read/write MUST be available with CSV. Spreadsheet users and scripts MUST be able to use these commands without the FileSquid app. | U-63; C-049, C-057; FE-070 |
| T-046 | Whenever FileSquid or a distribution stores metadata in an OCFL object, the default shape MUST be a Markdown logical file with YAML front matter (Obsidian-style namespaced properties). JSON, XML, XMP, or CSV MAY be written into an OCFL object only when a named requirement or integration profile explicitly requires that format. SQLite, PostgreSQL, Elasticsearch and other catalogs MUST be rebuildable from the Markdown file plus the OCFL object. They MUST NOT be the only copy of owner-authored metadata. | U-03, U-63; FE-070; [ADR 024](./architecture/024-filesquid-metadata-engine.md) |
| T-047 | Newly generated virtual exhibit identifiers MUST be RFC 4122 / RFC 9562 UUID version 4. A virtual exhibit belongs to a user on a repository. Cross-repository members MUST be public references with explicit availability ([FE-003](#independent-library)). Hosted public exhibit delivery MUST remain distinct from the private per-user mailbox root ([FE-034](#portable-preservation)). Acquisition HTTP evidence MUST remain under T-006. | U-07; FE-071; C-050; [ADR 021](./architecture/021-uuid-v4-identifiers.md) |

### Descriptive metadata

| ID | Requirement | Related |
|---|---|---|
| T-038 | `filesquid-metadata` MUST ship the ISO 15836-2 `/terms/` register and the 1:1 `dcterms:`↔Schema.org map. Unknown `dcterms:` local names MUST be rejected. Collection-domain `accrual*` terms MUST NOT be authored on a file. | U-55; FE-068; [ADR 024](./architecture/024-filesquid-metadata-engine.md) |

### Performance acceptance

| ID | Requirement | Related |
|---|---|---|
| T-009 | Accepted stack ADRs record implementation direction. That acceptance MUST NOT be treated as a U-11 performance pass. Framework reputation or language choice alone MUST NOT satisfy U-11. | U-11; ADR 001–016 |
| T-010 | Representative measurements of interaction, media readiness, throughput and resource use MUST pass on packaged desktop builds, `filesquid-server`, and the encrypted browser host once it ships, before a release or performance-gate acceptance. | U-11 |
| T-011 | Prototypes MUST exercise large catalogs, high decoded-image sizes, mixed media and concurrent work on the supported device profiles, beginning with macOS desktop, then `filesquid-server`. | U-11 |
| T-012 | Exact cache algorithms and acceleration mechanisms MAY remain later architectural decisions. | ADR 007 |

### Interactive rendering

| ID | Requirement | Related |
|---|---|---|
| T-013 | The UI MUST use a bounded visible item window and indexed or paged collection queries. It MUST NOT load or render an entire library to show a view. | U-11; FE-005 |
| T-014 | Views MUST preserve stable ordering, selection, keyboard focus and accessible semantics. | U-11 |
| T-015 | The application MUST generate appropriate thumbnail and display sizes on demand and refine available previews progressively. Supported scaled or tiled decoding and hardware acceleration MAY be used when measurements justify them. | U-11; T-012 |
| T-016 | Full-resolution originals MUST remain available for operations that require them. A preview is not an original. | U-02 |

### Prefetch

| ID | Requirement | Related |
|---|---|---|
| T-017 | The application MUST prefetch a bounded window before and after the current view and upcoming slideshow items, adjusting to direction, speed, available memory and network conditions. | U-11; FE-024 |
| T-018 | It MUST cancel obsolete speculative work after jumps or direction changes without discarding work still needed by the active view. Rapid scrolling MUST NOT create an unbounded queue of stale requests. | U-11 |

### Caching

| ID | Requirement | Related |
|---|---|---|
| T-019 | The application MUST cache thumbnails, display derivatives and suitable view or search results with finite memory and disk budgets. It MAY reuse safely prepared or decoded nearby images where beneficial. | U-11 |
| T-020 | Cache keys and invalidation MUST account for media and edit versions, view parameters and access changes. Stale or newly unauthorized content MUST NOT be displayed. | U-06, U-11 |
| T-021 | Application-controlled cache eviction MUST preserve originals, authoritative metadata and history, and explicitly retained offline content, with clear capacity controls. | U-03 |
| T-022 | Browser storage remains subject to platform quotas and eviction. It is a cache, never canonical. Cache loss MUST produce clear availability and recovery states. | ADR 008, 025 |

### Work isolation and scheduling

| ID | Requirement | Related |
|---|---|---|
| T-023 | Expensive decoding, hashing, indexing and other processing MUST be isolated from interactive rendering. | U-11, U-26 |
| T-024 | Pending work MUST use bounded queues, MUST deduplicate equivalent requests, and MUST prioritize current interaction and visible content above speculative processing. | U-11 |
| T-025 | CPU, GPU, storage and transfer admission MUST adapt to measured capacity, memory and free-space pressure, and available battery, thermal and network signals. | U-28 |
| T-026 | Background jobs MUST receive fair service or an explicit paused state. | U-11 |

### Durable jobs

| ID | Requirement | Related |
|---|---|---|
| T-027 | Imports, uploads, migrations and other supported lengthy batch jobs MUST persist verified progress and resume unresolved work after interruption. | U-27, U-27; ADR 007 |
| T-028 | Resume MUST revalidate source identity and permissions. Within-resource checkpoints MUST be used where the route supports them. Where offset resume is unsupported, the job MAY replay only the affected unit, with bounded retries and visible status. | ADR 007; FE-030 |
| T-029 | Processing completion, media verification and preservation protection MUST remain separate states. | T-005 |

### Release measurement

| ID | Requirement | Related |
|---|---|---|
| T-030 | Release checks MUST measure immediate feedback separately from useful-image readiness, slideshow timing, search completion and bulk throughput, including cold start, warm cache, cache eviction or missing previews, random navigation and simultaneous background work. | U-11 |
| T-031 | Numeric budgets and supported workload and device envelopes MUST be approved before acceptance. Performance regressions MUST block the affected release until resolved or explicitly reviewed. A spinner alone MUST NOT meet useful-media readiness. | U-11 |
| T-032 | Packaged-artifact measurements MUST include the declared 4-core / 16 GB / SSD envelope (U-28), worker-crash isolation (U-26) and the 30-second restart bound (U-27). | U-11, U-26, U-28, U-27 |

### Later gates

| ID | Requirement | Related |
|---|---|---|
| T-033 | FileSquid MUST NOT import a generic server or AI stack solely because it appears in the repository template. | ADR 001, 013 |
| T-034 | The following remain later ADRs or commercial gates and MUST NOT be treated as present in the accepted stack: named cloud providers and the geographies of each Drive's backup copies; payment processor; F-044 publisher; ONNX or model packs; HTTP recorder binary; job SQL columns; SPA-asset signing; independent crypto audit before hosted pilot; exact crate patch versions (lockfiles). | ADR 009; T-008 |

### API and hosts

| ID | Requirement | Related |
|---|---|---|
| T-048 | The FileSquid API contract MUST be versioned, owned by `filesquid-api` and published in the repository; TypeScript types MUST be generated from it, and a test MUST fail when the contract and generated types drift. | FE-074; [ADR 030](./architecture/030-filesquid-api-and-transports.md) |
| T-049 | The desktop host MUST serve the API in-process through a Tauri custom URI scheme without opening a TCP port, unless an ADR records otherwise. | FE-064, FE-075; [ADR 030](./architecture/030-filesquid-api-and-transports.md) |
| T-050 | The API MUST provide an event channel for push updates. HTTP and WebSocket transports MUST be evaluated against preview and ranged reads, uploads, push events, the no-TCP-port desktop rule and Service Worker interception for the encrypted browser host, and the outcome recorded in an ADR. | FE-074; [ADR 030](./architecture/030-filesquid-api-and-transports.md) |
| T-051 | Every distribution UI MUST be vanilla TypeScript with standard custom elements and no UI framework, styled only through design tokens (CSS custom properties, light and dark) and worded only through a label table keyed by internal names. | FE-073, FE-076; [ADR 031](./architecture/031-vanilla-typescript-ui-and-white-label-theming.md) |
| T-052 | FileSquid MUST be one Cargo workspace of module crates. A module crate MUST NOT take a default dependency on another module crate; an optional one is allowed, as the `metadata` module's optional `ocfl` store and the `prov` module's optional `ocfl` feature ([ADR 040](./architecture/040-prov-o-history.md)), and each MUST build and test on its own. | FE-069, FE-070, FE-072; [ADR 029](./architecture/029-filesquid-unified-product-and-distributions.md) |
| T-053 | `filesquid-server` MUST authenticate users, enforce group and collection permissions on every API call, and keep operational data in a rebuildable or operational-only store. | FE-022, FE-075; [ADR 032](./architecture/032-hosting-and-privacy-model.md) |
| T-054 | The `*.filesquid.tech` hosted service MUST store and relay ciphertext only, authorise keys and fence OCFL head publication, and MUST NOT parse OCFL or hold library keys. | FE-078; [ADR 004](./architecture/004-axum-hosted-ciphertext-mailbox.md), [ADR 032](./architecture/032-hosting-and-privacy-model.md) |
| T-055 | The activity journal MUST be written by a `tracing` layer and the domain services into an append-only table in the operational store (SQLite on desktop, PostgreSQL on servers). Domain operations MUST be journaled in the same transaction as the change, or before it is acknowledged; other events MUST use a bounded queue with backpressure and MUST NOT be dropped. Hosts MUST NOT write text log files; stdout JSON is unchanged. Entries MUST be redacted as ADR 014 requires (opaque IDs; no file contents, titles, captions, pixels, keys or credentials), and a server administrator MUST NOT see another user's private metadata through the journal. | FE-081; [ADR 014](./architecture/014-stdout-observability.md), [ADR 033](./architecture/033-activity-journal-and-live-health-monitoring.md) |
| T-056 | Metrics MUST come from one set of counters and gauges, exposed as JSON through the admin API on every host and as Prometheus text at `/metrics` on `filesquid-server`. A host MAY export journal entries and metrics over OpenTelemetry (OTLP); export MUST be off by default and store-and-forward from the journal through a persisted cursor, so nothing is lost while the sink is down. No message broker is required. | FE-080, FE-081; [ADR 033](./architecture/033-activity-journal-and-live-health-monitoring.md) |
| T-057 | Every long-running process MUST register in the host's process registry with type, state, progress and a heartbeat, update it as it runs, and be reported as stalled when its heartbeat exceeds its declared timeout. Health MUST be derived from the registry and host checks and reported with reasons. | FE-080; [ADR 007](./architecture/007-durable-jobs-and-interactive-performance.md), [ADR 033](./architecture/033-activity-journal-and-live-health-monitoring.md) |

## Standards, Laws, and Regulations

| Area | Intended application and decision boundary |
|---|---|
| Requirements and architecture | RFC 2119 normative terms; ISO/IEC/IEEE 29148 traceable, verifiable requirements; stakeholder viewpoints guided by ISO/IEC/IEEE 42010. Template conformity alone does not establish certified compliance. |
| Time | RFC 3339 UTC operational timestamps; retain original capture values, offsets and unknown time zones separately. Do not manufacture a UTC capture instant from an uncertain date. |
| Photo/video metadata | EXIF, IPTC and XMP where supported; native video container/track/timed metadata with IPTC Video Metadata Hub mappings. Read, sidecar correction, native writing and privacy stripping require separate capability evidence; video has no single universal EXIF equivalent. [XMP](https://developer.adobe.com/xmp/docs/xmp-specifications/), [IPTC Video Metadata Hub](https://iptc.org/standards/video-metadata-hub/) |
| Descriptive metadata | Owner-authored descriptive keys MUST be ISO 15836-2 Dublin Core `/terms/` properties (`http://purl.org/dc/terms/`, prefix `dcterms:`). The `/elements/1.1/` namespace (`dc:`) is a read-time alias for the fifteen core terms. Unmatched descriptive intent uses the closest Schema.org term (`https://schema.org/`, prefix `schema:`). [DCMI Metadata Terms](https://www.dublincore.org/specifications/dublin-core/dcmi-terms/), [ISO 15836-2:2019](https://www.iso.org/standard/71341.html). Must be able to add [RiC-O](https://www.ica.org/standards/RiC/RiC-O_1-1.html) support for archivists in Arkrim distribution.|
| Metadata and media | Preserve EXIF/IPTC/XMP and opaque sidecars; map extracted IPTC/XMP descriptive facts onto `dcterms:` for the owner-facing record; publish supported codecs, ICC color profiles, bit depth, paired-resource and export contracts. |
| Preservation and provenance | OCFL 1.1 packages and a FileSquid encryption profile; W3C PROV-O represented in a versioned JSON-LD profile; C2PA import/action records and signed publication outputs with watermark/soft-binding support. Standards provide separate evidence functions, not a blanket truth certification. [OCFL](https://ocfl.io/1.1/spec/), [PROV-O](https://www.w3.org/TR/prov-o/), [C2PA](https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html). |
| HTTP capture | Preserve recorded request/response method, target, version, headers, context and bodies, with message relationships and explicit raw/decoded/truncated/unavailable status. Credentials retained in evidence require encryption and distinct disclosure controls. [HTTP semantics](https://www.rfc-editor.org/rfc/rfc9110.html). |
| Accessibility | Proposed WCAG 2.2 AA target for web interfaces, plus native keyboard, screen-reader, contrast and input testing. AI-interaction notices and professional AI-manipulation disclosures MUST meet this target at first exposure. [W3C recommendation](https://www.w3.org/TR/WCAG22/). |
| Privacy obligations | Determine launch jurisdictions, controller/processor roles, consent/other lawful bases, minimization, retention, export/deletion and incident duties before hosted pilots. GDPR applicability and biometric-analysis implications require qualified jurisdiction-specific review; encryption alone is not a compliance claim. Professional people/pets analysis notice (U-24) is a product behaviour, not a GDPR certificate. [European Commission principles](https://commission.europa.eu/law/law-topic/data-protection/information-business-and-organisations/principles-gdpr_en). |
| AI output transparency (EU) | Regulation (EU) 2024/1689 Article 50 is a **publication and disclosure use case** for FileSquid- or distribution-branded generative/manipulation features and for professional publish of AI-touched media used in the Union, including when FileSquid’s operator is established outside the Union and the output is used in the Union. It is not a claim that FileSquid is a GPAI provider, a high-risk biometric system, or a certified conformity-assessment body. Household personal use is outside deployer duties. Open-source licensing MUST NOT be treated as waiving Article 50 for in-scope systems. Visible first-exposure disclosure and machine-readable marking are separate duties. The default visible labelling set MUST be the [EU icons for labelling AI-generated content](https://digital-strategy.ec.europa.eu/en/policies/eu-icons-labelling-ai-generated-content). FileSquid MAY adhere to the [Commission Code of Practice on Transparency of AI-generated Content](https://digital-strategy.ec.europa.eu/en/policies/code-practice-ai-generated-content) for FileSquid- or distribution-branded generative features; if it does not, it MUST still be able to demonstrate how those features mark outputs. Signing the code MUST NOT be marketed as product certification. Content generated before 2 August 2026 NEED NOT be labelled retroactively; new professional publication of AI-generated or materially AI-manipulated work after that date remains in the disclosure-profile scope. [Regulation (EU) 2024/1689](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:32024R1689). Qualified legal assessment remains follow-up work. [EU AI Act analysis](./references/EU-AI-act-analysis.md). |
| Commercial and licensing | Review name/trademark clearance, open-source licenses, model/data/music rights, codec distribution, provider terms, consumer subscriptions, tax and physical-service obligations before affected releases. The entire bundled native inventory (libvips, FFmpeg, LibRaw, libheif and related) MUST pass license and patent review before a public desktop binary; the default installer MUST remain LGPL-FFmpeg-clean. Release and CI MUST run `cargo audit` / `cargo deny` for crates.io dependencies and bundled C libraries. |
| Preservation context | OCFL 1.1 and [community extensions](https://ocfl.github.io/extensions/); NAMASTE conformance declarations; ISO 14721 (OAIS) and ISO 16363 (trustworthy repositories) as the context FileSquid supports. |
| Provenance encodings | JUMBF (ISO/IEC 19566-5), [CBOR (RFC 8949)](https://www.rfc-editor.org/rfc/rfc8949), [COSE (RFC 9052)](https://www.rfc-editor.org/rfc/rfc9052), [X.509 (RFC 5280)](https://www.rfc-editor.org/rfc/rfc5280), [Time-Stamp Protocol (RFC 3161)](https://www.rfc-editor.org/rfc/rfc3161); C2PA conformance programme and trust lists ([overview](https://opensource.contentauthenticity.org/docs/conformance/)). The `c2pa` module pins its C2PA specification version. |
| Interchange | [RFC 4180](https://www.rfc-editor.org/rfc/rfc4180) CSV, [YAML 1.2](https://yaml.org/spec/1.2.2/) front matter, XMP (ISO 16684-1) sidecars, Exif (CIPA DC-008), ID3v2, IPTC Photo Metadata Standard. |
| Data and research | FAIR data principles and Apache Parquet for derived datasets (Arkrim outputs). |
| API and identifiers | [HTTP semantics (RFC 9110)](https://www.rfc-editor.org/rfc/rfc9110.html); WebSocket (RFC 6455) if adopted ([ADR 030](./architecture/030-filesquid-api-and-transports.md)); OpenAPI 3.1 for the API contract; RFC 9562 UUID version 4 identifiers. |
| Storage | S3-compatible object storage API without vendor-only features. |

## Value Propositions

> - **For individuals and families:** a drive that feels familiar, works offline, and stores files in a standard layout you can copy and read without FileSquid; encrypted backup you can leave with everything intact (U-02, U-03, U-09, U-10).
> - **For self-hosters and small organisations:** one multi-user Rust server whose storage is an open archival layout, not a database plus blobs (U-06, U-19).
> - **For researchers and creators:** provenance and descriptions travel with the file from import onward (U-54–U-66, U-78).
> - **For archivists:** standard OCFL, versioned batch jobs and dataset outputs, through a UI built for their work (Arkrim) on the same platform as everyone else (U-39–U-53, U-85, U-86).
> - **For developers:** OCFL, metadata and C2PA engines that run natively and in the browser, with one shared CLI, `fsquid`, and one documented API (U-16, U-40, U-63, U-67).
> - **For distribution builders:** a complete preservation-grade backend under any brand and vocabulary (U-24, U-25).

### Constraint requirements

#### Platforms, originals and service limits

| ID | Requirement | Related |
|---|---|---|
| C-001 | Mobile web MUST NOT promise continuous background camera backup or long-running work after browser closure. Native mobile applications are a separate investment decision. | FE-017 |
| C-002 | A preview MUST NOT be treated as an original. Availability MUST be visible before export or editing. | T-016 |
| C-003 | Synchronization MUST NOT be treated as an independent backup. | U-09; FE-020 |
| C-004 | Cloud encryption MUST NOT be claimed to eliminate service metadata, compromised endpoints or infrastructure-provider dependence. | FE-020 |
| C-006 | Scale-to-zero MUST apply only to eligible work, not all paid-service infrastructure. Server storage, replication and baseline operations have continuing costs. | U-12 |
| C-007 | Free local access and export MUST survive paid cancellation. Local storage capacity MUST be limited by hardware, not an artificial asset-count paywall. | U-02, U-12 |
| C-028 | Originals MUST remain byte-exact. Format conversions MUST create linked renditions. TIFF, PNG, Markdown and JSON are examples of portable representations, not a mandate to replace acquired RAW, HEIC, video or signed resources. | FE-001, FE-008 |
| C-029 | Copying referenced-library pointers MUST NOT be treated as preserving missing media. | FE-003 |
| C-030 | Optional encrypted local packages and remotely unavailable originals MUST disclose key, tool and availability requirements. FileSquid MUST NOT promise unrestricted ordinary-file access for those packages. | U-02 |
| C-031 | The FileSquid paid core is a **user subscription account** for managed encrypted OCFL **backup and/or** encrypted account **sync**, hosted at `*.filesquid.tech`. Backup and sync MUST remain distinct guarantees. The same account model MUST scale from personal to institutional quotas on the same infrastructure. ImageSquid sells it under its own brand (imagesquid.app) as a white label of FileSquid's backup and sync features, on the same infrastructure; other distributions MAY do the same. Local capture, self-service bulk migration and upload tooling, C2PA record maintenance, collection-wide provenance/C2PA scans and exportable reports, metadata controls, preservation packaging and export MUST remain free. | U-12; FE-020 |
| C-051 | After verified paid encrypted preservation, the next core paid value offering is fast, reliable **batch jobs** on an owner's OCFL collection with full OCFL versioning compliance (committed new versions; no silent rewrite of accepted state), such as curation jobs (for example transcoding), Parquet datasets and research-grant FAIR reporting. They MUST be FileSquid modules exposed through the API, with the Arkrim distribution as their primary UI. These jobs MUST NOT replace the backup/sync subscription as the first commercial assumption. | U-12, U-39, U-15; C-031, C-044, C-045 |
| C-032 | Free tooling MUST NOT imply free unlimited hosted capacity or transfer. Scoped human migration assistance and selected publication are separately priced services. | U-12; C-006 |

#### Performance

| ID | Requirement | Related |
|---|---|---|
| C-008 | High performance MUST remain a free core requirement. “Near-instant” means verified interaction and media-readiness budgets for declared workloads. It MUST NOT mean that a missing remote original or an uncached large decode completes in zero time. Immediate meaningful state and measured progressive readiness MUST be shown, including failures and unavailable content. | U-11; T-010, T-031 |
| C-009 | Performance MUST NOT compromise exact originals, color or orientation fidelity, provenance, preservation verification, authorization or sharing privacy. Fast previews MUST be identified derivatives. Private caches and prefetch MUST respect library and recipient boundaries, applicable encryption and lock/logout policies. Resource pressure MUST reduce speculative or background work safely rather than exhaust memory or disk or discard authoritative content. | U-11; T-020, T-021, T-025 |

#### Provenance, metadata and disclosure

| ID | Requirement | Related |
|---|---|---|
| C-010 | A complete HTTP record that contains reusable credentials or submitted personal data MUST remain encrypted private evidence with controlled access. Sanitized display or export MUST NOT silently replace the retained capture. Provider publication MUST NOT automatically receive those secrets. | FE-032; T-006 |
| C-011 | Where the `c2pa` module is enabled, C2PA record initialization MUST work for every import even when signing, embedding or a provider is unavailable. Unsigned or prepared data MUST NOT be treated as a valid signed manifest. A later signature MUST NOT invent historical camera provenance or prove depicted truth. | FE-036, FE-038 |
| C-012 | Watermarking modifies pixels or bitstreams and therefore MUST target a new derivative with a fresh final binding. A paid publication offer MUST include tested watermark retrieval and credential validation for its declared formats and destinations. | FE-042 |
| C-013 | A sidecar edit MUST NOT be presented as removal of embedded metadata. | FE-056 |
| C-014 | Sharing and publication MUST enforce the user's scrubbing mode, ordinary retained-field choices and, when selected, the provenance/AI-mark retain-class on delivered media and accompanying metadata. Unknown or unsupported enabled-mode removal MUST block the route until a supported output or explicit policy change is selected. Failure MUST NOT silently disable scrubbing. | FE-039, FE-057 |
| C-015 | OCFL MUST NOT be treated as encryption or a disaster-recovery service by itself. Ciphertext verification, independent copy policy, user-held keys, retention/erasure and actual restore tests are all required. Unlimited versions or universal obsolete-format rendering are not promised. | U-10, U-03; FE-033 |
| C-016 | A valid C2PA manifest, a provenance-linked watermark, or a viewer badge that requires extra software MUST NOT be treated as the human-perceivable AI disclosure. Where a professional publish route requires that disclosure, it MUST be understandable at first exposure without dedicated tools. The default visible labelling set MUST be the EU icons for labelling AI-generated content. | U-84; FE-044 |
| C-033 | Sharing and publication scrubbing MUST default to enabled with an empty ordinary retained-field list (GPS, camera serials, and similar). Privacy-on outgoing filenames for sharing and publication MUST default to FileId (RFC 4122 / RFC 9562 UUID version 4) plus the real media extension, with an explicit **Use original filename** choice that the owner MAY save per policy. This is the one deliberate exception: every UI surface, search, and owner download and export MUST use the owner's original file name (or their rename), and the stored name `<file-uuid>.<ext>` is never shown. | U-66; FE-056 |
| C-034 | Verification of professional AI-disclosure routes MUST include presence or permitted absence of the visible label, presence of selected provenance/AI marks, and the watermark/C2PA result of a paid job. Failure MUST block the route or surface an explicit unsupported state. It MUST NOT silently drop the label or marks. | FE-057 |
| C-035 | Byte verification, source observations, signed provenance and depicted truth MUST remain distinct claims. | C-011, C-018 |
| C-043 | FileSquid MUST NOT accept descriptive keys in an undeclared namespace. Owner-authored descriptive keys MUST be `dcterms:` properties, confirmed `schema:` overflow, or terms in a registered extension namespace (a declared prefix and IRI, for example RiC-O, or an owner-declared custom namespace for free key:value pairs). Unknown `dcterms:` local names remain rejected. Later F-047 professional profiles remain a named, validated overlay. | U-55; FE-068 |
| C-050 | Every member of a virtual exhibit MUST be marked for public access, with no privacy restriction on that exhibited media. Hidden media, private collections and private shared libraries MUST NOT appear in an exhibit. Acquisition HTTP evidence, credentials, cookies, keys and encrypted backup MUST remain in the private evidence envelope and MUST NOT be disclosed through an exhibit ([C-010](#provenance-metadata-and-disclosure)). Hosted public exhibit delivery MUST remain distinct from the private per-user mailbox ([FE-034](#portable-preservation)). | U-07; FE-071; T-047 |

#### AI and Article 50

| ID | Requirement | Related |
|---|---|---|
| C-017 | Assistive standard editing that does not substantially alter the input or its semantics MUST NOT be classified or labelled as AI-generated content. Material AI changes MUST remain in the marking and disclosure scope. | FE-009, FE-010 |
| C-018 | FileSquid MUST NOT market C2PA signatures, watermarks, EU icons, or the professional publication profile as EU AI Act certification, as proof that an image is or is not AI-generated in law, as proof of depicted truth, or as satisfaction of every applicable law. | FE-048 |
| C-019 | FileSquid MUST NOT place a general-purpose AI (GPAI) model on the market, MUST NOT resell frontier inference, and MUST NOT mark every BYO output as FileSquid-generated. | FE-011, FE-050, FE-051 |
| C-020 | FileSquid MUST NOT implement emotion recognition, untargeted facial-image scraping, or remote biometric identification in publicly accessible spaces. People/pets albums are local organisation of the owner’s library, not a biometric identification product. | FE-015, FE-026 |
| C-021 | FileSquid MUST NOT describe the consumer application as a whole as an Article 50 AI system. Article 50 behaviours attach to specific generative, interactive, biometric-categorisation and professional-publish features. | FE-063 |
| C-022 | Releasing FileSquid under a free and open-source licence MUST NOT be documented or marketed as removing Article 50 marking or disclosure duties for features that fall under that article. | C-021 |
| C-023 | FileSquid MUST NOT require retroactive visible labelling of content generated before 2 August 2026. New professional publication of AI-generated or materially AI-manipulated work after that date remains in the disclosure-profile scope. | FE-043 |
| C-024 | Article 50 publication and disclosure work MUST NOT precede ownership, recovery and paid encrypted preservation. | FE-063 |

#### Release sequence

| ID | Requirement | Related |
|---|---|---|
| C-025 | After the FileSquid MVP ([C-056](#release-sequence)), encrypted backup and sync at `*.filesquid.tech` MUST be the next release and the primary product. Day-to-day file work MUST stay in the Drive. OCFL compliance and the preservation layout MUST stay behind that Drive. The first consumer release (the default distribution, the `ocfl` and `metadata` modules, on desktop and `filesquid-server`) MUST follow that backup release. Encrypted backup and sync MUST precede ImageSquid. ImageSquid MUST be the second consumer release, delivered as a FileSquid distribution, and the FileSquid `c2pa` module (Content Credentials, its API routes and panel) MUST ship with it. Files imported before the `c2pa` module is enabled MUST then receive initial records that state when they were made, cite the import record made at import and MUST NOT claim import time. The `c2pa` module reaches the ImageSquid distribution only through the FileSquid API. FileSquid releases MUST NOT wait for interpretation support for every file type (C-027). | U-15; FE-063; C-052 |
| C-056 | The FileSquid MVP MUST be an early-access build of the default distribution on the macOS desktop host only: no `filesquid-server`, no Linux or Windows installer, and no accounts, groups or sharing. Its scope MUST be: create, open, copy and recover a local library from OCFL alone; file and folder import with durable jobs, where every import stores the original under its FileId plus extension and commits, in the same OCFL version, the file's metadata record `<file-uuid>.metadata.md` with the original file name, extension, format and the source-system and host facts named in [ADR 038](./architecture/038-import-file-record.md) (each missing fact recorded as unavailable with a reason), and its PROV-O activity; original file names shown on every surface and used for owner downloads; the Drive-style browser (list and grid, previews, upload, folders, rename or move, copy, remove and restore, selection, sort keys, streamed ZIP download) with the three-tab file panel (File info, Description, Provenance; Versions opens from a header button) and the Logs view with export; search with filter chips; version labels and restore; metadata extraction and `dcterms:` / `schema:` / registered-namespace editing persisted in that metadata record; the PROV-O history of every version and change ([ADR 040](./architecture/040-prov-o-history.md)); the activity journal and Monitor dashboard. The MVP MUST NOT include C2PA or the Content Credentials panel ([C-025](#release-sequence)). The activity stream and comments, virtual exhibits, the distribution manifest, JSON sidecars, published module crates, installer signing and the updater MUST follow the MVP. The MVP installer MAY be unsigned and not notarized; it MUST then be labelled early access where it is distributed (the app's About view instead states that the build is not signed or notarised), MUST NOT be marketed as the consumer release, and its install guide MUST state how to open it on macOS. | U-15, U-34; FE-017, FE-075; C-025 |
| C-052 | After the ImageSquid release, the owner MUST choose the order of the later milestones: the Arkrim archives distribution (with curation jobs, Parquet and FAIR reporting), IIIF/GLAM interoperability, and ProofMode integration. Hosted private OSS inference (F-048) remains a separate SKU after verified paid encrypted preservation. The backup/sync subscription (C-031) and the OCFL-job layer (C-051) stay in force regardless of that order. | U-39, U-15; C-031, C-051 |
| C-026 | Every distribution (ImageSquid, Arkrim, third parties) MUST reuse the FileSquid architecture through the FileSquid API. A distribution MUST NOT introduce a third application language or a backend of its own. | U-29, U-15 |
| C-027 | “All file types” in FileSquid means it is not limited to photo and video. FileSquid MUST NOT promise a working viewer, editor or metadata mapper for every existing format. Unknown types MUST remain exact-byte preservable with disclosed interpretation limits. | U-15 |

#### Module rules

| ID | Requirement | Related |
|---|---|---|
| C-044 | The default public API of `filesquid-ocfl` MUST be generic OCFL 1.1 (storage roots, objects, inventories, SHA-256, filesystem and S3-compatible backends). FileSquid's file-object layout, collection snapshots, recipes, C2PA sidecars, catalog rebuild and dual-profile encryption MUST live above that module in FileSquid's application layer (`filesquid-api` and later modules). | U-40; FE-069; [ADR 023](./architecture/023-filesquid-ocfl-engine-and-parquet.md), [ADR 029](./architecture/029-filesquid-unified-product-and-distributions.md) |
| C-045 | A later OCFL-to-Parquet projection MUST be an optional Cargo feature of `filesquid-ocfl` (default off). FileSquid MVP MUST NOT compile Apache Arrow, Apache Parquet, DuckDB or Iceberg into the engine default graph. The MVP engine MUST already expose inventory-driven, version-pinned logical-state enumeration so that feature can be added without rewriting the core API. | U-40; T-039, T-041; [ADR 023](./architecture/023-filesquid-ocfl-engine-and-parquet.md) |
| C-046 | Analytical outputs MUST write to a caller-chosen location outside OCFL object and storage-root reserved paths. A dataset accepted for preservation MUST re-enter as ordinary OCFL object content. Parquet, DuckDB and Iceberg MUST remain derived representations; OCFL remains authoritative for accepted archival state. | C-044, C-045; [ADR 010](./architecture/010-ocfl-1-1-dual-profile.md) |
| C-047 | `filesquid-ocfl` MUST remain independently installable as a FOSS library and tool. FileSquid MUST NOT absorb it into a private module, require the FileSquid app in order to use the engine, or treat community OCFL CLIs as a substitute. The crate MUST build, test and publish without other FileSquid crates. Layout remains OCFL 1.1; “improved versioning” means crash-safe `vN` publication (reserve before content, inventory last, unpublished `vN`, S3 compare-and-swap), not a second preservation standard. | U-40; FE-069; [ADR 016](./architecture/016-rust-ocfl-library.md), [ADR 023](./architecture/023-filesquid-ocfl-engine-and-parquet.md) |
| C-048 | `filesquid-metadata` MUST remain an independently installable FOSS metadata CRUD engine. It MUST extract, create, read, update and delete metadata for files in any supported store; `filesquid-ocfl` is an optional store, and FileSquid MUST use it as canonical ([ADR 027](./architecture/027-filesquid-metadata-pluggable-stores.md)). It MUST NOT reimplement OCFL, absorb FileSquid catalog or UI types, or require the FileSquid app. FileSquid MUST NOT keep a parallel metadata extract or write path. | U-63; FE-070; [ADR 022](./architecture/022-dublin-core-descriptive-metadata.md), [ADR 024](./architecture/024-filesquid-metadata-engine.md) |
| C-049 | Default OCFL metadata storage for FileSquid and its distributions MUST be Markdown with YAML front matter. CSV, JSON, XML, and XMP are interchange (or OCFL payloads) only when a named requirement or integration profile explicitly requires them. The engine MUST NOT ship MODS or SQL import/export. | U-63; T-045, T-046 |
| C-053 | `filesquid-ocfl`, `filesquid-metadata` and `filesquid-c2pa` MUST run natively in the desktop host and in `filesquid-server`, and MUST build as `wasm32-unknown-unknown` for the encrypted browser host. On the ciphertext-only hosted service (`*.filesquid.tech`) they MUST NOT process private content. `filesquid-ocfl` MUST reach bytes only through an async, runtime-agnostic storage backend interface (local filesystem on native, S3-compatible on native and WASM, in-memory for tests). Browser storage MUST be a cache, never a canonical OCFL root. | U-20, U-23; FE-069, FE-070; [ADR 032](./architecture/032-hosting-and-privacy-model.md) |
| C-055 | File facts MUST have one owner. `filesquid-ocfl` MUST own storage facts only: object identifier, logical path and implied folders, versions, version `created`, `user` and `message`, digests and fixity, size and content location; an extension-based type label is a display hint only. Every other file fact (embedded EXIF/ID3/IPTC/XMP, format identification, original creation or capture dates, the import record of source-system and host facts, Dublin Core and Schema.org description, titles, captions, keywords, rights) MUST belong to `filesquid-metadata`. Content Credentials MUST belong to `filesquid-c2pa`. Application meaning (collections, people, favorites, search indexes, recipes) MUST belong to FileSquid's application layer and be presented by distributions. | U-01; FE-073; [ADR 028](./architecture/028-filesquid-ui-packages-and-panel-composition.md) |
| C-054 | `filesquid-c2pa` MUST implement C2PA processing from scratch: JUMBF/CBOR manifest stores, assertions, hard bindings, COSE signatures, certificate-chain and timestamp validation, manifest minting, edits, embedding and scan reports. It MUST NOT hand-roll cryptographic primitives; established crates behind a crypto-backend trait supply them. The official `c2pa` crate and `c2patool` MUST be CI interoperability oracles, never the implementation or a runtime dependency. Default dependencies MUST exclude `filesquid-ocfl`, Axum, Tauri and other FileSquid crates. Trust lists and signers MUST be caller-supplied. Like C-053, it MUST NOT process private content on the ciphertext-only hosted service. | U-67; FE-072; [ADR 026](./architecture/026-filesquid-provenance-engine.md) |
| C-057 | Every FileSquid command-line tool MUST be invoked as `fsquid`, followed by commands and parameters. FileSquid MUST NOT ship another CLI binary name (for example `filesquid-metadata`); the `filesquid-desktop` and `filesquid-server` hosts are applications, not CLI tools. One crate, `filesquid-cli`, MUST build the `fsquid` binary with the commands of every module (OCFL, metadata, PROV-O history, C2PA; the metadata module's are `extract`, `get`, `set`, `export`, `import`, `namespace` (`register`, `list`) and `validate`; the `prov` module's are `prov show`, `prov log`, `prov validate` and `prov export`); module crates MUST keep their command logic in their libraries and ship no binaries. A command whose name only one module defines MUST run at the top level (`fsquid <command>`). Every command MUST also run in its module group (`fsquid <module> <command>`, for example `fsquid c2pa verify`), and the group MUST be required only for a command name that more than one module defines. `validate` MUST always require its group (`fsquid ocfl validate`, `fsquid metadata validate`). FileSquid hosts MUST NOT shell out to `fsquid` for canonical I/O. | U-40, U-63, U-67; FE-069, FE-070, FE-072; C-047, C-048, C-054; [ADR 036](./architecture/036-single-fsquid-cli.md) |

#### Parity, preservation and sequence

| ID | Requirement | Related |
|---|---|---|
| C-038 | Preservation MUST remain possible even when interpretation or editing is unavailable. | C-027; FE-033 |
| C-041 | Safe migration MUST remain a reusable capability whose placement is governed by its dependencies. | FE-028, FE-029 |
| C-042 | Commercial rollout MUST follow evidence of useful local operation, verified protection, paid demand, and sustainable costs.|
