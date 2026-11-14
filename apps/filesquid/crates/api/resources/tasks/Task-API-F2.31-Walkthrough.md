# Task API-F2.31 Walkthrough: The owner's library folder and volume names on the desktop

Task row: [API-F2.31](../plan.md). Global task: Task FS-2.5 ([implementation plan](../../../../../../resources/implementation-plan.md)). Owner decision 2026-11-02, verbatim: "RE: 5 - the desktop user can see folder name and volume so they can better manage their own private desktop".

## Summary of changes

- **`FolderNames` (`filesquid-api`, `local_library`).** The names of a folder: `folder_name`, the folder's own last path component, and `volume_name`, the name of the volume that holds it. Each is `None` when unreadable. `FolderNames::safe` passes both through API-S-005's safe transformation (`files::names::safe_name`). `made_safe` repeats it on whatever a host handed over, and is idempotent.
- **Port for the facts.** `LibraryRegistry::folder_names(location_id)` has a default that answers no names, so every host other than the desktop answers `null` without any change. `MemoryLibraryRegistry::with_folder_names` stands in for a desktop in tests. The plan row named the `LibraryLocations` port. The names went on `LibraryRegistry` instead, because the facts already read the free space through `LibraryRegistry::free_space`, and both reads now share one blocking thread (`read_facts`).
- **Port for the choice.** `LocationChooser::choose` answers `ChosenFolder { location_id, names }`. The route journals only the location ID and answers both names after `made_safe`. They are `null` when the dialog was cancelled or the name is unreadable.
- **Answers.** `LocationChoice` and `LibraryFacts` carry `folder_name` and `volume_name`. The facts answer them for the library's folder, `null` when it cannot be reached now. The rename answer carries the same facts.
- **Desktop reader (`filesquid-native::folder_names`, `desktop` feature).** `read_folder_names` canonicalises the folder; a folder that cannot be reached answers no names. The folder name is its own last component. The volume name comes from the mount point, found with the standard library only: the topmost ancestor on the same device (`MetadataExt::dev`). The mount point is answered only once verified: the next ancestor is on another device, or it is the root. An ancestor whose metadata cannot be read, or a folder more than 256 levels deep, answers no mount point, so an ordinary folder is never taken for a volume (review round 1). A mount point where the operating system mounts external or removable volumes is named after the volume, so its last component is the volume's name. That covers `/Volumes/<name>` on macOS, and `/media/<name>`, `/media/<user>/<name>`, `/run/media/<user>/<name>` and `/mnt/<name>` on Linux. Any other mount answers `null`, including the system disk. Windows answers `null` too, because it has no Unix device numbers.
- **Desktop chooser.** `DesktopChooser::choose` registers the picked folder, then reads its names through `HostLocations::folder_names`.
- **Contract.** OpenAPI 0.38.0: `folder_name` and `volume_name` (`["string", "null"]`, 1 to 1,024 bytes) on `LocationChoice` and `LibraryFacts`, and updated route descriptions. Both properties are optional in the schema, so clients built against an earlier minor version stay valid. Every host built from 0.38.0 always sends them. `ui/src/api/schema.d.ts` and `contract-version.ts` are regenerated, and the UI package's typecheck, lint and tests pass unchanged.
- **Feature file.** `library_location_names_API-S-034.feature` is now bound: all 6 scenarios have step definitions in `support/location_names_steps.rs`.

## Notes for the owner and the UI lane

- **The system disk has no volume name.** A volume name is answered for external and removable volumes only. Naming the system disk would need a platform call on macOS (`getattrlist` `ATTR_VOL_NAME`). That cannot be compile-checked in this repository's Linux environment, which has no macOS target installed. The SRS allows `null` here: "each `null` when the host cannot read it".
- **The server.** `filesquid-server` keeps no library registry, so its facts route still answers `503 unavailable`, as before this task. The row's "its facts carry both names as `null`" holds for any registry the server installs later, through the trait default. The test for it is `a_host_that_reads_no_names_answers_both_as_null`.
- **UI-F2.22** can read `folder_name` and `volume_name` from `LocationChoice` and `LibraryFacts`.

## SRS mapping

## API-S-034: Folder and volume names

