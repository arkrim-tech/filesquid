# Architecture Decision Record - FileSquid

038-import-file-record.md

## Title

An import file record of source-system and host facts for every imported file

## Status

Accepted

Accepted: 2026-10-18. Follows [ADR 037](./037-c2pa-in-release-2.md), which removes the import-time C2PA record from the MVP and release 1; this record replaces it there. Extends [ADR 027](./027-filesquid-metadata-pluggable-stores.md) (a second canonical metadata file in the OCFL object) and applies the one-owner rule of [C-055](../product-vision.md#module-rules).

Amended 2026-10-19 (owner decision): there is no separate `file-record.md`. Every imported original is stored under the universally unique name `<file-uuid>.<ext>` (its UUID v4 FileId, [ADR 021](./021-uuid-v4-identifiers.md), plus its original extension only when that extension is 1 to 16 ASCII letters or digits; any other or longer extension gives no suffix, so no source name can become an OCFL path), and right next to it, in the same directory, `<file-uuid>.metadata.md` is THE metadata record of that file ([ADR 027](./027-filesquid-metadata-pluggable-stores.md) as amended the same day). The import facts of Decision 3 live in its append-only `import` section, beside the owner's exact original file name, the file extension, the format and MIME type and the file's descriptive metadata. Wherever this record says "file record" it now means that `import` section. Decisions 2 and 5 below are rewritten accordingly; Decisions 1, 3, 4, 6 and 7 keep their content (one entry per acquisition in the same OCFL version as the content, the same facts and bounds, private by default, never a C2PA record). The display name shown everywhere is the original file name (or the owner's later rename); the stored name is never shown ([PRD F-002](../prd.md#f-002-safe-file-folder-and-device-import)). Each import also commits its PROV-O activity in the same version ([ADR 040](./040-prov-o-history.md)).

## Context / Requirement Reference

- Requirements: [PRD F-002](../prd.md#f-002-safe-file-folder-and-device-import) (safe import), [PRD F-043](../prd.md#f-043-c2pa-records-and-action-disclosure) (import-time records once the `c2pa` module ships), [PRD F-046](../prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports) (outgoing disclosure policy), [PRD F-052](../prd.md#f-052-metadata-module), [C-055](../product-vision.md#module-rules), [C-056](../product-vision.md#release-sequence).
- Until ADR 037, every import started an unsigned C2PA record bound to the file's identity, hashes and source. Without C2PA in the MVP, FileSquid would keep only what OCFL stores: the content, its SHA-256 digest, the logical path and the version's `created`, `user` and `message`.
- Facts about where a file came from (its original name and path, filesystem timestamps, owner and permissions, extended attributes such as the macOS quarantine and "where from" attributes, Finder tags) exist only on the source system. Once the source is gone they cannot be recovered, so they must be captured at import.
- The owner decided on 2026-10-18 that every import creates a file record holding as much information about the file as the source system and the host operating system provide, in place of the C2PA record.

## Decision

1. **One file record per imported file.** Every completed import or upload MUST commit a file record in the same OCFL version as the imported content. An explicit referenced import, which records the existing bytes and location instead of copying them ([F-009](../prd.md#f-009-multiple-libraries-and-filesystem-administration)), commits its file record in the OCFL version that records the reference, and the record states that the bytes were referenced, not copied. Import completion MUST require both the verified content and its file record; a failure between them leaves a retryable pending import, never a file without a record.
2. **Owner and location.** The file record belongs to `filesquid-metadata` ([C-055](../product-vision.md#module-rules)). It is the append-only `import` section of the file's metadata record `<file-uuid>.metadata.md`, next to the stored original `<file-uuid>.<ext>` in the file's OCFL object, written through the `ocfl` store ([ADR 027](./027-filesquid-metadata-pluggable-stores.md)): YAML front matter with a versioned schema identifier, then an optional Markdown body. The crate defines the schema, validation and persistence and takes the facts from its caller; it does not read the filesystem, so it still builds for `wasm32` ([ADR 025](./025-client-side-filesquid-engines.md)). The host that performs the import gathers the facts: the desktop host's native import code on desktop, `filesquid-api`'s upload route for browser or server uploads.
3. **Contents.** Each acquisition entry records, where available:
   - **Acquisition:** when the record was written and when the import started and finished (RFC 3339, UTC); the import method (file picker, folder, drag and drop, another application, API upload); the job and operation identifiers; and the OCFL object identifier, version, logical path, SHA-256 digest and size it describes (cited for binding; `filesquid-ocfl` still owns those facts).
   - **Source file:** original file name exactly as reported, with its Unicode normalisation form; original absolute path, the import root and the path relative to it; the **file size** in bytes as the source reported it before FileSquid read it (for a symbolic link or alias, the size of the followed target, whose bytes are the ones read; the link itself is recorded by its target path below), and the verified size in bytes of the bytes FileSquid read and hashed: the copied content for a managed import, or the referenced bytes at their source location for an explicit referenced import (the two sizes MUST match, or the import fails verification and stays pending); birth (created), modified, status-changed, accessed and "date added" times at native precision, captured before FileSquid reads the source so its own read cannot change the accessed time (where a filesystem updates it anyway, the value is labelled as read after access); owner and group (names and numeric IDs), POSIX mode, ACL entries and file flags (hidden, locked); whether it was a symbolic link or alias and its target; the content type the operating system reports (UTI and MIME on macOS) and the file name extension.
   - **Source volume:** volume name and UUID, filesystem type, mount point, and whether it is local, removable or network storage.
   - **Extended attributes:** every extended attribute name with its size and value. Names and values that are valid UTF-8 are stored as YAML strings; any other name or value is stored as base64 with `encoding: base64` beside it, so binary data round-trips exactly; known ones also decoded, such as `com.apple.quarantine` (agent, time, event identifier), `com.apple.metadata:kMDItemWhereFroms` (origin URLs), Finder tags, colour label and comment, and the resource fork size.
   - **Host:** operating system name, version and build; CPU architecture; host name; the operating-system account that ran the import; and the FileSquid version, build and distribution.
   - **Unavailable facts:** every field the source or host could not provide is listed with a reason (not supported by the source, permission denied, not provided by an upload). A value is never guessed or invented.
4. **Bounds.** Per acquisition entry, attribute inspection MUST stop after 256 attribute names and 1 MiB of attribute value bytes read. A value of 64 KiB or less is stored in full; a larger value is stored as its size and SHA-256 digest, streamed within the same 1 MiB read budget. When the budget runs out, the remaining values are stored by name and size only, with their digests marked incomplete. When more than 256 names exist, the entry is marked truncated with the number of names enumerated. The bounds apply to each acquisition entry; a re-import adds a new entry with its own budget.
5. **Immutable and append-only.** Existing `import` entries are never edited. Importing the same bytes again appends a new acquisition entry in a new OCFL version, earlier entries stay unchanged, and the metadata store refuses any save that changes or removes an existing entry or the recorded original file name. Owner corrections go to the descriptive statements of the same metadata record, never to the `import` section.
6. **Private by default.** Paths, account and host names, numeric IDs, origin URLs, quarantine agents and Finder comments are identity and location fields under the F-046 outgoing disclosure policy: they are scrubbed by default from exports and shares, and kept in the owner's own library, its copies and its recovery. The Metadata panel shows a read-only projection of the record through the metadata routes of `filesquid-api`. That projection MUST NOT return raw filesystem paths to the UI ([U-18](../product-vision.md#one-api-hosts-and-distributions)): it shows the original file name, the path relative to the import root and the volume name, and leaves out the absolute path and the import root.
7. **Not a C2PA record.** The file record makes no C2PA claim and MUST NOT be presented as Content Credentials. When the `c2pa` module ships ([ADR 037](./037-c2pa-in-release-2.md)), the backfilled initial C2PA record of a file imported earlier cites its file record and OCFL version as the import evidence.

## Rationale

- Source-system facts are lost once the source is gone; capturing them at import is the only chance, and it costs little next to the copy and hash the import already does.
- Keeping the record in the OCFL object means a library recovered from OCFL alone still knows where every file came from ([F-042](../prd.md#f-042-ocfl-preservation-and-portable-recovery)).
- A separate append-only file keeps observed facts apart from owner-editable description in `metadata.md`, and gives the later C2PA backfill honest evidence to cite.
- Recording unavailable fields explicitly keeps the record verifiable: an absent value means "not recorded" only when the record says why.

## Assumptions

- The macOS desktop host can read birth time, extended attributes, ACLs and file flags through the platform APIs available to native Rust code; Linux and Windows hosts will fill the fields their filesystems provide (for example the Windows `Zone.Identifier` stream) and mark the rest unavailable.
- Browser uploads provide little more than a name, size, last-modified time and media type; that is acceptable when the rest is marked unavailable.

## Alternatives Considered

- **Keep only OCFL storage facts.** Rejected: origin, filesystem timestamps and attributes would be lost for every file imported before release 2.
- **Write the facts as statements in `metadata.md`.** Rejected: `metadata.md` is owner-editable description, and mixing observed facts with editable statements would let a correction overwrite evidence.
- **A minimal record (name, path, timestamps).** Rejected by the owner, who asked for as much information as the source system and host provide.
- **Store the record only in the operational database.** Rejected: it would not survive recovery from OCFL alone.

## Consequences / Implications

- Easier: provenance evidence exists from the MVP onward, and the release-2 C2PA backfill has something concrete to cite.
- Harder: the desktop import (DE-1.2) needs native code for macOS file metadata, and later Linux and Windows equivalents; the metadata crate (MD-0.8) gains a schema, validation and bounds; the metadata routes, which also give uploads their record (API-F1.4), and the Metadata panel (UI-F2.4) gain small additions. The plan adds about 20 hours to the MVP.
- Harder: the record holds personal data (paths, account and host names, origin URLs); the default scrub on export and share is mandatory, and the privacy statement says the record exists.
- Every OCFL object of an imported file carries the `import` section in its `<file-uuid>.metadata.md`; a library copied with ordinary tools keeps it.

## Related Decisions / Notes

- [ADR 037](./037-c2pa-in-release-2.md) (C2PA in release 2), [ADR 027](./027-filesquid-metadata-pluggable-stores.md) (metadata stores), [ADR 024](./024-filesquid-metadata-engine.md) (metadata engine), [ADR 025](./025-client-side-filesquid-engines.md) (client engines), [ADR 010](./010-ocfl-1-1-dual-profile.md) (OCFL), [ADR 035](./035-macos-first-desktop-mvp.md) (macOS first).
