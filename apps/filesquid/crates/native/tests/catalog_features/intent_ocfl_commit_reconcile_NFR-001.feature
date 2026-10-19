# NFR-001: Preservation and transactional integrity
# F-042: OCFL preservation and portable recovery
# DE-S-010: Intent, OCFL snapshot commit and receipt reconcile on the desktop
Feature: Intent, OCFL snapshot commit and receipt reconcile
  As an owner
  I want every organisation change to be durable in my library's OCFL files before it is acknowledged
  So that a crash never loses or duplicates an acknowledged change, even without the catalog

  # Happy path
  Scenario: A change is published as an OCFL version before it is acknowledged
    Given an empty catalog directory
    When the owner creates the library "Family archive" with the collection "Summer"
    Then the library's OCFL snapshot should be at version "v2"
    And no intent should be pending

  # Error case
  Scenario: A crash before the snapshot leaves the work pending
    Given an empty catalog directory whose commits crash before the snapshot of "op-summer"
    And the owner created the library "Family archive"
    When the owner adds the collection "Summer" under operation "op-summer"
    And the host restarts
    Then the intent of "op-summer" should be pending
    And the library's OCFL snapshot should be at version "v1"
    When the owner adds the collection "Summer" under operation "op-summer"
    Then the library's OCFL snapshot should be at version "v2"
    And no intent should be pending

  # Error case
  Scenario: A crash after the snapshot loses no acknowledged state
    Given an empty catalog directory whose commits crash after the snapshot of "op-summer"
    And the owner created the library "Family archive"
    When the owner adds the collection "Summer" under operation "op-summer"
    And the host restarts
    Then the collection "Summer" should be listed
    And no intent should be pending

  # Edge case
  Scenario: A lost catalog is rebuilt from the OCFL snapshot
    Given an empty catalog directory
    And the owner created the library "Family archive" with the collection "Summer"
    When the catalog file is deleted and the host restarts
    Then the collection "Summer" should be listed
    And the library "Family archive" should be at revision 2
