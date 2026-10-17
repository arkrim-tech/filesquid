# OCFL-S-007: The engine reaches bytes only through an async, runtime-agnostic storage backend interface
# F-051: OCFL module
Feature: Storage backend interface
  As a host that embeds filesquid-ocfl
  I want to choose where OCFL bytes are stored
  So that the same engine works on a local disk, in object storage and in tests

  # Happy path
  Scenario Outline: Bytes written through a backend read back unchanged
    Given an empty <backend> storage backend
    When I put "hello OCFL" at "objects/a/inventory.json"
    Then "objects/a/inventory.json" should exist with size 10
    And streaming "objects/a/inventory.json" should return "hello OCFL"

    Examples:
      | backend          |
      | in-memory        |
      | local filesystem |

  # Error case: a competing writer moved the head
  Scenario Outline: A stale compare-and-swap does not overwrite the head
    Given an empty <backend> storage backend
    And "objects/a/inventory.json" holds "v1" and I remember its entity tag
    When another writer replaces "objects/a/inventory.json" with "v2"
    And I replace "objects/a/inventory.json" with "v3" only if it still matches the remembered entity tag
    Then the write should be rejected as a precondition failure
    And streaming "objects/a/inventory.json" should return "v2"

    Examples:
      | backend          |
      | in-memory        |
      | local filesystem |

  # Error case: create-only reservation
  Scenario Outline: Creating a key that already exists is rejected
    Given an empty <backend> storage backend
    And "objects/a/v2/reservation" holds "writer-1" and I remember its entity tag
    When I create "objects/a/v2/reservation" with "writer-2" only if it is absent
    Then the write should be rejected as a precondition failure
    And streaming "objects/a/v2/reservation" should return "writer-1"

    Examples:
      | backend          |
      | in-memory        |
      | local filesystem |

  # Edge case: each read returns a bounded chunk
  Scenario Outline: Multipart objects are read back in bounded chunks
    Given an empty <backend> storage backend
    And a 3 MiB file at "objects/a/v1/content/big.bin" uploaded in 1 MiB multipart parts
    When I stream "objects/a/v1/content/big.bin" in 256 KiB chunks
    Then every chunk should be at most 256 KiB
    And the streamed bytes should equal the uploaded bytes

    Examples:
      | backend          |
      | in-memory        |
      | local filesystem |

  # Edge case: bounded listing
  Scenario Outline: Listing pages through keys in lexical order
    Given an empty <backend> storage backend
    And 5 files under "objects/"
    When I list "objects/" with a page size of 2
    Then I should receive 3 pages holding all 5 keys in lexical order

    Examples:
      | backend          |
      | in-memory        |
      | local filesystem |

  # Error case: path escape
  Scenario Outline: Keys that escape the storage root are rejected
    Given an empty <backend> storage backend
    When I put "x" at "../outside.txt"
    Then the write should be rejected as an invalid key

    Examples:
      | backend          |
      | in-memory        |
      | local filesystem |

  # Edge case: chunk limit
  Scenario Outline: A chunk larger than the maximum chunk size is refused
    Given an empty <backend> storage backend
    When I request a read chunk one byte larger than the maximum chunk size
    Then the read should be rejected as over the limit

    Examples:
      | backend          |
      | in-memory        |
      | local filesystem |

  # Source
  # - apps/filesquid/crates/ocfl/resources/srs.md OCFL-S-007
  # - resources/prd.md F-051
  # - resources/architecture/025-client-side-filesquid-engines.md Decision 2 and 3
  # - resources/architecture/012-s3-compatible-ciphertext-storage.md conflict guard
