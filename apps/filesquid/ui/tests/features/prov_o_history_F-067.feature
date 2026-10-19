# F-067: PROV-O history of every version and change
# UI-S-020: History panel
# UI-S-019: Display names on every surface
# Scenarios only; step definitions arrive with Task FS-2.8 (UI-F2.15).
Feature: History panel
  As a library owner
  I want to see and export what happened to a file
  So that I can understand and keep its history

  Background:
    Given the default FileSquid UI on the desktop host
    And a file "Holiday 2023.jpg" with an import, a rename and a metadata edit

  # Happy path
  Scenario: The History panel lists a file's activities newest first
    When I open the History panel for the file
    Then I should see three activities, newest first, each with its kind, agent and time
    And the panel should name the file "Holiday 2023.jpg" and never its stored name

  Scenario: The panel mounts on its own
    When the History panel is mounted without the Details, Versions and Metadata panels
    Then it should render its activities without errors

  # User interaction
  Scenario: The owner exports the history as JSON-LD
    When I choose "Export history" in the History panel
    Then a JSON-LD file of the file's PROV-O records should be downloaded under the file's original name

  Scenario: The owner follows an activity to its journal entries
    When I open the journal link of the rename activity
    Then the Journal tab should show the entries of that operation

  # Edge case
  Scenario: Reconstructed activities are marked
    Given a version whose record was reconstructed without a recorded agent
    When I open the History panel for its file
    Then that activity should be marked as reconstructed
    And its agent should be shown as unrecorded

  # Source
  # - resources/srs.md SRS-F-067
  # - resources/prd.md F-067, F-055
  # - apps/filesquid/ui/resources/srs.md UI-S-019, UI-S-020
