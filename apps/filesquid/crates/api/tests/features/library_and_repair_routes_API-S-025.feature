# API-S-025: Local library and repair routes
# API-S-023: Version-inventory repair, consented and journaled (OCFL-S-020)
# SRS-F-001 / PRD F-001: create and open a local library; copy a completed snapshot
# SRS-F-042 / PRD F-042: OCFL preservation and portable recovery from files alone
Feature: Library and repair routes
  As the owner of a FileSquid library
  I want to create, open, copy, rebuild and repair my library through the FileSquid API
  So that every FileSquid UI reaches one meaning of these operations, on every host

  The host registers each folder the owner chose under an opaque location ID and injects it
  as an OCFL storage root, so no path crosses the API. The same router serves the desktop
  owner and the server administrator.

  Scenario: Create a library in an empty folder through the API
    Given the desktop owner's host with an empty folder registered
    When the owner creates a library in that folder through the API
    Then the answer should name an opaque library ID and no filesystem path
    And the registered folder should hold only the OCFL 1.1 storage-root declaration

  Scenario: Open a library's folder through the API
    Given the desktop owner's library with a complete object and an object without a version inventory
    When the owner opens the library's folder through the API
    Then the answer should name the same library with 2 objects
    And the library folder should be unchanged

  Scenario: A folder that already holds a file is refused once
    Given the desktop owner's host with a registered folder that holds a file
    When the owner creates a library in that folder through the API
    Then the answer should be the error "conflict"
    And exactly one refusal "root_not_empty" should be journaled for "library.root.create"
    And the registered folder should still hold only that file

  Scenario: Copy a completed snapshot and rebuild from files through the API
    Given the desktop owner's library with a complete object and an object without a version inventory
    When the owner copies a completed snapshot of the library to an empty folder through the API
    Then the copy should report 2 objects and no invalid object
    And the library folder should be unchanged
    When the owner rebuilds the library from its files through the API
    Then the rebuild should flag only the object without a version inventory

  Scenario: The candidate query offers only the object without a version inventory
    Given the desktop owner's library with a complete object and an object without a version inventory
    When the owner asks the API which objects need a version-inventory repair
    Then only the object without a version inventory should be offered

  Scenario: A consented repair by the desktop owner makes the object committable
    Given the desktop owner's library with a complete object and an object without a version inventory
    When the owner repairs the object without a version inventory with consent through the API
    Then the object should be answered as repaired
    And the consent and one outcome row per object should be recorded as "ocfl.repair.version_inventory"
    And the repaired object should accept a new version

  Scenario: A head that already has both files is refused with nothing_to_repair
    Given the desktop owner's library with a complete object and an object without a version inventory
    When the owner repairs the complete object with consent through the API
    Then the object should be answered as refused with "nothing_to_repair"
    And the consent and one outcome row per object should be recorded as "ocfl.repair.version_inventory"
    And the object's outcome row should be failed with "nothing_to_repair"
    And the library folder should be unchanged

  Scenario: A repair without consent is refused once and changes nothing
    Given the desktop owner's library with a complete object and an object without a version inventory
    When the owner asks the API to repair the object without a version inventory without consent
    Then the answer should be the error "conflict"
    And exactly one refusal "consent_required" should be journaled for "ocfl.repair.version_inventory"
    And the library folder should be unchanged

  Scenario: A repair without a credential is refused once
    Given the desktop owner's library with a complete object and an object without a version inventory
    When a caller without a credential asks the API to repair the object without a version inventory
    Then the answer should be the error "unauthorized"
    And exactly one refusal "missing_credentials" should be journaled for "api.auth.denied"
    And the library folder should be unchanged

  Scenario: The server administrator is refused the repair
    Given the server administrator's library with a complete object and an object without a version inventory
    When the server administrator repairs the object without a version inventory with consent through the API
    Then the answer should be the error "forbidden"
    And exactly one refusal "not_permitted" should be journaled for "api.auth.denied"
    And the library folder should be unchanged

  Scenario: The server administrator can still rebuild a library
    Given the server administrator's library with a complete object and an object without a version inventory
    When the server administrator rebuilds the library from its files through the API
    Then the rebuild should flag only the object without a version inventory
