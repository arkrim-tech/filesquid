# F-061: Activity journal and live health monitoring
# SRS-F-061: Activity journal and live health monitoring
# API-S-030: Client events; API-S-031: Journal query and export; API-S-032: Durable journal
# DE-S-013: Journal sealing and rebuild; UI-S-021: Logs view; UI-S-023: Client event delivery
# Scenarios only; step definitions arrive with Task FS-2.9 (unbound steps are skipped by the harness).
Feature: Durable, complete and browsable activity journal
  As a library owner
  I want the journal to survive the loss of the database and to be easy to browse and export
  So that the record of everything FileSquid did is never lost

  Background:
    Given a desktop library with recorded activity

  # Happy path
  Scenario: A committed operation carries its journal entries in the same OCFL commit
    When I rename a file
    Then the new OCFL version should contain the operation's journal entries beside its PROV-O record
    And each entry should carry the operation ID and the object reference of the new version

  Scenario: Other events are sealed every hour
    Given an authorisation refusal and a health change were journaled
    When the hourly seal runs
    Then the library's journal object should gain one JSON Lines segment with those entries
    And the seal should be a registered, journaled process

  Scenario: Library events are sealed on library close
    Given library entries that are not yet sealed
    When I close the library
    Then they should be sealed into the library's journal object

  Scenario: Host events are sealed on app close
    Given host entries that are not yet sealed
    When I close the app
    Then they should be sealed into the installation's journal object

  Scenario: Database deleted, rebuild restores the journal
    Given committed operations and other entries sealed at the last hourly seal
    And entries that changed nothing written after that seal
    When the operational database is deleted and I rebuild the library from its files
    Then the journal should contain every committed operation's entries
    And every entry up to the last seal
    And no entry twice
    And only entries that changed nothing after the last seal should be missing

  # Edge cases
  Scenario: A failed seal drops nothing
    Given the library's storage root is not writable during a seal
    When the next seal runs after the storage root is writable again
    Then every entry that was not sealed should be in the new segment

  Scenario: A browser console warning reaches the journal
    When the UI reports a console warning
    Then the journal should hold one "ui.console.warn" entry with the component, error code and opaque IDs

  Scenario: Browser console errors reach the journal without their text
    When the UI reports a console error with a message that contains a file name
    Then the journal should hold one "ui.console.error" entry with the component, error code and opaque IDs
    And the entry should not contain the message text

  Scenario: Client events are bounded
    When the UI sends 21 events in one request
    Then the request should be refused with an error code
    And a caller over the rate bound should receive status 429 with a Retry-After header

  Scenario: Rate limits delay client events but never merge them
    Given the UI has 150 console warnings queued
    When the route answers 429 with a Retry-After header
    Then the UI should retry after the Retry-After delay with backoff
    And the journal should eventually hold 150 "ui.console.warn" entries, one per event
    And no "ui.console.overflow" entry should be written

  Scenario: Only events still queued when the page closes are counted
    Given the UI has 3 console errors queued that the route has not yet accepted
    When the page closes
    Then the journal should hold one "ui.console.overflow" entry with the count 3
    And every event delivered before the page closed should have its own journal entry

  # User interactions
  Scenario: The owner browses and filters the Logs view
    When I open the Logs view and filter by operation "metadata.edit" and the last day
    Then I should see matching entries newest first with paging
    And opening an entry should show its detail and a link to its PROV-O activity

  Scenario: The owner exports the journal as CSV
    When I export the journal filtered by library and level "warn" as CSV
    Then a CSV file with one row per matching entry and opaque IDs only should be downloaded
    And the journal should record a "journal.export" entry

  # Error case
  Scenario: A non-owner cannot export the journal
    Given a caller without the owner credential
    When it requests the journal export
    Then the request should be refused and journaled as a refusal

  Scenario: A non-owner cannot open the Logs view
    Given a caller without the owner credential
    When it opens the Logs view
    Then the tab should show a permission state and no entries

  # Source
  # - resources/srs.md SRS-F-061
  # - resources/prd.md F-061, F-001, F-042
  # - apps/filesquid/crates/api/resources/srs.md API-S-030, API-S-031, API-S-032
  # - apps/filesquid/crates/desktop/resources/srs.md DE-S-013
  # - apps/filesquid/ui/resources/srs.md UI-S-021
