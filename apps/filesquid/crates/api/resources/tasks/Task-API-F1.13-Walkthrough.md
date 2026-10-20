# Task API-F1.13 Walkthrough: Display names and download names

Branch `feat/api-f1.13-display-download-names`, module `filesquid-api` ([plan row](../plan.md)), part of global Task FS-1.3. The stored-name half moved into API-F1.4 (owner decision 2026-10-20, "A+3a"). Contract 0.20.0.

## What changed

- **Rename changes only the display name.**
  - `POST /api/v1/files/renames` (`files.write`, body `RenameRequest`) sets the record's display name through `FileRecord::set_display_name`.
  - It commits one new version of the file object through the metadata module's `ocfl` store, with the caller as the version user, and is journaled as `files.rename` with opaque IDs only.
  - The stored name, the logical path, the original name and every import entry stay. The same name is 409; an empty name, one over 1,024 bytes or one with `/` or NUL is 400. Neither publishes a version.
- **UI-F2.2 finding.** The UI renamed files through `POST /api/v1/files/moves`, which changed the logical path. A move of a file object's `original/` (or of the folder itself) is now refused (400) with a pointer to the rename route. Only an object stored at `files/<file-id>` counts: a collection snapshot's identifier is a `urn:uuid:` too, and its own `original/` folder still moves.
- **Display names on every built route.**
  - `FileHistory.display_name` and `ObjectVersions.display_name` are new. The second is `null` for an object that is not a file object.
  - Listings, details, content and download already used the record (API-F1.4).
- **Content-Disposition** (RFC 6266, API-S-027).
  - `filename*=UTF-8''` carries the display name.
  - The `filename` fallback keeps printable ASCII and replaces every other character and every `"`, `\`, `/`, `;` and control character with `_`.
  - A display name with no ASCII letter or digit falls back to the FileId plus extension.
- **One safe display name.** `names::safe_name` now trims and cuts exactly as `filesquid-metadata`'s `safe_display_name`; a unit test holds them equal.
- **Sharing default** (C-033). `canonical::outgoing_name(file_id, record, policy)` answers the FileId plus the stored extension under the default `OutgoingNames::FileId`, or the display name under `OutgoingNames::OriginalName`. No sharing or publication route exists yet; this function is the seam they call.

## Left for later

- Withholding `OriginalName` from a shared record. The sharing routes apply MD-0.10's scrub class (MD-S-013), and MD-0.10 is still open.
- Search results and events naming files. No search route (F-063) or file event exists yet.

## SRS mapping

| SRS ID | Title | Evidence |
| --- | --- | --- |
| [API-S-027](../srs.md) | Display and download names | `integration_names` (7 tests), `display_and_download_names_API-S-027.feature` (6 scenarios), `files::names` unit tests, `unit_canonical` sharing tests |
| [API-S-026](../srs.md) | The FileSquid OCFL application profile (the original's stored name is fixed) | `a_move_of_a_file_objects_original_is_refused_with_a_pointer_to_rename`, `a_rename_changes_only_the_display_name_in_one_new_version` |
| [API-S-005](../srs.md) | Opaque handles, never raw filesystem paths (the one safe display transformation) | `the_display_name_matches_the_metadata_modules_safe_display_name` |
| [API-S-002](../srs.md) | The versioned API contract owned by this crate | `unit_contract` (rename route, `RenameRequest`, new `display_name` properties, 0.20.0), regenerated `schema.d.ts` |

## Test output

`cargo test -p filesquid-api --locked --lib files::names`:

```text
test files::names::tests::a_name_without_an_ascii_letter_or_digit_falls_back_to_the_file_id ... ok
test files::names::tests::quotes_backslashes_slashes_semicolons_and_controls_never_reach_the_fallback ... ok
test files::names::tests::a_non_ascii_name_with_spaces_gets_a_fallback_and_an_exact_utf8_form ... ok
test files::names::tests::long_names_are_cut_at_a_character_boundary ... ok
test files::names::tests::the_display_name_matches_the_metadata_modules_safe_display_name ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 46 filtered out; finished in 0.00s
```

`cargo test -p filesquid-api --locked --test integration_names`:

```text
test a_move_of_a_file_objects_original_is_refused_with_a_pointer_to_rename ... ok
test a_rename_to_the_same_or_an_invalid_name_publishes_nothing ... ok
test a_rename_changes_only_the_display_name_in_one_new_version ... ok
test an_original_folder_in_an_object_that_is_not_a_file_object_still_moves ... ok
test a_name_without_an_ascii_letter_or_digit_falls_back_to_the_file_id ... ok
test history_and_versions_carry_the_display_name ... ok
test download_names_survive_quotes_backslashes_slashes_and_controls ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
```

`cargo test -p filesquid-api --locked --test unit_canonical` (new tests and summary):

```text
test a_sharing_name_drops_an_extension_the_stored_name_does_not_keep ... ok
test sharing_names_a_file_by_its_file_id_unless_the_policy_selects_the_original_name ... ok
test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s
```

`cargo test -p filesquid-api --locked --test features` (this feature and the run summary):

```text
Feature: Display and download names
  Scenario: The history and versions of a file carry its display name
   ✔  Given the owner has uploaded "Été à Nice.jpg"
   ✔  Then the file's history should name it "Été à Nice.jpg"
   ✔  And the file's versions should name it "Été à Nice.jpg"
  Scenario: A rename changes only the display name, in one new version
   ✔  Given the owner has uploaded "IMG_0001.JPG"
   ✔  When the owner renames the file to "Harbour at dawn.JPG"
   ✔  Then the rename should publish version "v2"
   ✔  And the listing should show "Harbour at dawn.JPG" at the same stored path
   ✔  And the facts should still hold the original name "IMG_0001.JPG"
   ✔  And the rename should be journaled as "files.rename" with opaque IDs only
  Scenario: A move of a file's stored original is refused
   ✔  Given the owner has uploaded "a.txt"
   ✔  When the owner moves the file's original to "b.txt"
   ✔  Then the move should be refused with a pointer to the rename route
  Scenario: A download name keeps quotes, backslashes and semicolons out of the fallback
   ✔  Given the owner has uploaded "summer.jpg"
   ✔  When the owner renames the file to "Sum\"mer\\2023;1.jpg"
   ✔  Then the download should be named "attachment; filename=\"Sum_mer_2023_1.jpg\"; filename*=UTF-8''Sum%22mer_2023%3B1.jpg"
  Scenario: A name without an ASCII letter or digit falls back to the file identifier
   ✔  Given the owner has uploaded "summer.jpg"
   ✔  When the owner renames the file to "写真 — 夏"
   ✔  Then the download fallback should be the file identifier plus ".jpg"
  Scenario: Sharing names a file by its identifier unless the policy selects the original name
   ✔  Given the owner has uploaded "Été à Nice.jpg"
   ✔  Then the default outgoing name should be the file identifier plus ".jpg"
   ✔  And the outgoing name under the original-name policy should be "Été à Nice.jpg"
23 features
344 scenarios (242 passed, 102 skipped)
1182 steps (1080 passed, 102 skipped)
```
