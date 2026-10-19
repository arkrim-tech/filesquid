# F-067: PROV-O history of every version and change
# F-055: Default UI and panel composition; F-061: Activity journal
# UI-S-019: Display names; UI-S-020: PROV-O tab; UI-S-022: File panel tabs
# Scenarios only; step definitions arrive with Tasks FS-2.8 and FS-2.9 (UI-F2.15).
Feature: File panel tabs: File facts, Descriptive, PROV-O and Audit log
  As a library owner
  I want to see a file's facts, description, history and audit log in one consistent panel
  So that I can understand and keep everything that happened to it

  Background:
    Given the default FileSquid UI on the desktop host
    And a file "Holiday 2023.jpg" with an import, a rename and a metadata edit

  # Happy path
  Scenario: The file panel offers four tabs with the same look and behaviour
    When I open the file panel for the file
    Then I should see the tabs "File facts", "Descriptive", "PROV-O" and "Audit log"
    And each tab should show a table of property and value rows with copy and an in-tab search

  Scenario: File facts show the metadata record
    When I open the "File facts" tab
    Then I should see the original file name "Holiday 2023.jpg", the extension, the format and MIME type, the size, the digest and the import facts
    And the stored name should not be shown

  Scenario: The PROV-O tab lists the history newest first
    When I open the "PROV-O" tab
    Then I should see three activities newest first with their prov attributes and values
    And the associated agents should be the installation agent and the FileSquid software agent with its version

  Scenario: The Audit log tab lists the file's journal entries
    When I open the "Audit log" tab
    Then I should see the file's journal entries newest first with paging
    And each entry should link to its PROV-O activity

  # User interactions
  Scenario: The owner exports the history as JSON-LD
    When I choose "Export history" in the "PROV-O" tab
    Then a JSON-LD file of the file's PROV-O records should be downloaded

  Scenario: Searching within a tab filters its rows
    When I type "digest" into the search field of the "File facts" tab
    Then only the rows whose property or value matches should remain

  Scenario: Every tab is keyboard accessible
    When I move through the tabs with the arrow keys
    Then focus and selection should follow the accessibility contracts

  # Edge cases
  Scenario: Reconstructed activities are marked
    Given a version whose record was reconstructed without a recorded agent
    When I open the "PROV-O" tab for its file
    Then that activity should be marked as reconstructed
    And its agent should be shown as unrecorded

  Scenario: A tab mounts without the other panels
    When the "PROV-O" tab is mounted without the Details and Versions panels
    Then it should render its rows without errors

  # Error case
  Scenario: A tab shows a distinct error state
    Given the history route fails
    When I open the "PROV-O" tab
    Then I should see the tab's error state, not an empty list

  # Source
  # - resources/srs.md SRS-F-067, SRS-F-055, SRS-F-061
  # - resources/prd.md F-067, F-055, F-061
  # - apps/filesquid/ui/resources/srs.md UI-S-019, UI-S-020, UI-S-022
