# F-067: PROV-O history of every version and change
# F-055: Default UI and panel composition; F-061: Activity journal
# UI-S-019: Display names; UI-S-020: Provenance tab; UI-S-021: Logs view; UI-S-022: File panel tabs
# Scenarios only; step definitions arrive with Tasks FS-2.8 and FS-2.9 (UI-F2.15).
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

  Scenario: The Logs view is reached from the sidebar below Settings
    When I look at the sidebar
    Then the "Logs" item should be placed directly below "Settings"
    And choosing it should open the Logs view with all journal entries newest first

  Scenario: The owner exports the history as JSON-LD
    When I choose "Export history" in the "Provenance" tab
    Then a JSON-LD file of the file's PROV-O records should be downloaded

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

  # Source
  # - resources/srs.md SRS-F-067, SRS-F-055, SRS-F-061
  # - resources/prd.md F-067, F-055, F-061
  # - apps/filesquid/ui/resources/srs.md UI-S-019, UI-S-020, UI-S-021, UI-S-022
