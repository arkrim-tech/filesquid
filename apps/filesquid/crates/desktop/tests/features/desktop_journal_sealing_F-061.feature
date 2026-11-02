# F-061: Activity journal and live health monitoring
# SRS-F-061: Activity journal and live health monitoring (durable journal)
# DE-S-013: Journal sealing and rebuild
Feature: The desktop seals its activity journal and rebuilds it from files
  As the owner of a FileSquid desktop
  I want my activity journal sealed into OCFL objects on a schedule and at close
  So that deleting or losing the app's database loses no sealed entry and keeps every journal ID

  # Happy path
  Scenario: The hourly seal runs on its own and stops with the app
    Given a desktop whose sealing runs on a short hourly interval
    When a host event is journaled
    Then it should be sealed into the installation's journal object without being asked
    And the hourly seal should end when the app exits

  # Happy path
  Scenario: Closing the app seals what is pending
    Given a running desktop with its sealing started
    When a host event is journaled and the app closes
    Then the close-time seal should have sealed it into the installation's journal object

  # Edge case
  Scenario: A seal missed by a forced quit is caught up at the next start
    Given a desktop that journaled a library creation and was forced to quit without sealing
    When the desktop starts again
    Then the library creation should be sealed into the installation's journal object
    And the journal should record one "journal.seal" run that succeeded, as a registered process

  # Error case (lost database)
  Scenario: A deleted database is recreated and refilled under the same journal IDs
    Given a desktop that journaled a library creation and was forced to quit without sealing
    And the desktop started again, which caught up the seal
    When the operational store is deleted and the desktop starts again
    Then every entry of the first launch should be back under its own journal ID
    And the journal should record "journal.refill" succeeded once
    And no journal entry should name the data directory

  # Edge case (library journal)
  Scenario: A served library's events are sealed into its own folder and restored by its rebuild
    Given a running desktop serving a library from a folder the owner chose
    When an upload into that library is journaled and the journal is sealed
    Then the upload should be sealed into the library's own storage root
    And after the operational store is lost, the library's rebuild should restore it under its journal ID
