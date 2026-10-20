# F-061: Activity journal and live health monitoring
# SRS-F-061: Activity journal and live health monitoring
# API-S-030: Client events; API-S-031: Journal query and export; API-S-032: Durable journal
# DE-S-013: Journal sealing and rebuild; UI-S-021: Logs view; UI-S-023: Client event delivery
# The client-event, query and export scenarios are bound in tests/support/journal_steps.rs
# (Task API-F2.10); the sealing, rebuild and UI scenarios are bound by the later FS-2.9 tasks
# (unbound steps are skipped by the harness).
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

  Scenario: A full client queue spills to IndexedDB and loses nothing
    Given the host has been unreachable and the UI's in-memory queue holds 10,000 console warnings
    When 5 more console warnings occur
    Then the 5 warnings should be stored in the page's IndexedDB store
    And when the host is reachable again the journal should eventually hold 10,005 "ui.console.warn" entries, one per event
    And no "ui.console.overflow" entry should be written

  Scenario: Events queued at page close are persisted and delivered on the next open
    Given the UI has 2 console errors already delivered and 3 console errors the route has not yet accepted
    When the page closes
    Then the 3 undelivered events should be persisted to IndexedDB
    And when FileSquid opens again the journal should hold 3 more "ui.console.error" entries, delivered oldest first before any new event
    And no event should be journaled twice
    And no "ui.console.overflow" entry should be written

  Scenario: A refused IndexedDB write after the queue is full is counted at page close
    Given the host has been unreachable and the UI's in-memory queue holds 10,000 console warnings
    And the page's IndexedDB store refuses further writes because its 50 MiB budget is reached
    When 4 more console warnings occur and the page then closes
    Then the journal should eventually hold one "ui.console.overflow" entry with the count of events the UI could not keep
    And every event that was kept should have its own "ui.console.warn" entry

  Scenario: Persisted events are never delivered under another account's credential
    Given "ana" left 3 undelivered console errors persisted on this device for the server host
    When "ben" signs in on the same device and FileSquid opens
    Then none of Ana's 3 events should be sent with Ben's credential
    And they should be delivered, each as its own entry, when "ana" signs in again

  Scenario: Without IndexedDB the events the UI could not keep are counted at page close
    Given IndexedDB is unavailable in the page
    And the UI has 2 console errors already delivered and 3 console errors the route has not yet accepted
    When the page closes
    Then the journal should hold one "ui.console.overflow" entry with the count 3
    And the 2 delivered events should each have their own "ui.console.error" entry

  Scenario: A client event resent after a lost acknowledgement adds no entry
    When the UI sends a console warning twice with the same client event ID
    Then both requests should be acknowledged
    And the journal should hold one "ui.console.warn" entry for that client event ID

  Scenario: An event raised by another principal is refused
    When the UI sends a console warning raised by another principal
    Then the request should be refused as not permitted and journaled as a refusal
    And no "ui.console.warn" entry should be written

  Scenario: The overflow count is bounded and exempt from the rate bound
    Given the caller has used up the rate bound
    When the UI sends one overflow event with the count 1000000
    Then the journal should hold one "ui.console.overflow" entry with the count 1000000
    And an overflow event with the count 0 should be refused with an error code

  Scenario: The journal filters by file and by operation ID
    When I filter the journal by the file
    Then I should see only the entries about that file
    When I filter the journal by the operation ID of the rename
    Then I should see only the entries of that operation

  Scenario: An entry links to its PROV-O activity or is marked journal-only
    When I open the journal entry of the committed rename
    Then it should link to the PROV-O record of its object version
    When I open the journal entry of the failed move
    Then it should be marked journal-only

  Scenario: Display names are shown for display and never exported
    Given the host resolves display names for the owner
    When I read the journal with names and export it
    Then the page should show the file's display name
    And the export should carry opaque IDs only

  Scenario: OCFL engine warnings carry the operation and the object version
    When the OCFL engine warns about a lease takeover inside an operation
    Then the journal entry should name the engine operation, the operation ID and the object version

  Scenario: Metrics snapshots are off by default
    When the monitor ticks with its default settings
    Then no "metrics.snapshot" entry should be written

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
    Then the Logs view should show a permission state and no entries

  # Source
  # - resources/srs.md SRS-F-061
  # - resources/prd.md F-061, F-001, F-042
  # - apps/filesquid/crates/api/resources/srs.md API-S-030, API-S-031, API-S-032
  # - apps/filesquid/crates/desktop/resources/srs.md DE-S-013
  # - apps/filesquid/ui/resources/srs.md UI-S-021, UI-S-023
