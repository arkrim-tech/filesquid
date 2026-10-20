# Task API-F1.4 Plan: Metadata property routes

Branch `feat/api-f1.4-metadata-property-routes`, module `filesquid-api` ([plan row](../plan.md)), part of global Task FS-1.3. The owner decided the record format on 2026-10-20 ("A+3a", [ADR 038](../../../../../../resources/architecture/038-import-file-record.md) Decision 8; [Conflict Artifact](./Task-API-F1.4-Conflict.md)): `filesquid-metadata`'s `FileRecord` is the only codec, and API-F1.4 also takes API-F1.13's stored-name half.

## Requirements

[API-S-026](../srs.md), [API-S-027](../srs.md), [API-S-018](../srs.md), [MD-S-002](../../../metadata/resources/srs.md), [MD-S-008](../../../metadata/resources/srs.md), [MD-S-012](../../../metadata/resources/srs.md), [MD-S-013](../../../metadata/resources/srs.md), [MD-S-014](../../../metadata/resources/srs.md).

## Design

1. **One codec.**
   - `canonical/original.rs` drops `OriginalRecord`, `encode_companion` and `decode_companion`.
   - `commit_file_original` takes a full `FileRecord` and the original's bytes. It stages the original at `original/<file-uuid>[.<stored ext>]` and `FileRecord::to_markdown` at `original/<file-uuid>.metadata.md` in one version.
   - It refuses a record whose new import entries do not cite those bytes and that path.
   - `read_file_original` decodes with `FileRecord::from_markdown` and answers the record with the original's path.
2. **Upload creates the per-file object.**
   - `PUT /api/v1/files/upload?library_id&name&size[&file_id][&media_type][&last_modified][&message]`, with the bytes as the body.
   - Without `file_id`, the route mints a FileId (UUID v4) and creates `files/<file-id>` (identifier `urn:uuid:<file-id>`) with the original and its record in v1.
   - With `file_id`, it is a new version of that object: new bytes at the same stored path and one appended import entry. The display name is unchanged.
   - The verified size is the size of the bytes received. A declared `size` that differs fails verification, and nothing is committed.
   - Facts the client does not provide are listed as unavailable (`ImportEntry::for_upload`).
   - The answer is the `FileCommit` plus `file_id`.
3. **Content answers use the record.** For a per-file object's original, the content and download routes name the file by the record's display name and carry its media type.
4. **Metadata routes** (action `files.read` for reads, `files.write` for edits). Each edit commits one version through `OcflStore` (MD-S-014), with the API user and an operation message.
   - `GET /api/v1/files/metadata`: the file's statements, as prefix, term and value, with a source of `extracted` (`exif:`) or `asserted`.
   - `PUT /api/v1/files/metadata/statements`: set one statement by CURIE.
     - `dcterms:` must pass the `/terms/` register (`dc:` maps onto it).
     - `schema:` is accepted.
     - Any other prefix must be registered.
   - `POST /api/v1/files/metadata/removals`: remove one statement.
   - `GET /api/v1/files/metadata/terms?library_id&name`: the add order for a bare name. First `dcterms:<name>` when the register knows it, then the `schema:<name>` suggestion, then each registered namespace that accepts it.
   - `GET` and `POST /api/v1/files/metadata/namespaces`: the library's registry, as `namespaces.md` in the snapshot object; register an extension or custom namespace as the owner.
   - `GET /api/v1/files/facts`: the record's `FileFacts` projection (MD-S-013, API-S-027), with no absolute path, import root, host or account. Nothing edits it.

## Verification

- Unit tests `tests/unit/canonical.rs`.
- Integration tests `tests/integration/metadata.rs` on a real filesystem root.
- BDD feature `tests/features/metadata_property_routes_F-052.feature`.
- Contract tests and regenerated `schema.d.ts`.
- `FULL=1 bash scripts/pre-commit.sh`.
