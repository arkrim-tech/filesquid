# OCFL-S-004: Logical state MUST be inventory-driven at a caller-pinned version; stream logical files; enumerate committed logical state.
# F-051: OCFL module (committed logical-state enumerator, T-039)
Feature: Enumerate committed logical state
  As a program that rebuilds a catalog or projects a dataset from OCFL
  I want to enumerate the committed logical state of a whole storage root
  So that I can rebuild my own indexes from the storage root alone, without a database

  Background:
    Given an OCFL storage root in memory holding the objects "ab/obj1" with three versions and "cd/obj2" with one

  # Happy path
  Scenario: Each committed file at head is enumerated with its locator
    When I enumerate the committed logical state at each object's head
    Then the enumeration should be:
      | event   | object          | version | path     | content locator             |
      | object  | urn:example:one | v3      |          | ab/obj1                     |
      | version | urn:example:one | v3      |          |                             |
      | file    | urn:example:one | v3      | a.txt    | ab/obj1/v3/content/a.txt    |
      | file    | urn:example:one | v3      | c.txt    | ab/obj1/v1/content/b.txt    |
      | object  | urn:example:two | v1      |          | cd/obj2                     |
      | version | urn:example:two | v1      |          |                             |
      | file    | urn:example:two | v1      | only.txt | cd/obj2/v1/content/only.txt |
    And every enumerated locator should hold bytes that match the file's digest

  Scenario: Every committed version is enumerated oldest first
    When I enumerate the committed logical state of every version
    Then the versions enumerated for "urn:example:one" should be "v1, v2, v3"
    And version "v2" of "urn:example:one" should list "a.txt, c.txt, d.txt"

  # Edge cases
  Scenario: Files the inventory does not list are not enumerated
    Given a stray file "v3/content/stray.txt" in the object "ab/obj1"
    When I enumerate the committed logical state of every version
    Then no enumerated file should be "stray.txt"

  Scenario: A version above the committed head is not enumerated
    Given version "v4" content and a version inventory exist in "ab/obj1" while its root head is "v3"
    When I enumerate the committed logical state of every version
    Then the versions enumerated for "urn:example:one" should be "v1, v2, v3"

  # Error cases
  Scenario: A broken object is reported and the others are still enumerated
    Given the object "b/broken" in the storage root has no root inventory
    When I enumerate the committed logical state at each object's head
    Then the enumeration should report "b/broken" with OCFL code "E063"
    And the objects enumerated should be "ab/obj1, cd/obj2"

  # Source
  # - resources/product-vision.md T-039
  # - resources/prd.md F-051, F-042
  # - apps/filesquid/crates/ocfl/resources/srs.md OCFL-S-004, OCFL-S-003
  # - resources/architecture/023-filesquid-ocfl-engine-and-parquet.md (engine core item 6)
  # - resources/architecture/016-rust-ocfl-library.md (Read and browse)
