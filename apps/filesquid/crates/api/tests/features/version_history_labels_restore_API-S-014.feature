# API-S-014: Version listing, labels, restore and history size
# SRS-F-065 / PRD F-065: file version history and retention
# API-S-028: a PROV-O record per commit
Feature: Version history, labels and restore through the API
  As the owner of a FileSquid library
  I want to see every version of a file with its size and label, restore an earlier one and name versions
  So that I can find and bring back any earlier state without FileSquid ever discarding a version

  Background:
    Given the owner's file "a.txt" was saved as "alpha" and then as "bravo!"

  Scenario: The version listing shows each version's size, digest and label
    When the owner lists the versions of "a.txt"
    Then the listing should show 2 versions with sizes 5 and 6
    And each version should show its digest, creation time and label

  Scenario: An earlier version is read in ranges
    When the owner reads bytes 1 to 2 of "a.txt" as it was in "v1"
    Then the answer should be the partial content "lp"

  Scenario: Restore makes exactly one new version and keeps every earlier one
    When the owner restores "a.txt" from "v1" with the label "Original"
    Then the object should have 3 versions
    And "a.txt" should hold "alpha" again
    And version "v3" should be labelled "Original"
    And version "v2" should still hold "bravo!"

  Scenario: A label set later is kept with the file
    When the owner labels version "v1" as "First draft"
    Then version "v1" should be labelled "First draft"
    And the object should keep its labels in its own files

  Scenario: A label can be cleared
    Given the owner labelled version "v1" as "First draft"
    When the owner clears the label of version "v1"
    Then version "v1" should have no label

  Scenario: A label that changes nothing makes no version
    Given the owner labelled version "v1" as "First draft"
    When the owner labels version "v1" as "First draft"
    Then the answer should be the version error "conflict"
    And the object should have 3 versions

  Scenario: History size is reported per file and per library
    When the owner asks how much history "a.txt" keeps
    Then the file's history should keep 2 contents in 11 bytes
    And the library's history should keep 2 contents in 11 bytes

  Scenario: The API offers no way to delete, expire or thin versions
    Then the contract should offer no version deletion, expiry or thinning

  Scenario: A restore records its PROV-O activity in the same version
    Given the host records provenance
    When the owner restores "a.txt" from "v1" with the label "Original"
    Then version "v3" should hold its PROV-O record
    And the restore should be journaled once with its commit
