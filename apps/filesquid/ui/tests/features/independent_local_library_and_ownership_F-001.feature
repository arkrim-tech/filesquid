# SRS-F-001: Independent local library and ownership
Feature: Independent local library and ownership
  As a library owner
  I want a library I can copy and reconstruct without FileSquid
  So that ownership does not depend on a running service

  Scenario: Closed-app copy reconstructs organization
    Given I have a managed local library with a completed OCFL snapshot
    When I close FileSquid and copy that snapshot with ordinary filesystem tools
    Then I should reconstruct supported organization without the operational database or a running FileSquid service

  Scenario: Changes stay pending until the snapshot is durable
    Given I import or change files
    When the OCFL snapshot is not yet durable
    Then those changes should remain visibly pending

  Scenario: Inspection does not authorize in-place inventory edits
    Given I can inspect and copy a completed snapshot
    When I view that snapshot outside FileSquid
    Then that access should not authorize in-place edits of OCFL inventories or signed originals

  # Task UI-1.1: Library create/open/recover UI
  # API-S-034: the folder is chosen through the location-choices route and reaches the library
  #   routes only as an opaque location ID
  # API-S-023, API-S-025: the consented version-inventory repair
  # Executable proofs: tests/unit/library-*.test.ts, tests/ui/library-*.test.ts and
  #   tests/e2e/preview/library.spec.ts

  Scenario: Creating a library passes the chosen folder on as a location ID
    Given I am on the library start page
    When I choose "Create library" and pick an empty folder
    Then the host should be asked for a folder with the purpose "create"
    And the create route should receive only the location ID the host answered
    And I should see the new library's ID, its OCFL version and its object count
    And no filesystem path should appear on the page or in any request

  Scenario: Opening a library passes the chosen folder to the open route
    Given I am on the library start page
    When I choose "Open library" and pick a folder that holds a library
    Then the host should be asked for a folder with the purpose "open"
    And the open route should receive the location ID and nothing in the folder should be rewritten

  Scenario: Recovering from a copied snapshot rebuilds the catalog from files
    Given I have a copied OCFL storage root
    When I choose "Recover" and pick that folder
    Then the library should be opened and its catalog rebuilt from the OCFL files alone
    And the report should show the objects read, the objects that could not be opened and the objects with validation errors
    And nothing should be repaired or deleted

  Scenario: Cancelling the folder dialog changes nothing
    Given I am on the library start page
    When I choose "Create library" and cancel the folder dialog
    Then no library request should be sent
    And I should be told that nothing changed

  Scenario: A host without a folder chooser shows the unavailable state with a Retry
    Given the host answers "503 unavailable" to the folder choice
    When I choose "Open library"
    Then I should see the unavailable state with its reason and a Retry
    And the create, open and recover choices should still be offered
    And Retry should ask for the folder again

  Scenario: Retry after a failed open keeps the folder already chosen
    Given I chose "Open library" and picked a folder
    And the host answered "503 unavailable" to opening it
    When I choose Retry
    Then the open route should receive the same location ID again
    And I should not be asked for a folder again

  Scenario: Retry after a failed rebuild of a recovered copy rebuilds only
    Given I chose "Recover" and picked a copied OCFL storage root
    And the copy was opened but the host answered "503 unavailable" to rebuilding it
    When I choose Retry
    Then the rebuild route should receive the opened library again
    And the copy should not be opened again and I should not be asked for a folder again

  Scenario: Retry after a failed copy copies to the destination already chosen
    Given I am on a library's page
    And I chose "Copy library to…" and picked an empty folder
    And the host answered "503 unavailable" to the snapshot copy
    When I choose Retry
    Then the snapshot route should receive the same destination location ID again
    And I should not be asked for a destination again

  Scenario: Copying a library asks for a destination and reports the copy
    Given I am on a library's page
    When I choose "Copy library to…" and pick an empty folder
    Then the host should be asked for a folder with the purpose "copy_destination"
    And the snapshot route should receive the library and the destination location ID
    And the report should show the objects, files and bytes written and each copied object with validation errors

  Scenario: Files that can be read but not changed are said so, with a link to the repair offer
    Given a library holds objects whose head has no version inventory
    When I open the library's page or the Monitor's W010 note
    Then I should be told that files in those objects can be read but not changed
    And a link should lead to the library's repair offer

  Scenario: The repair runs only after the owner confirms the consent dialog
    Given the repair offer lists two objects
    When I review the repair
    Then the dialog should name both objects
    And it should say that one copy of the root inventory and its sidecar will be added to each object's latest version directory, that a new version recording the repair will be committed to each object's history, and that no file content changes
    And no repair request should be sent until I confirm
    When I confirm
    Then the repair route should receive the consent for exactly those two objects
    And I should see the result for each object

  Scenario: Cancelling the consent dialog changes nothing
    Given the repair consent dialog is open
    When I cancel it or press Escape
    Then no repair request should be sent
    And focus should return to the control that opened the dialog

  Scenario: A refused repair says nothing was changed
    Given the host refuses the repair because the consent no longer matches
    When I confirm the consent dialog
    Then I should be told that nothing was changed and to review the repair again
    And no result should be listed

  # Source
  # - srs.md SRS-F-001
  # - prd.md F-001
