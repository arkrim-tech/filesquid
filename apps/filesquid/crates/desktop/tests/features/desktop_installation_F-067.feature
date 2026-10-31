# F-067: PROV-O history of every version and change
# SRS-F-067: PROV-O history of every version and change
# DE-S-012: Installation UUID
# DE-S-011: Native journal and commit linkage
Feature: The desktop's installation identity and the journal linkage of its commits
  As the owner of a FileSquid installation
  I want one stable, anonymous installation identity, and every committed version linked to the journal
  So that my history names this installation, never me or my computer, and a crash loses no journal entry

  # Happy path
  Scenario: The first launch creates the installation UUID once
    Given a desktop data directory with no installation UUID
    When the desktop launches twice
    Then both launches should read the same UUID v4
    And the installation file should hold schema 1, the UUID and an RFC 3339 UTC creation time
    And the journal should record "host.installation.created" once
    And no journal entry should name the installation UUID or the data directory

  # Edge case
  Scenario Outline: A lost copy is restored from the other
    Given a desktop that has launched once
    When its <copy> is lost and the desktop launches again
    Then the launch should read the same UUID as before
    And the installation file should be as it was after the first launch
    And the journal should record "host.installation.restored" once

    Examples:
      | copy              |
      | installation file |
      | operational store |

  # Edge case
  Scenario: Losing both copies makes a new installation, and journals it
    Given a desktop that has launched once
    When both copies are lost and the desktop launches again
    Then the launch should read a new UUID v4
    And the journal should record "host.installation.created" once

  # Error case
  Scenario: A malformed installation file is refused without naming a path
    Given a desktop data directory whose installation file holds no UUID v4
    When the desktop launches
    Then the launch should be refused with an error that names no path
    And the journal should record "host.installation" failed with the error code "malformed"

  # Error case (crash recovery)
  Scenario: A crash between an import's commit and its journal entry is repaired with the recorded ID
    Given a desktop with an open library whose commits record their history
    And a file was imported but the app stopped before its commit reached the journal
    When the import is replayed twice after the restart
    Then the journal should hold that import's "ocfl.commit" entry once, under the ID its version recorded
    And that entry should name the import's operation and object version and no path
