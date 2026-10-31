# F-067: PROV-O history of every version and change
# F-055: Default UI and panel composition; F-061: Activity journal
# UI-S-019: Display names; UI-S-020: Provenance tab; UI-S-021: Logs view; UI-S-022: File panel tabs
# Task UI-F2.15 (global Task FS-2.9). Executable proof: tests/unit/provenance-model.test.ts,
# tests/unit/provenance-source.test.ts, tests/unit/file-info-rows.test.ts, tests/ui/provenance-panel.test.ts,
# tests/ui/file-info-tab.test.ts, tests/ui/files-route.test.ts (jsdom) and
# tests/e2e/preview/provenance-panel.spec.ts (production build served by vite preview).
# The Logs view itself (View in Logs target, journal-only labels) is Task UI-F2.16.
Feature: File panel tabs: File info, Description and Provenance
  As a library owner
  I want to see a file's facts, description and history in one consistent panel
  So that I can understand and keep everything that happened to it

  Background:
    Given the default FileSquid UI on the desktop host
    And a file "Holiday 2023.jpg" with an import, a rename and a metadata edit

  # Happy path
  Scenario: The file panel offers three tabs with the same look and behaviour
    When I open the file panel for the file
    Then I should see the tabs "File info", "Description" and "Provenance"
    And I should not see an "Audit log" tab
    And each tab should show a table of property and value rows with copy and an in-tab search

  Scenario: File info shows the metadata record
    When I open the "File info" tab
    Then I should see the original file name "Holiday 2023.jpg", the extension, the format and MIME type, the size, the digest and the import facts
    And the stored name should not be shown

  Scenario: Description shows the Dublin Core and Schema.org statements
    When I open the "Description" tab
    Then I should see the file's dcterms and schema statements as property and value rows

  Scenario: The Provenance tab lists the history newest first
    When I open the "Provenance" tab
    Then I should see three activities newest first with their prov attributes and values
    And the associated agents should be the installation agent and the FileSquid software agent with its version

  # User interactions
  Scenario: View in Logs opens the Logs view filtered to the file
    When I choose "View in Logs" in the "Provenance" tab
    Then the Logs view should open filtered to the file
    And each entry of a committed operation should link back to its Provenance activity

  Scenario: Failed and refused entries in Logs are journal-only
    Given a refused rename of the file
    When I open the Logs view filtered to the file
    Then the refused entry should be labelled journal-only
    And it should have no link to a Provenance activity

  Scenario: A single-file download in Logs is journal-only
    Given a single-file download of the file
    When I open the Logs view filtered to the file
    Then the "file.download" entry should be labelled journal-only
    And it should have no link to a Provenance activity

  Scenario: An operation that commits several objects links to one activity per object version
    Given the import of the file committed the file object and the library snapshot
    When I open the import's entry in the Logs view
    Then it should link to two Provenance activities, one per committed object version
    And both activities should carry the same operation ID

  Scenario: The Logs view is reached from the sidebar below Settings
    When I look at the sidebar
    Then the "Logs" item should be placed directly below "Settings"
    And choosing it should open the Logs view with all journal entries newest first

  Scenario: The owner exports the history as JSON-LD
    When I choose "JSON-LD" in the "Provenance" tab
    Then a JSON-LD file of the file's PROV-O records should be downloaded
    And the export should be announced

  Scenario: An export that left records out says so
    Given the history export reports that records were left out
    When I choose "JSON-LD" in the "Provenance" tab
    Then the announcement should say that some records were left out

  Scenario: The latest activity is expanded and shows its prov attributes
    When I open the "Provenance" tab
    Then the latest activity should show prov:startedAtTime, prov:endedAtTime, prov:wasAssociatedWith, prov:used, prov:wasGeneratedBy and prov:wasDerivedFrom
    And the other activities should be collapsed and read their record only when I expand them

  Scenario: Each activity links to the Logs view for its operation
    When I open the "Provenance" tab as the owner and expand every activity
    Then each activity should offer "View in Logs" for its own operation ID
    And the footer should offer "View in Logs" for the file

  Scenario: Anyone who is not the owner or an administrator gets no View in Logs
    When I open the "Provenance" tab on a host where I am not the owner
    Then no "View in Logs" link should be shown

  Scenario: File info shows the import facts without any absolute path
    When I open the "File info" tab
    Then I should see when the file was uploaded in local time and in UTC and the source with its volume
    And the path relative to the import root should be shown only when the API knows it
    And no absolute path, import root, host name or account name should be shown

  Scenario: File info shows what could be read from the file
    Given the file has exif statements
    When I open the "File info" tab
    Then I should see them under "Technical · read from the file", not counted as properties

  Scenario: A format with nothing to read says so
    Given the file has no exif statements
    When I open the "File info" tab
    Then I should see "Nothing could be read from this format."

  Scenario: The Provenance tab has loading and empty states
    Given the history route answers slowly
    When I open the "Provenance" tab
    Then I should see the loading placeholder rows with "Loading provenance…"
    Given the file has no recorded activity
    Then I should see "No activities recorded"

  Scenario: Searching within a tab filters its rows
    When I type "digest" into the search field of the "File info" tab
    Then only the rows whose property or value matches should remain

  Scenario: Every tab is keyboard accessible
    Given focus is on the "File info" tab
    When I press the right arrow key twice
    Then the "Provenance" tab should have focus and be selected
    And its panel should be shown and the other tab panels hidden

  # Edge cases
  Scenario: Reconstructed activities are marked
    Given a version whose record was reconstructed without a recorded agent
    When I open the "Provenance" tab for its file
    Then that activity should be marked as reconstructed
    And its agent should be shown as unrecorded

  Scenario: A tab mounts without the other panels
    When the "Provenance" tab is mounted without the Details and Versions panels
    Then it should render its rows without errors

  # Error case
  Scenario: A tab shows a distinct error state
    Given the history route fails
    When I open the "Provenance" tab
    Then I should see the tab's error state, not an empty list
    And it should show the reason, "Nothing was changed.", the code and "Try again"

  Scenario: A record that cannot be read stays inside its activity
    Given the record of the latest version cannot be read
    When I open the "Provenance" tab
    Then that activity should show an alert with "Try again"
    And the other activities should still be listed

  Scenario: A failed export changes nothing
    Given the history export fails
    When I choose "JSON-LD" in the "Provenance" tab
    Then I should see "Could not export the history. Nothing was changed."

  # Source
  # - resources/srs.md SRS-F-067, SRS-F-055, SRS-F-061
  # - resources/prd.md F-067, F-055, F-061
  # - apps/filesquid/ui/resources/srs.md UI-S-019, UI-S-020, UI-S-021, UI-S-022
