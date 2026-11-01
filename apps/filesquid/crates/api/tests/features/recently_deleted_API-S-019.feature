# API-S-019: Library domain commands and queries: one Recently deleted, restore, retention and
# expiry (Task API-F2.15; ADR 041 Decisions 8 to 11)
# API-S-021: Navigation listing (the one recently_deleted node; navigation_listing_API-S-021.feature)
# PRD F-008 (deletion, hidden files and duplicate review), F-065 (version history and retention)
Feature: One Recently deleted, restore, retention and expiry
  Deletion is a state, not a place: a file deleted on its own and the files of a deleted
  collection keep their object, logical path and home collection and wait in one Recently
  deleted until their library's retention ends, then expire as a recorded purge.

  Scenario: A file deleted on its own and the files of a deleted collection share one Trash
    Given a library with a collection "Site visits" holding 2 files
    When the owner deletes the first file on its own
    And the owner deletes the collection "Site visits"
    Then the first file should be deleted one by one
    And the second file should be deleted with its collection
    And every deleted file should remember the collection title "Site visits"

  Scenario: A restore whose home collection is gone creates the first free untitled collection
    Given a library with a collection "Site visits" holding 1 file
    And a collection titled "untitled collection" already exists
    And the owner deletes the collection "Site visits"
    When the owner restores the file
    Then the file should be restored into a new collection titled "untitled collection 2"
    And it should keep its place in its object

  Scenario: A restore is refused when every untitled title is taken
    Given a library with a collection "Site visits" holding 1 file
    And the owner deletes the collection "Site visits"
    And every untitled collection title up to 9999 is taken
    When the owner restores the file
    Then the command should be refused as "conflict" with the code "restore_name_exhausted"
    And the file should still be deleted

  Scenario: A library's own retention never moves an expiry already shown
    Given a library with a collection "Trips" holding 2 files
    And the owner deleted the first file a day earlier
    When the owner sets the library's retention to 90 days
    And the owner deletes the second file on its own
    Then the first file should keep its earlier deletion times
    And the second file should be purged 90 days after deletion

  Scenario Outline: A retention outside 1 to 3,650 days is refused
    Given a library with a collection "Trips" holding 1 file
    When the owner sets the library's retention to <days> days
    Then the command should be refused as "invalid_request" with the code "invalid_retention"

    Examples:
      | days |
      | 0    |
      | 3651 |

  Scenario: Expired files leave every view and their object work is recorded
    Given a library with a collection "Trips" holding 2 files
    And the owner has deleted every file
    When the expiry runs 31 days later under "keep_version_history"
    Then no file should be listed in the deleted view
    And each expired file's object should become a tombstone
    And the purge should be journaled once with opaque IDs only

  Scenario: A failed object purge is journaled as failed and retried at the next run
    Given a library with a collection "Trips" holding 1 file
    And the owner has deleted every file
    And the next object purge fails
    When the expiry runs 31 days later under "delete_all_version_history"
    Then the object purge should be journaled as failed
    When the expiry runs 31 days later under "delete_all_version_history"
    Then each expired file's object should be deleted
