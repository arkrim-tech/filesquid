# F-065: File version history and retention
# SRS-F-065: File version history and retention
# UI-S-009: Versions panel
Feature: File version history and retention
  As a user
  I want to see every version of a file, restore or download any of them and label the ones that matter
  So that I can trust that FileSquid never thins or deletes history behind my back

  Background:
    Given a file with four OCFL versions created over the last day
    And version 3 carries the OCFL message "Draft for review"

  # Listing and reads
  Scenario: The Versions panel lists every version newest first
    When I open the Versions panel for the file
    Then I should see four entries newest first with label, relative time and size
    And the current version should be marked
    And version 3 should show the label "Draft for review"

  Scenario: Any version can be downloaded
    When I choose download on version 2
    Then the bytes of version 2 should be served with ranged reads
    And the current version should remain unchanged

  # Restore
  Scenario: Restore is one action that creates a new version
    When I choose restore on version 2
    Then a fifth version should exist whose content equals version 2
    And versions 1 to 4 should be unchanged

  Scenario: Every change is a version; nothing is coalesced
    When I save two changes to the file ten seconds apart
    Then two new versions should exist
    And the Versions panel should show both

  # Labels
  Scenario: A label set at commit time is the OCFL version message
    When I commit a change with the label "Final"
    Then the new version's OCFL message should be "Final"

  Scenario: A label set later is recoverable application data and searchable
    When I edit the label of version 1 to "Initial import"
    Then the label should show in the panel
    And searching for "Initial import" should find the file
    When the operational store is rebuilt from the OCFL root
    Then the label should still be present

  Scenario: Labels never affect retention
    Given version 2 is unlabeled and version 3 is labeled
    When a year passes and storage is nearly full
    Then both versions should still exist

  # Retention
  Scenario: No delete, expiry or thinning control exists
    When I open the per-version menu
    Then it should offer restore, download and edit label
    And it should not offer delete
    When a client calls the API to delete a single version
    Then the API should refuse because the operation does not exist

  Scenario: No job removes inventory-referenced content to free space
    Given the storage root is at 95 percent capacity
    When the host runs its maintenance jobs
    Then no version content should have been removed
    And the user should see a capacity warning that explains the hardware or quota limit without discarding data

  # Task UI-F2.3: the Versions view and the restore dialog (UI-S-009, UI-S-031 mockups 26 and 27)
  Scenario: The history button opens the Versions view in place of the tabs
    Given a file "harbour-masterplan-v7.pdf" with seven versions, the newest labelled "Issued for tender"
    When I press the history button in the file panel header
    Then the button should be pressed and the tabs hidden
    And I should see Back, "7 versions" and the eyebrow "History 96.3 MB · newest first"
    And the rows should list v7 to v1 with the label or "No label" and "{relative time} · {size}"
    And only v7 should carry the "Current" chip and offer no menu
    And the note "FileSquid keeps every version." should explain that nothing is thinned
    When I choose Back
    Then the tabs should return and focus should be on the history button

  Scenario: Restore as new version opens a dialog naming what changes
    Given the Versions view of "harbour-masterplan-v7.pdf" whose head is v7
    When I choose "Restore as new version" in the menu of v5
    Then a dialog should ask "Restore v5 as a new version?" and name the file
    And it should show the source line of v5 and "New current version, same content as v5" for v8 on the active tint
    And it should say "v6 and v7 stay in the history. You can restore them later."
    And it should offer an optional label for v8, Cancel and one primary Restore

  Scenario: Restoring sends one call with the label and lists the new current version
    Given the restore dialog for v5 is open
    When I type the label "Back to client review version" and choose Restore
    Then exactly one restore of v5 for the file should be sent with that label
    And the dialog should close and "harbour-masterplan-v7.pdf restored. Version v8 committed." should be announced
    And v8 should be listed first as the current version with that label

  Scenario: A label the route would refuse is refused before any call
    Given the restore dialog for v5 is open
    When I type a label longer than 512 bytes, or one with a control character, and choose Restore
    Then no restore should be sent
    And the field should be marked invalid with "Use at most 512 bytes and no control characters."

  Scenario: A refused restore keeps the dialog open and changes nothing
    Given another change was committed to the file first
    When I restore v5
    Then the dialog should stay open with "The files changed or another change is in progress. Reload and try again."
    And Escape should close it and return focus to the menu button of v5

  Scenario: History size is reported
    When I open the file's details
    Then I should see the bytes used by its version history
    When I open the library's storage used
    Then version history should be included in the total

  Scenario: The difference from thinning drives is in the parity register
    When I read the drive-parity table in the capability register
    Then it should state no version expiry, no version deletion and no free-space cap with a reason

  # Source
  # - resources/prd.md F-065
  # - resources/srs.md SRS-F-065
  # - apps/filesquid/ui/resources/srs.md UI-S-009
  # - apps/filesquid/crates/api/resources/srs.md API-S-014
