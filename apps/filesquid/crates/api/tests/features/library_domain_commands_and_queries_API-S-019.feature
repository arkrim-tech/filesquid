# API-S-019: Library domain commands and queries
# PRD F-001 (independent local library), F-049 (terminology and item type), F-038 (open
# interfaces), F-004 (collections retain files), F-008 (hidden files and recoverable deletion)
Feature: Library domain commands and queries
  The application layer in filesquid-api defines one set of versioned commands and queries
  for a library. Hosts inject the catalog store, so no filesystem path or SQL crosses the API,
  and every change reaches the catalog with its journal entry in one commit.

  Scenario: Create and open a library without an account or a connection
    Given a FileSquid application layer with an empty catalog
    When the owner creates a library named "Family archive"
    Then the library should have a UUID version 4 identifier
    And opening the library should answer the name "Family archive" at revision "1"

  Scenario: Deleting a collection deletes its files in place
    Given a library with a collection "Trips" holding 2 files
    When the owner deletes the collection "Trips"
    Then the ordinary listing should show 0 files
    And the deleted view should show 2 files
    And every deleted file should keep its home collection "Trips"
    And no file should be listed outside its home collection

  Scenario: Deleting a collection keeps the times of a file already deleted
    Given a library with a collection "Trips" holding 2 files
    And the owner deleted the first file a day earlier
    When the owner deletes the collection "Trips"
    Then the first file should keep its earlier deletion times
    And the second file should be deleted now, to be purged 30 days later

  Scenario: A library's own retention period sets the purge-after time
    Given a library with a collection "Trips" holding 1 file
    And the library keeps deleted files for 7 days
    When the owner deletes the collection "Trips"
    Then the deleted view should show 1 file to be purged 7 days after deletion

  Scenario: Removing a file from a virtual collection keeps its other memberships
    Given a library with a collection "Trips" holding 1 file
    And the file is a member of the virtual collections "Best" and "Family"
    When the owner removes the file from "Best"
    Then the collection "Trips" should still list the file
    And the virtual collection "Family" should still list the file
    And the virtual collection "Best" should list no file

  Scenario: A rule-based virtual collection refuses hand-picked members
    Given a library with a collection "Trips" holding 1 file
    And a rule-based virtual collection "Recent"
    When the owner adds the file to "Recent"
    Then the command should be refused as "invalid_request"

  Scenario: Hidden and deleted files leave the ordinary listing
    Given a library with a collection "Trips" holding 3 files
    When the owner hides the first file and deletes the second
    Then the ordinary listing should show 1 file
    And the hidden view should show 1 file
    And the deleted view should show 1 file to be purged 30 days after deletion

  Scenario: Permanent deletion needs a confirmation of the exact scope
    Given a library with a collection "Trips" holding 2 files
    And the owner has deleted every file
    When the owner permanently deletes every file confirming 1 file
    Then the command should be refused as "invalid_request"
    And the deleted view should show 2 files

  Scenario: A command against a stale revision is a conflict and changes nothing
    Given a library with a collection "Trips" holding 1 file
    When the owner renames the collection "Trips" with a stale revision
    Then the command should be refused as "conflict"
    And nothing should have been committed

  Scenario: Retrying a command with the same operation ID changes nothing
    Given a library with a collection "Trips" holding 1 file
    When the owner sends the same create-collection command twice
    Then both answers should name the same collection
    And the library should list 2 collections

  Scenario: A command that changes nothing stores nothing
    Given a library with a collection "Trips" holding 1 file
    When the owner renames the collection "Trips" to "Trips" as operation "op-same-title"
    Then the command should answer committed at the current revision
    And the catalog should hold no receipt for operation "op-same-title"
    And nothing should have been committed
    And no journal entry should have been stored

  Scenario: A retried command that changed nothing is planned again
    Given a library with a collection "Trips" holding 1 file
    And the owner has renamed the collection "Trips" to "Trips" as operation "op-same-title"
    When the owner renames the collection "Trips" to "Holidays"
    And the owner retries operation "op-same-title"
    Then the command should be refused as "conflict"

  Scenario: Paging visits every file once in a stable order
    Given a library with a collection "Trips" holding 7 files
    When the owner lists the library 3 files per page
    Then every file should appear exactly once in identifier order
    And a query asking for 1001 items per page should be refused

  Scenario: The journal records every change without names or titles
    Given a library with a collection "Holiday in Lisbon" holding 1 file
    When the owner renames the collection "Holiday in Lisbon" to "Secret plans"
    Then the catalog should hold one journal entry per commit
    And no journal entry should contain "Lisbon" or "Secret plans"
