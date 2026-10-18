# OCFL-S-004: Logical state MUST be inventory-driven at a caller-pinned version; stream logical files; enumerate committed logical state.
# F-051: Reusable filesquid-ocfl engine (read and browse)
Feature: Read and browse OCFL objects
  As a preservation programme using filesquid-ocfl
  I want to read any committed version of an OCFL object through its inventory
  So that every file, including unchanged bytes kept in older version directories, is recovered exactly

  Background:
    Given an OCFL object "obj" with three versions in an in-memory storage backend

  # Happy path
  Scenario: Logical state is resolved through the inventory at a pinned version
    When I resolve the logical state of version "v2"
    Then the logical paths should be "a.txt, c.txt, d.txt"
    And "c.txt" should be read from the content path "v1/content/b.txt"

  Scenario: Every logical file streams with bytes that match its inventory digest
    When I stream every logical file of every version in chunks of 2 bytes
    Then every streamed logical file should match its inventory digest

  Scenario: Version history comes from the inventory
    When I read the version history
    Then the history should list "v1, v2, v3" with their created times, messages and users

  Scenario: Two versions are compared
    When I compare version "v1" with version "v2"
    Then the changes should be "renamed b.txt to c.txt, added d.txt"

  # Edge cases
  Scenario: Files that the inventory does not list are not logical state
    Given a stray file "v3/content/stray.txt" in the object
    When I resolve the logical state of version "v3"
    Then the logical paths should be "a.txt, c.txt"

  Scenario: A version above the root head is unpublished
    Given version "v4" content and a version inventory exist while the root head is "v3"
    When I open the object
    Then its head should be "v3"
    And pinning version "v4" should be refused as an unknown version

  Scenario: A storage root lists its outermost objects
    Given the storage root that holds it also holds the objects "ab/obj1" and "cd/ef/obj2"
    And a stray file "v1/content/inner/0=ocfl_object_1.1" in the object
    When I open the storage root
    Then the discovered objects should be "ab/obj1, cd/ef/obj2, obj"

  # Error cases
  Scenario: Content that does not match its digest is refused
    Given the content file "v1/content/b.txt" is altered
    When I stream "c.txt" of version "v3"
    Then the read should be refused with OCFL code "E092"

  Scenario Outline: An object whose root files are broken is refused when opened
    Given the object file "<file>" is <fault>
    When I open the object
    Then the read should be refused with OCFL code "<code>"

    Examples:
      | file                  | fault                            | code |
      | 0=ocfl_object_1.1     | removed                          | E003 |
      | 0=ocfl_object_1.1     | replaced with "ocfl_object"      | E007 |
      | inventory.json        | removed                          | E063 |
      | inventory.json.sha256 | removed                          | E058 |
      | inventory.json.sha256 | replaced with "x inventory.json" | E060 |
      | v3/inventory.json     | replaced with "{}"               | E064 |

  # Source
  # - resources/prd.md F-051
  # - apps/filesquid/crates/ocfl/resources/srs.md OCFL-S-004, OCFL-S-002, OCFL-S-017
  # - resources/architecture/016-rust-ocfl-library.md (Published head (readers))
  # - resources/architecture/023-filesquid-ocfl-engine-and-parquet.md (Decision 2 to 4)
