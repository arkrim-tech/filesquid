# API-S-011: Row facts — every file row of the view and of the search carries library_id, the head
# version and the home collection_id with that collection's title (null for both without a
# collection); every folder row carries the files and bytes beneath it, marked incomplete when a
# bound stopped the count; collection and folder rows carry their collection_id. Recent — view=recent
# lists the caller's own changed, opened and uploaded files, newest first, with the action kind.
# Destination refusals carry their rule code as an ApiError sub-code. The path a write used is
# answered and recorded in the collection snapshot version, never in the journal.
# API-S-012: search rows carry the same facts. API-S-021: collection and folder navigation nodes
# carry collection_id. API-S-014: history_bytes beside the library's total, the bytes on disk held
# only by versions other than each object's head, each distinct content counted once.
# Owner decisions 2026-10-31, verbatim: "accept recommendations" (decisions 2, 4 and 14) and "well do a" (history bytes).
# Scenarios only; Task API-F2.21 adds their step definitions (unmatched steps are reported as skipped).
Feature: Row facts, the Recent view and destination identifiers
  As a library owner
  I want every row to name its library, version and collection
  So that the Drive can act on a file and name a destination without guessing

  Background:
    Given row-facts fixture: a library "Family" with collection "Summer" holding "beach.jpg" at version v2 and a folder "Trips" of 3 files totalling 3000 bytes

  # Happy path
  Scenario: A view file row names its library, head version and home collection
    When row-facts caller reads the view of collection "Summer"
    Then row-facts row "beach.jpg" should carry the library ID of "Family", head "v2", the collection ID of "Summer" and the title "Summer"

  Scenario: A search file row carries the same facts
    When row-facts caller searches the repository for "beach"
    Then row-facts row "beach.jpg" should carry the library ID of "Family", head "v2", the collection ID of "Summer" and the title "Summer"

  Scenario: A folder row counts the files and bytes beneath it
    When row-facts caller reads the view of collection "Summer"
    Then row-facts folder row "Trips" should report 3 files and 3000 bytes, complete

  Scenario: Collection and folder navigation nodes carry their collection ID
    When row-facts caller lists the navigation children of library "Family"
    Then row-facts node "Summer" should carry the collection ID of "Summer"
    And row-facts node of folder "Summer/Trips" should carry the collection ID of "Summer"

  Scenario: A collection or folder node resolves to the destination an upload names
    When row-facts caller asks the destination of the navigation node of folder "Summer/Trips"
    Then row-facts destination should be the library ID of "Family", the collection ID of "Summer" and the folder "Trips"

  Scenario: The Recent view lists the caller's own actions newest first
    Given row-facts caller uploaded "a.txt", then opened "beach.jpg", then renamed "notes.md"
    When row-facts caller reads the Recent view
    Then row-facts rows should be "notes.md" changed, "beach.jpg" opened, "a.txt" uploaded, in that order

  Scenario: The library's storage total names the bytes only older versions keep
    Given row-facts fixture: "beach.jpg" whose v1 content of 400 bytes differs from its head and "notes.md" whose v1 and head share one content
    When row-facts caller reads the library history size of "Family"
    Then row-facts history bytes should be 400, counting the shared content of "notes.md" as no history

  Scenario: Content shared by two older versions counts once as history
    Given row-facts fixture: "plan.pdf" whose v1 and v2 hold one content of 500 bytes and whose head v3 holds another
    When row-facts caller reads the library history size of "Family"
    Then row-facts history bytes should be 500, the shared content counted once

  # Edge cases
  Scenario: A file in no collection carries null collection facts
    Given row-facts fixture: a file "loose.bin" registered in no collection
    When row-facts caller searches the repository for "loose"
    Then row-facts row "loose.bin" should carry a null collection ID and a null collection title

  Scenario: A folder count stopped by a bound says so
    Given row-facts fixture: folder "Summer/Big" holds more files than one view may count
    When row-facts caller reads the view of collection "Summer"
    Then row-facts folder row "Big" should be marked incomplete

  # Error and privacy cases
  Scenario Outline: A destination refusal carries its rule code as a sub-code
    When row-facts caller uploads with <destination>
    Then row-facts answer should be HTTP <status> with sub-code "<subcode>"

    Examples:
      | destination           | status | subcode              |
      | no collection         | 400    | destination_required |
      | an unknown collection | 404    | collection_not_found |

  Scenario: An upload whose numbers are all taken carries path_suffix_exhausted
    Given row-facts fixture: "Summer/beach.jpg" and "Summer/beach-1.jpg" through "Summer/beach-9999.jpg" are taken
    When row-facts caller uploads "beach.jpg" into collection "Summer"
    Then row-facts answer should be HTTP 409 with sub-code "path_suffix_exhausted"

  Scenario: A node that is not a collection or folder is no destination
    When row-facts caller asks the destination of the navigation node of library "Family"
    Then row-facts answer should be refused as invalid_request

  Scenario: A restore with no free untitled collection name carries its sub-code
    Given row-facts fixture: a deleted file whose collection is gone and every "untitled collection" title up to the cap is taken
    When row-facts caller restores it
    Then row-facts answer should carry sub-code "restore_name_exhausted"

  Scenario: The Recent view never shows another caller's actions
    Given row-facts second caller opened "beach.jpg"
    When row-facts caller reads the Recent view
    Then row-facts rows should not include the second caller's action

  Scenario: The path a numbered upload used is answered and kept out of the journal
    Given row-facts fixture: "Summer/beach.jpg" is taken
    When row-facts caller uploads "beach.jpg" into collection "Summer"
    Then row-facts answer should name the path "beach-1.jpg"
    And row-facts snapshot version of the operation should record the path "beach-1.jpg"
    And row-facts journal entry of the operation should hold opaque IDs only
