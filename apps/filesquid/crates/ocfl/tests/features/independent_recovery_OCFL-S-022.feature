# OCFL-S-022: Independent recovery. A copy of a completed snapshot made with ordinary filesystem tools opens, validates and enumerates like the source; an independent reader rebuilds files, folders and sidecar pairs from it; altered originals and inventories are detected; a head without a version inventory stays refused and is never repaired by copying or reading.
# F-001: Independent local library and ownership
# F-042: OCFL preservation and portable recovery
# RC-010: Complete OCFL preservation and independent recovery
Feature: Independent recovery of a completed snapshot
  As a library owner
  I want to copy my storage root with ordinary tools and rebuild it without FileSquid or Git
  So that ownership and recovery never depend on a running service

  Background:
    Given a completed snapshot written by the engine
    And I copy the storage root with ordinary filesystem tools

  # Happy path
  Scenario: The copy opens, validates and enumerates like the source
    When the engine opens, validates and enumerates the copy
    Then every object of the copy should validate with zero errors
    And the copy should enumerate the same files as the source
    And the copy should hold only OCFL files

  Scenario: An independent reader rebuilds files, folders and sidecar pairs
    When an independent reader recovers the copy with only a JSON parser and SHA-2
    Then every logical file of every version should be recovered with its original bytes
    And the folders and the sidecar pairs of "v2" should be rebuilt

  Scenario: Inspecting the copy changes nothing
    When the engine opens, validates and enumerates the copy
    Then the copy should be byte-identical to before
    And the source should be byte-identical to before

  # Error cases: altered bytes and inventories in the copy
  Scenario: A changed signed original is detected
    Given a stored byte of "IMG_0001.jpg" in the copy is changed
    When the engine opens, validates and enumerates the copy
    Then validation should report "E092"
    And streaming "IMG_0001.jpg" of "v1" should stop with "E092"
    And the independent reader should refuse the copy as a digest mismatch

  Scenario: A changed root inventory is refused when its sidecar is untouched
    Given the root inventory in the copy is edited
    When the engine opens, validates and enumerates the copy
    Then validation should report "E060"
    And opening the object should be refused with "E060"

  Scenario: A changed inventory is detected even with a recomputed sidecar
    Given the root inventory and its sidecar in the copy are forged together
    When the engine opens, validates and enumerates the copy
    Then validation should report "E092"
    And the independent reader should refuse the copy as a digest mismatch

  Scenario: A changed earlier version inventory is detected
    Given the version 1 inventory in the copy is edited
    When the engine opens, validates and enumerates the copy
    Then validation should report "E060"

  # Edge case: owner decision, a head without a version inventory is never repaired here
  Scenario: A copy without a head version inventory stays refused
    Given the head version inventory is missing from the copy
    When the engine opens, validates and enumerates the copy
    Then validation should warn "W010" with no errors
    And a commit on the copy should be refused for the missing version inventory
    And the copy should be byte-identical to before

  # Source
  # - resources/prd.md F-001, F-042, RC-010
  # - apps/filesquid/crates/ocfl/resources/srs.md OCFL-S-022, OCFL-S-019, OCFL-S-020
  # - apps/filesquid/ui/tests/features/independent_local_library_and_ownership_F-001.feature
