# Task API-F1.4 Conflict Artifact: two incompatible encodings of one file record

Raised 2026-10-20 while starting [Task API-F1.4](../plan.md) (Metadata property routes), per [sync-logic.md](../../../../../../.agents/rules/sync-logic.md). API-F1.4 must commit an upload's import record and serve and edit a file's statements in that record. Two writers of the same record now exist, and neither can read what the other writes.

## 1. Conflict description

Each file's metadata record `<file-uuid>.metadata.md` (schema `filesquid.original.v1`) has two implementations with two incompatible encodings:

- **`filesquid-api` `canonical` module** ([original.rs](../../src/canonical/original.rs), Task DE-0.3): `encode_companion` writes exactly four front-matter lines, `schema`, `original_name`, `extension` and `media_type`. `decode_companion` accepts only that six-line layout, so it refuses any record with more lines. `commit_file_original` and `read_file_original` use it, and the desktop host reads display names through it.
- **`filesquid-metadata` `FileRecord`** ([file_record.rs](../../../metadata/src/file_record.rs), Task MD-0.8): writes `schema`, `original_name`, `name_normalization`, `display_name`, `extension`, `format`, `media_type`, the descriptive namespaces and the append-only `import` list. `FileRecord::from_markdown` requires every one of those header keys and at least one import entry. It refuses the canonical form with `MissingField("name_normalization")`, and a record without an import entry with `NoImport`.

As a result:

- A record committed by `OcflStore::commit_import_with` (the MD-0.8 path that API-F1.4 must use for uploads) breaks `read_file_original`: the display name is lost and the head is refused.
- A record committed by `commit_file_original` cannot be read, edited or appended to by the metadata store, so API-F1.4 cannot add statements to it.

A second, related gap is where an upload's record lives. API-F1.3's `PUT /files/upload` stores the bytes at a logical path the client chooses inside an existing object. API-S-026 and ADR 038 place originals at `original/<file-uuid>.<ext>` in a per-file object `files/<file-id>`, with the record at `original/<file-uuid>.metadata.md`. API-F1.13, which runs after API-F1.4, owns that stored name. So API-F1.4 has no defined record path for a file uploaded through the API-F1.3 route.

## 2. PRD / SRS reference

- [API-S-026](../srs.md): "The metadata record MUST hold the owner's original file name exactly, the extension (`null` when there is none) and the media type, as YAML front matter (`schema: filesquid.original.v1`, then `original_name`, `extension` and `media_type`, each written as JSON) … `read_file_original` MUST answer it from the metadata record … and MUST refuse a head without the record or the original it names".
- [MD-S-013](../../../metadata/resources/srs.md) and [MD-S-008](../../../metadata/resources/srs.md) (Task MD-0.8): the record holds the original name with its normalisation form, the display name, the extension, the format and MIME type, and the append-only `import` section, persisted through the `ocfl` store.
- API-F1.4 plan row: "The upload route of API-F1.3 gains its import record here: an upload commits the record in the same OCFL version as the content".

## 3. ADR reference

- [ADR 038](../../../../../../resources/architecture/038-import-file-record.md) Decision 2: "The file record belongs to `filesquid-metadata` ([C-055]) … The crate defines the schema, validation and persistence and takes the facts from its caller". Amendment of 2026-10-19: `<file-uuid>.metadata.md` "is THE metadata record of that file", with the original name, extension, format and MIME type, descriptive metadata and the `import` section.
- [ADR 027](../../../../../../resources/architecture/027-filesquid-metadata-pluggable-stores.md) (as amended 2026-10-19): the `ocfl` store of `filesquid-metadata` persists that per-file record.
- C-055 (module rules): every file fact has one owner.

## 4. Recommended resolution

**Option A (recommended): `filesquid-metadata` is the only codec.**

1. Replace `encode_companion` and `decode_companion` in `canonical/original.rs` with `FileRecord::to_markdown` and `FileRecord::from_markdown`.
   - `commit_file_original` takes the import facts the host gathered and commits a full `FileRecord`.
   - `read_file_original` answers `display_name()`, `extension()` and `media_type()` from it.
2. Amend API-S-026 so that the metadata record is the MD-0.8 file record of MD-S-013, not a four-field form of its own. ADR 038 and ADR 027 already say so.
3. For uploads, choose one:
   - **3a.** Move the stored-name half of API-F1.13 into API-F1.4: an upload creates the per-file object `files/<file-id>` with `original/<file-uuid>.<ext>` and its record, through the `canonical` layout.
   - **3b.** Keep API-F1.3's path model for now. An upload to logical path `P` commits its record at `P` with the last segment's extension replaced by `.metadata.md`, refusing a name that collides. API-F1.13 then moves both to the UUID layout.

   3a avoids a temporary layout that API-F1.13 would have to migrate.

**Option B: two schemas.** Keep the canonical four-field form under a new schema ID (for example `filesquid.original-name.v1`) and let `FileRecord` own `filesquid.original.v1`. This keeps two files per original and contradicts ADR 038's "THE metadata record". Not recommended.

Artifacts to update under Option A:

- `apps/filesquid/crates/api/src/canonical/original.rs`, `store.rs` and their tests (`tests/unit/canonical.rs`).
- The desktop host's `read_file_original` callers (DE-0.3 code in `filesquid-desktop`, desktop lane).
- [API-S-026](../srs.md) text.
- The API-F1.4 and API-F1.13 rows in [plan.md](../plan.md) for the split chosen in 3a or 3b.
- The walkthrough of DE-0.3, if it quotes the four-line format.

`filesquid-metadata` needs no change.

## 5. Impact assessment

Without a resolution, API-F1.4 cannot meet its acceptance criteria:

- An upload that commits its import record through the metadata store makes `read_file_original` refuse that file's head. The desktop then loses the display name and the content answers' media type for every uploaded file.
- Statement edits through the metadata store fail on every file the desktop committed with `commit_file_original`.
- Two writers of one schema ID would leave libraries with records that one reader or the other refuses. That breaks API-S-026's "MUST refuse a head without the record" for valid libraries and the single-owner rule for file facts (C-055).

Nothing has been implemented for API-F1.4 yet. Work continues only once the owner chooses an option.

## Resolution (owner decision, 2026-10-20)

The owner chose **Option A with 3a** ("A+3a"), recorded in [ADR 038](../../../../../../resources/architecture/038-import-file-record.md) Decision 8, [API-S-026](../srs.md) and the API-F1.4 [plan row](../plan.md):

- `filesquid-metadata`'s `FileRecord` is the only codec of `<file-uuid>.metadata.md`.
- An upload creates the per-file object `files/<file-id>` with `original/<file-uuid>.<ext>` and its record in its first version. The stored name keeps `.<ext>` only when the extension is 1 to 16 ASCII letters or digits after a dot that does not start the name; otherwise the original is stored as `original/<file-uuid>` with no suffix, and the record keeps the full extension.

This task implements it.
