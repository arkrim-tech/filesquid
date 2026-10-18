# OCFL-S-019: Inspection and copy never edit a completed object. Reading, browsing, validating or copying an object MUST NOT change its inventories or stored content; a copy MUST refuse a destination inside an OCFL storage root or object root, so only the writer (OCFL-S-018) changes an object, and only by adding a version.
# F-042: OCFL preservation and portable recovery
# T-036: Direct inspection and copy MUST NOT authorize arbitrary in-place edits of OCFL inventories or signed originals
Feature: Inspection and copy never edit a completed object
  As a preservation programme reading OCFL objects with filesquid-ocfl
  I want copies of logical files to land only outside OCFL roots
  So that inspecting an object can never rewrite its inventory or its stored originals

  Background:
    Given an OCFL object "obj" committed at "v1" through the engine writer

  # Happy path
  Scenario: A logical file copies out to another backend
    When I copy "b.txt" of "v1" to "export/b.txt" on another backend
    Then the copy should hold the bytes of "b.txt"
    And the source object should be byte-identical to before the copy

  # Error cases: a copy aimed inside the object it reads
  Scenario Outline: A copy into the object root is refused
    When I copy "b.txt" of "v1" to "<destination>" in the same storage
    Then the copy should be refused as a write into an OCFL root
    And "<destination>" should be unchanged
    And the source object should be byte-identical to before the copy
    And the object should validate with zero errors

    Examples:
      | destination           |
      | obj/inventory.json    |
      | obj/v1/content/a.txt  |
      | obj/v2/content/b.txt  |

  # Edge case: the whole backend is a storage root
  Scenario: A copy into a storage root is refused
    Given the storage holds an OCFL storage root declaration
    When I copy "b.txt" of "v1" to "export/b.txt" in the same storage
    Then the copy should be refused as a write into an OCFL root
    And "export/b.txt" should be unchanged

  # Source
  # - resources/prd.md F-042
  # - resources/product-vision.md T-036
  # - apps/filesquid/crates/ocfl/resources/srs.md OCFL-S-019
  # - resources/architecture/016-rust-ocfl-library.md "Direct inspection and copy MUST NOT authorize in-place inventory or signed-original edits"