On the desktop host, the answer for a chosen folder carries `folder_name` (the last path component only, after API-S-005's safe transformation) and `volume_name`, each `null` when unreadable. Every other host answers no name. The names never reach a journal entry, a refusal, an event or a log line.

| Constraint | Tests |
|---|---|
| A chosen folder answers its location ID with both names | `unit_location_choice::a_chosen_folder_is_answered_with_its_own_name_and_volume_and_nothing_else_names_them`, scenario "A chosen folder is answered with its name and volume", desktop `the_owner_picks_a_folder_in_the_hosts_dialog_and_the_app_sees_only_its_id` |
| The last path component only, safely transformed; no path or parent folder | `unit_location_choice::the_route_answers_names_only_after_the_safe_transformation`, `integration_library_facts::folder_names_pass_the_safe_transformation_of_api_s_005`, native `a_folder_answers_its_own_name_and_the_volume_of_its_mount_point`, step "answer should carry no path and no parent folder" |
| An unreadable volume name is `null` | scenario "A volume whose name cannot be read is null", native `the_volume_name_is_the_mount_point_name_of_an_external_volume_only` |
| An unverified mount point gives no volume name | native `a_folder_deeper_than_the_ancestor_cap_answers_no_mount_point_and_no_volume` |
| A cancelled choice carries no names | `unit_location_choice::a_cancelled_dialog_answers_cancelled_and_journals_it` |
| Not in a journal entry, event or log line | the unit test above (with a live log recorder), scenario "The names never reach the journal, a refusal or an event", desktop `the_facts_name_the_library_folder_until_it_cannot_be_reached` |
| A host without a chooser answers `503` | `unit_location_choice::without_a_dialog_or_with_an_unreadable_body_nothing_is_asked` (unchanged) |

## API-S-038: Library facts

The facts answer `folder_name` and `volume_name` on the desktop, `null` when unreadable or when the folder cannot be reached now, and always `null` on every other host.

| Constraint | Tests |
|---|---|
| The facts and rename answers name the folder and volume | `integration_library_facts::the_desktop_facts_name_the_library_folder_and_its_volume_and_never_journal_them`, scenario "The library facts name the library's folder and volume", desktop `a_named_library_answers_its_facts_and_is_listed_as_recent_without_its_folder` |
| `null` when unreadable or unreachable | `integration_library_facts::an_unreadable_volume_or_an_unreachable_folder_answers_null_names`, scenario "A library folder that cannot be reached now has no names", desktop `the_facts_name_the_library_folder_until_it_cannot_be_reached`, native `the_registry_answers_a_registered_folder_s_names_and_none_once_it_is_gone` |
| Always `null` on every other host | `integration_library_facts::a_host_that_reads_no_names_answers_both_as_null`, scenario "A host that is not the owner's desktop answers no name" |
| Contract | `unit_contract` (version 0.38.0, schema examples including both names) |

## API-S-005: Opaque handles and display names

The folder and volume names pass the one safe transformation of API-S-005: control characters dropped, `/` and `\` turned into `_`, surrounding spaces trimmed, at most 1,024 bytes kept, and a name with nothing left answered as `null`. Tests: the safe-transformation tests listed under API-S-034.

## DE-S-017: Native library folder choice

The desktop answers the chosen folder's own name and its volume's name, never a parent folder or the path. Tests: desktop `the_owner_picks_a_folder_in_the_hosts_dialog_and_the_app_sees_only_its_id` and `a_cancelled_or_unusable_pick_registers_nothing`, and native `unit_folder_names`.

## DE-S-006: No host path reaches the webview

No answer carries a path or a parent folder. Tests: the "no path" assertions in the desktop tests and the API step "answer should carry no path and no parent folder".

## Test-first evidence

The API, native and desktop tests were run before the implementation and failed to compile against the missing items:

```text
error[E0432]: unresolved import `filesquid_api::local_library::FolderNames`
error[E0432]: unresolved import `filesquid_api::local_library::choice::ChosenFolder`
error[E0599]: no method named `with_folder_names` found for struct `MemoryLibraryRegistry` in the current scope
error[E0599]: no method named `folder_names` found for struct `Arc<MemoryLibraryRegistry>` in the current scope
error[E0432]: unresolved import `filesquid_native::folder_names`
error[E0053]: method `choose` has an incompatible type for trait
```

No `Task-API-F2.31-Plan.md` or `-ToDos.md` was written for this task. The SRS constraints and their tests are listed in the tables above.

## Test output

```text
$ cargo test -p filesquid-api --locked   (excerpt: the suites this task changed; whole run exit 0)
running 6 tests
test a_picked_folder_answers_its_opaque_location_id_only ... ok
test a_chosen_folder_is_answered_with_its_own_name_and_volume_and_nothing_else_names_them ... ok
test a_cancelled_dialog_answers_cancelled_and_journals_it ... ok
test the_route_answers_names_only_after_the_safe_transformation ... ok
test refusals_are_typed_and_journaled_once_with_their_code ... ok
test without_a_dialog_or_with_an_unreadable_body_nothing_is_asked ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
running 16 tests
test a_host_that_reads_no_names_answers_both_as_null ... ok
test a_caller_who_may_not_read_the_repository_reads_no_library_facts ... ok
test a_library_is_created_with_its_name_and_the_facts_answer_it ... ok
test a_library_this_host_remembers_but_has_not_opened_now_is_listed_as_not_open ... ok
test a_library_created_without_a_name_is_untitled_and_open_may_name_it ... ok
test a_name_that_breaks_the_rules_is_refused_before_anything_is_created ... ok
test a_name_given_on_open_that_replaces_another_is_journaled_as_a_rename ... ok
test a_rename_of_an_unknown_library_or_to_a_bad_name_is_refused_once ... ok
test folder_names_pass_the_safe_transformation_of_api_s_005 ... ok
test an_unknown_or_unreadable_library_is_not_found_and_journaled_once ... ok
test an_unreadable_volume_or_an_unreachable_folder_answers_null_names ... ok
test a_rename_is_journaled_with_the_library_only_and_a_same_name_changes_nothing ... ok
test free_bytes_are_null_with_their_reason_when_the_volume_cannot_be_read ... ok
test without_a_registry_the_facts_and_recent_routes_are_unavailable ... ok
test the_desktop_facts_name_the_library_folder_and_its_volume_and_never_journal_them ... ok
test recent_lists_at_most_20_readable_libraries_newest_first_without_paths ... ok
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
Feature: The owner's library folder and volume names on the desktop
  Scenario: A chosen folder is answered with its name and volume
   ✔  Given location-names desktop host whose dialog chooses the folder "Family archive" on the volume "Photos SSD"
   ✔  When location-names owner asks for a location for the purpose create
   ✔  Then location-names answer should carry a location ID, folder_name "Family archive" and volume_name "Photos SSD"
   ✔  And location-names answer should carry no path and no parent folder
  Scenario: The library facts name the library's folder and volume
   ✔  Given location-names library opened from the folder "Family archive" on the volume "Photos SSD"
   ✔  When location-names owner reads the library facts
   ✔  Then location-names facts should carry folder_name "Family archive" and volume_name "Photos SSD"
  Scenario: A volume whose name cannot be read is null
   ✔  Given location-names desktop host that cannot read the volume name of the chosen folder
   ✔  When location-names owner asks for a location for the purpose open
   ✔  Then location-names answer should carry folder_name and a null volume_name
  Scenario: A library folder that cannot be reached now has no names
   ✔  Given location-names remembered library whose folder cannot be reached
   ✔  When location-names owner reads the library facts
   ✔  Then location-names facts should carry a null folder_name and a null volume_name
  Scenario: A host that is not the owner's desktop answers no name
   ✔  Given location-names host without folder names
   ✔  When location-names caller reads the library facts
   ✔  Then location-names facts should carry a null folder_name and a null volume_name
  Scenario: The names never reach the journal, a refusal or an event
   ✔  Given location-names desktop host whose dialog chooses the folder "Tax 2026" on the volume "Backup"
   ✔  When location-names owner asks for a location and then creates a library there
   ✔  Then location-names journal, events and log lines should contain neither "Tax 2026" nor "Backup"
   ✔  And location-names journal should name only the opaque location and library IDs
571 scenarios (388 passed, 183 skipped)
2015 steps (1832 passed, 183 skipped)

$ cargo test -p filesquid-native --features desktop --test unit_folder_names --locked
running 3 tests
test the_volume_name_is_the_mount_point_name_of_an_external_volume_only ... ok
test a_folder_answers_its_own_name_and_the_volume_of_its_mount_point ... ok
test the_registry_answers_a_registered_folder_s_names_and_none_once_it_is_gone ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 15 tests

$ cargo test -p filesquid-desktop --features test-webdriver --locked   (excerpt; whole run exit 0)
71 scenarios (71 passed)
335 steps (335 passed)
test a_cancelled_or_unusable_pick_registers_nothing ... ok
test a_named_library_answers_its_facts_and_is_listed_as_recent_without_its_folder ... ok
test the_facts_name_the_library_folder_until_it_cannot_be_reached ... ok
test the_owner_picks_a_folder_in_the_hosts_dialog_and_the_app_sees_only_its_id ... ok

$ npx vitest run   (apps/filesquid/ui, against the regenerated types)
 Test Files  126 passed (126)
      Tests  2874 passed | 1 skipped (2875)
```

Clippy (`-p filesquid-api -p filesquid-native -p filesquid-desktop --all-targets --features filesquid-desktop/test-webdriver -- -D warnings`), the UI package's `tsc --noEmit` and ESLint all exit 0.

Coverage: the repository sets no numeric coverage threshold; every constraint above has a unit, integration or Cucumber test.

## Mocked Environment Variables

None.

## Pre-commit hook

The head-commit run is `FULL=1 bash scripts/pre-commit.sh`. Its summary line (`Test scope: full`) is recorded in the pull request body, because the run follows the last commit.
