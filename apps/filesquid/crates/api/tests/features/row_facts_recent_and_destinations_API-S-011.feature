# API-S-011: Row facts — every file row of the view and of the search carries library_id, the head
# version and the home collection_id with that collection's title (null for both without a
# collection); every folder row carries the files and bytes beneath it, marked incomplete when a
# bound stopped the count; collection rows carry their collection_id. Recent — view=recent
# lists the caller's own changed, opened and uploaded files, newest first, with the action kind.
# Destination refusals carry their rule code as an ApiError sub-code. The path a write used is
# answered and recorded in the collection snapshot version, never in the journal.
# API-S-012: search rows carry the same facts. API-S-021: collection and folder navigation nodes
# carry collection_id, and the destination route resolves them. API-S-014: history_bytes beside
# the library's total, the bytes on disk held only by versions other than each object's head,
# each distinct content counted once.
# Owner decisions 2026-10-31, verbatim: "accept recommendations" (decisions 2, 4 and 14) and "well do a" (history bytes).
# The folder rows of a view are its collections and virtual collections (the view lists a
# library, collection or virtual-collection node, API-S-011); folders inside a collection are
# navigation nodes (API-S-021).
Feature: Row facts, the Recent view and destination identifiers
  As a library owner
  I want every row to name its library, version and collection
  So that the Drive can act on a file and name a destination without guessing

  Background:
    Given row-facts fixture: a library with collection "Holidays" holding "Beach.jpg" at version v2 and 3 files of 1000 bytes each in its folder "Trips"

  # Happy path
  Scenario: A view file row names its library, head version and home collection
    When row-facts caller reads the view of the collection
    Then row-facts row "Beach.jpg" should carry the library ID, head "v2", the collection ID and the title "Holidays"

  Scenario: A search file row carries the same facts
    When row-facts caller searches the repository for "beach"
    Then row-facts row "Beach.jpg" should carry the library ID, head "v2", the collection ID and the title "Holidays"

  Scenario: A folder row counts the files and bytes beneath it
    When row-facts caller reads the view of the library
    Then row-facts folder row "Holidays" should report 4 files and 3004 bytes, complete
    And row-facts folder row "Holidays" should carry the collection ID

  Scenario: Collection and folder navigation nodes carry their collection ID
    When row-facts caller lists the navigation children of the library
    Then row-facts node "Holidays" should carry the collection ID
    When row-facts caller lists the navigation children of the collection
    Then row-facts node "Trips" should carry the collection ID

  Scenario: A collection node resolves to the destination an upload names
    When row-facts caller asks the destination of the collection node
    Then row-facts destination should be the library ID and the collection ID, with no folder

  Scenario: A folder node resolves to its collection and folder
    When row-facts caller asks the destination of the folder node "Trips"
    Then row-facts destination should be the library ID, the collection ID and the folder "Trips"

  Scenario Outline: A node that cannot be read gets the listing's one not_found answer
    When row-facts caller asks the destination of <node>
    Then row-facts answer should be the same not_found answer as a navigation listing of an unknown node

    Examples:
      | node                             |
      | an unknown node                  |
      | a malformed handle               |
      | a node the caller may not read   |
      | a folder that no longer exists   |

  Scenario: The Recent view lists the caller's own actions newest first
    Given row-facts caller uploaded "a.txt", then opened "Beach.jpg", then renamed "t1.jpg" to "Harbour.jpg"
    When row-facts caller reads the Recent view
    Then row-facts first rows should be "Harbour.jpg" changed, "Beach.jpg" opened and "a.txt" uploaded, in that order

  Scenario: The library's storage total names the bytes only older versions keep
    Given row-facts history fixture: "a.bin" whose v1 content of 400 bytes differs from its head and "b.bin" whose v1 and head share one content
    When row-facts caller reads the library history size
    Then row-facts history bytes should be 400

  Scenario: Content shared by two older versions counts once as history
    Given row-facts history fixture: "plan.pdf" whose v1 and v2 hold one content of 500 bytes and whose head v3 holds another
    When row-facts caller reads the library history size
    Then row-facts history bytes should be 500

  # Edge cases
  Scenario: A file in no collection carries null collection facts
    Given row-facts fixture: a file "loose.bin" registered in no collection
    When row-facts caller searches the repository for "loose"
    Then row-facts row "loose.bin" should carry a null collection ID and a null collection title

  Scenario: A folder count stopped by a bound says so
    Given row-facts fixture: the collection lists more files than one view may count
    When row-facts caller reads the view of the library
    Then row-facts folder row "Holidays" should be marked incomplete

  # Error and privacy cases
  Scenario Outline: A destination refusal carries its rule code as a sub-code
    When row-facts caller uploads with <destination>
    Then row-facts answer should be HTTP <status> with sub-code "<subcode>"

    Examples:
      | destination           | status | subcode              |
      | no collection         | 400    | destination_required |
      | an unknown collection | 404    | collection_not_found |

  Scenario: An upload whose numbers are all taken carries path_suffix_exhausted
    Given row-facts fixture: "full.txt" and "full-1.txt" through "full-9999.txt" are taken in the collection
    When row-facts caller uploads "full.txt" into the collection
    Then row-facts answer should be HTTP 409 with sub-code "path_suffix_exhausted"

  Scenario: A node that is not a collection or folder is no destination
    When row-facts caller asks the destination of the library node
    Then row-facts answer should be refused as invalid_request

  Scenario: A restore with no free untitled collection name carries its sub-code
    Given row-facts fixture: a deleted file whose collection is gone and every "untitled collection" title up to the cap is taken
    When row-facts caller restores it
    Then row-facts answer should be HTTP 409 with sub-code "restore_name_exhausted"

  Scenario: The Recent view never shows another caller's actions
    Given row-facts second caller opened "Beach.jpg"
    When row-facts caller reads the Recent view
    Then row-facts rows should not include the second caller's action

  Scenario: The path a numbered upload used is answered and kept out of the journal
    When row-facts caller uploads "Beach.jpg" into the collection
    Then row-facts answer should name the path "Beach-1.jpg"
    And row-facts snapshot registration of the upload should record the path "Beach-1.jpg"
    And row-facts journal should hold opaque IDs only
