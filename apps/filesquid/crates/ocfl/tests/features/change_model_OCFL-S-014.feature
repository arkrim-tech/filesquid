# OCFL-S-014: Delta computation. Given a working state and a base version, the engine computes the change set by digest (added, changed, renamed, deleted, reinstated), writes only bytes whose digest is not already in the manifest, and renames, deletes and reinstatements change state only.
# OCFL-S-015: Change-model API. stage, status, diff, commit, log, checkout, revert and reinstate; each commit publishes exactly one version; revert and reinstate add a version and never modify earlier ones; staging and checkout stay outside the storage root and reads need no staging data.
# OCFL-S-016: Concurrent commits. A moved base head is rebased onto when no staged path changed in the intervening versions; otherwise a conflict is reported without publishing.
# OCFL-S-018: Write order and unpublished versions (a head without its version inventory stays refused as a commit base)
# OCFL-S-020: Version-inventory repair (the change model never repairs implicitly)
# OCFL-S-010: Read and write conformance (b) (committed objects validate in the engine and a community validator)
# F-051: OCFL module
# NFR-001: Preservation and transactional integrity
Feature: Forward-delta change model
  As a preservation programme editing OCFL objects with filesquid-ocfl
  I want a git-like stage, status, diff and commit over the engine's write path
  So that each new version stores only new bytes and never overwrites another writer's version

  Background:
    Given an object committed at "v1" with "a.txt", "b.txt" and "c.txt" through the change model

  # Happy path
  Scenario: A staged change set is classified and stores only new bytes
    When writer "writer-a" stages new bytes for "a.txt", renames "b.txt" to "docs/b.txt" and deletes "c.txt"
    Then the status should list "changed a.txt", "deleted c.txt" and "renamed b.txt -> docs/b.txt"
    When writer "writer-a" commits the staged changes
    Then the object head should be "v2" through the change model
    And the commit should have written only "v2/content/a.txt"
    And the change-model object should validate with zero errors

  Scenario: Reinstating deleted content copies no bytes
    Given writer "writer-a" committed "v2" deleting "c.txt"
    When writer "writer-a" stages the original bytes of "c.txt"
    Then the status should list "reinstated c.txt"
    When writer "writer-a" commits the staged changes
    Then the commit should have written nothing
    And "c.txt" at "v3" should point at "v1/content/c.txt"

  Scenario: Revert and reinstate each add a version and keep history
    Given writer "writer-a" committed "v2" deleting "c.txt"
    When writer "writer-a" reverts to "v1"
    Then the object head should be "v3" through the change model
    And the state of "v3" should equal the state of "v1"
    And no file of "v1" or "v2" should have changed
    And the commit should have written nothing

  # Edge cases: concurrent writers
  Scenario: A moved head is rebased onto when no staged path changed
    Given writer "writer-a" staged new bytes for "a.txt"
    And writer "writer-b" committed "v2" with new bytes for "b.txt"
    When writer "writer-a" commits the staged changes
    Then the object head should be "v3" through the change model
    And "v3" should hold the changes of both writers

  Scenario: A moved head that changed a staged path is a conflict
    Given writer "writer-a" staged new bytes for "a.txt"
    And writer "writer-b" committed "v2" with new bytes for "a.txt"
    When writer "writer-a" commits the staged changes
    Then the commit should be refused as a conflict on "a.txt"
    And the object head should be "v2" through the change model

  # Error cases
  Scenario: An empty change set publishes nothing
    When writer "writer-a" commits the staged changes
    Then the commit should be refused as nothing to commit
    And the object head should be "v1" through the change model

  Scenario: A head without its version inventory stays refused and is not repaired
    Given the head version inventory of "v1" was removed
    And writer "writer-a" staged new bytes for "a.txt"
    When writer "writer-a" commits the staged changes
    Then the commit should be refused for the missing version inventory
    And the head version inventory of "v1" should still be missing

  Scenario: Staging inside the object root is refused
    When a writer stages a file in a staging area inside the object root
    Then staging should be refused as inside an OCFL root
