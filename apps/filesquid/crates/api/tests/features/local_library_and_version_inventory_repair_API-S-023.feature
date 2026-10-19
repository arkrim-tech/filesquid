# API-S-023: Version-inventory repair, consented and journaled (OCFL-S-020)
# SRS-F-001 / PRD F-001: create and open a local library; copy and inspect a completed snapshot
# SRS-F-042 / PRD F-042: OCFL preservation and portable recovery from files alone
Feature: Local library create, open, copy, rebuild and version-inventory repair
  The application layer in filesquid-api decides what creating, opening, copying, rebuilding
  and repairing a local library means. The host injects the owner-chosen folder as an OCFL
  storage root, so no path crosses the API. Inspection and copy never edit the library, and
  the only command that adds a file to an existing version directory is the repair the owner
  consents to.

  Scenario: Create a library in an empty folder without an account or a connection
    Given an empty folder chosen by the owner
    When the owner creates a library there
    Then the folder should hold only the OCFL 1.1 storage-root declaration
    And the creation should be journaled as succeeded

  Scenario: A folder that already holds files is refused
    Given a folder chosen by the owner that already holds a file
    When the owner creates a library there
    Then the request should be refused with "root_not_empty"
    And nothing should be written to the folder

  Scenario: Copying a completed snapshot leaves the library untouched
    Given a library with a complete object and an object whose head has no version inventory
    When the owner copies a completed snapshot to an empty folder
    Then the copy should validate without errors and open as a library of 2 objects
    And nothing should be written to the library
    And the object without a version inventory should still lack it

  Scenario: Rebuilding from files reports problems and repairs nothing
    Given a library with a complete object and an object whose head has no version inventory
    When the owner rebuilds the library from its files
    Then the rebuild should list 2 objects
    And the object without a version inventory should be flagged as a repair candidate
    And nothing should be written to the library

  Scenario: The candidate query lists the objects to offer
    Given a library with a complete object and an object whose head has no version inventory
    When the owner asks which objects need a version-inventory repair
    Then only the object without a version inventory should be listed

  Scenario: A consented repair by the owner makes the object committable
    Given a library with an object whose head has no version inventory
    And a new version of that object is refused until it is repaired
    When the owner consents and repairs that object
    Then the object should be repaired
    And a new version of that object should be accepted
    And the consent and one outcome per object should be journaled without paths

  Scenario: The repair is refused without consent
    Given a library with an object whose head has no version inventory
    When the repair is requested without the owner's consent
    Then the request should be refused with "consent_required"
    And nothing should be written to the library
    And the refusal should be journaled

  Scenario: The repair is refused for another caller
    Given a library with an object whose head has no version inventory
    When the server administrator requests the repair with consent
    Then the request should be refused with "forbidden"
    And nothing should be written to the library
    And the refusal should be journaled

  Scenario: A head that already has its version inventory is not repaired again
    Given a library with a complete object
    When the owner consents and repairs that object
    Then the object should be refused with "nothing_to_repair"
    And nothing should be written to the library
