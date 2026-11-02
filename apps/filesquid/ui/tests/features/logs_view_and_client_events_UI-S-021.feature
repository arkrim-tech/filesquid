# UI-S-021: Logs view (the single place to browse the audit log: newest first with paging, filters
#   by time range, library, operation, outcome and level and by file, object or operation ID, one
#   entry in detail, a link of a committed operation to its Provenance activities, journal-only
#   labels, View in Logs, live updates, export as JSON Lines or CSV, the permission state)
# UI-S-023: Client event delivery (console warnings and errors to POST /api/v1/client-events,
#   a bounded in-page queue with retries, spill and persistence in IndexedDB, the page-close count)
# UI-S-032: Monitor and Logs composition (mockups 38 and 39); UI-S-024: visual constants
# PRD F-061: Activity journal and live health monitoring; SRS-F-061; API-S-030; API-S-031
# Written with the feature-writer conventions (.agents/skills/feature-writer/SKILL-feature-writer.md).
# Task UI-F2.16 runs these against "vite build" plus "vite preview": the Vitest suites
# tests/ui/logs-view.test.ts, log-entry.test.ts, logs-export.test.ts, logs-route.test.ts and
# files-route-open.test.ts, tests/unit/client-event-queue.test.ts and console-forwarding.test.ts,
# and the Playwright specs tests/e2e/preview/logs-view.spec.ts and client-events.spec.ts.
Feature: Logs view and client events
  As the owner of this device
  I want one place to browse everything FileSquid did, and the app's own warnings and errors to be
  journaled without loss
  So that I can answer what happened, to which file and when, and share a support log safely

  Background:
    Given the app runs on the desktop host
    And the journal holds entries of committed operations, failed operations and refusals

  # Happy path
  Scenario: The sidebar offers Logs directly below Settings
    When the shell loads
    Then the sidebar should show a "Logs" item directly below "Settings"
    And opening it should show the Logs view with the Logs row current

  Scenario: The Logs view lists the journal newest first
    When I open the Logs view
    Then I should see the entries of the last 24 hours, newest first
    And each row should show the time, level, operation, outcome, target and a short operation id
    And the line under the table should say how many entries are shown

  Scenario: An entry opens in a 360 px detail panel
    When I select an entry
    Then the detail panel should show the entry's times, level, outcome, actor, library, object, file and operation id
    And the panel region should be 360 px wide

  Scenario: View provenance and Show file open the file's panel
    Given I selected the entry of a committed operation on a file
    When I choose "View provenance"
    Then the file should open with its Provenance tab
    And choosing "Show file" should open it with its File info tab

  Scenario: Opened from View in Logs on a file or an operation
    When I open "#/logs?file=<file id>" or "#/logs?operation_id=<operation id>"
    Then the Logs view should list the entries of that file or operation over all time
    And the search field should show the ID

  Scenario: A live entry is counted and added on request
    Given the Logs view is open
    When the journal publishes a new entry that matches the filter
    Then the Live chip should say "Live · 1 new"
    And choosing it should add the entry at the top

  Scenario: Download the filtered journal as JSON Lines
    When I choose "Download…", keep "Current filter" and "JSON Lines" and choose "Download"
    Then the file "filesquid-logs-<date>.jsonl" should be saved with the filtered entries, oldest first
    And it should hold opaque ids only, with no file names

  # Edge cases
  Scenario Outline: Every filter narrows the list
    When I choose "<chip>" and then "<value>"
    Then only entries that match should be listed
    And the chip should show the chosen value

    Examples:
      | chip      | value        |
      | Last 24 hours | Last hour |
      | Library: all  | the library's name |
      | Operation: all | metadata.set |
      | Outcome: all  | failed |
      | Outcome: all  | refused |
      | Level: all    | error |

  Scenario: Search by operation, object and operation id
    When I search for "ocfl.commit", for "urn:uuid:<file id>@v8" and for "<operation id>"
    Then each search should filter by the operation, the object and the operation id it names
    And text that names none of them should not search and should say what can be searched

  Scenario: Failed and refused entries and downloads are journal-only
    When I select a failed entry, a refused entry or a "file.download" entry
    Then the row should carry the "journal-only" tag
    And the detail should offer no "View provenance"

  Scenario: Load older reads the next page
    Given more entries exist than one page holds
    When I choose "Load older"
    Then the next older entries should be added below
    And the line should say "Showing 50 entries … · more are available" until the last page is read

  Scenario: Download a time range as CSV with one header row
    When I choose "Time range", "CSV" and "Download"
    Then the CSV should hold the entries between the two times
    And it should have exactly one header row even when the export took several requests

  # Error cases
  Scenario: A caller who is not the owner sees a permission state
    Given the API answers the journal route with 403
    When I open the Logs view
    Then the view should show a permission state and no entries
    And the Live chip should not be shown

  Scenario: A failed download keeps the dialog open
    Given the export route fails
    When I choose "Download"
    Then the dialog should show "The download could not be made. Nothing was saved. Try again."
    And no file should be saved

  Scenario: A failed read shows its reason and Try again
    Given the journal route answers 500
    When I open the Logs view
    Then the view should show the error state with its code and "Try again"

  # Client events (UI-S-023)
  Scenario: A console warning becomes one journal entry with no message text
    When the page logs a warning
    Then the UI should send an event with a level, the component "console", an error code and a client event ID
    And the event should carry no message text, URL, file name or stack

  Scenario Outline: Delivery is retried with backoff and never merges events
    Given the client-events route answers <answer> to the first request
    When the UI sends 3 warnings
    Then it should retry the same events, waiting at least <wait>
    And each of the 3 events should become its own journal entry
    And each original client event ID should be journaled exactly once

    Examples:
      | answer                  | wait          |
      | 429 with Retry-After: 2 | 2 seconds     |
      | 401                     | 1 second      |
      | 403                     | 1 second      |
      | 503                     | 1 second      |
      | a network failure       | 1 second      |

  Scenario: A refused batch is split into single events
    Given the route answers 400 to a batch with one bad event
    When the UI delivers it
    Then each event should be sent alone
    And the refused event should be replaced by one "ui.console.error" event with the error code "client_event_refused"

  Scenario: A full queue spills into IndexedDB and loses nothing
    Given the in-memory queue holds 10,000 events
    When 5 more warnings are raised
    Then the 5 events should be stored in the page's IndexedDB store
    And all 10,005 should be delivered, oldest first

  Scenario: Undelivered events are persisted at page close and sent first on the next open
    Given 3 events are undelivered
    When the page closes and the app opens again
    Then the 3 events should be delivered, oldest first, before new events
    And each should keep its original client event ID
    And each original client event ID should be journaled exactly once

  Scenario: An event queued while storage accepts writes survives a page that ended without pagehide
    Given an event was queued and its IndexedDB write committed
    When the page ended without a pagehide event and the app opens again
    Then the event should be delivered on that open

  Scenario: Another principal's stored events are held untouched
    Given stored events carry another principal than this session's
    When the app opens
    Then those events should stay in the store and be sent only under that principal's credential

  Scenario: Stored events raised against another host stay until that host and principal are active
    Given stored events were raised against another host and principal
    When the app opens under a different host
    Then those events should stay in the store
    And they should be sent only when that same host and principal are active

  Scenario: Without IndexedDB the UI counts what it could not keep
    Given IndexedDB is unavailable in the page
    When 4 warnings are raised and the page closes
    Then the UI should send one "ui.console.overflow" event with a count of 4 as a keep-alive request
    And the event should carry its own client event ID and no error code
    And the event should use the fixed component "client-events"
    And the event should carry no opaque ids
    And the count should be from 1 to 1,000,000
