# OCFL-S-001: Git MUST NOT be the OCFL library; `.git` MUST NOT appear in an OCFL object or storage root
# F-042: OCFL preservation and portable recovery
Feature: Git guard and plain storage hierarchy
  As a preservation programme that copies OCFL storage with ordinary file tools
  I want the engine to refuse Git metadata, links and empty directories in the storage hierarchy
  So that every library stays plain, valid OCFL that never needs Git to recover

  # Happy path
  Scenario Outline: A plain storage root passes the guard
    Given an empty <backend> storage backend
    And an OCFL storage root with the object "ab/obj-1"
    When I run the storage root guard
    Then the guard should pass

    Examples:
      | backend          |
      | in-memory        |
      | local filesystem |

  # Error case: Git metadata at a root
  Scenario Outline: Git metadata in a storage root or object root is refused
    Given an empty <backend> storage backend
    And an OCFL storage root with the object "ab/obj-1"
    And a stored file at "<git file>"
    When I run the storage root guard
    Then the guard should refuse a .git directory in "<root>"

    Examples:
      | backend          | git file             | root     |
      | in-memory        | .git/HEAD            |          |
      | in-memory        | ab/obj-1/.git/config | ab/obj-1 |
      | local filesystem | .git/HEAD            |          |
      | local filesystem | ab/obj-1/.git/config | ab/obj-1 |
      | local filesystem | ab/.git              | ab       |

  # Edge case: preserved content may itself be a Git repository
  Scenario Outline: A Git repository preserved as object content is allowed
    Given an empty <backend> storage backend
    And an OCFL storage root with the object "ab/obj-1"
    And a stored file at "ab/obj-1/v1/content/repo/.git/HEAD"
    When I run the storage root guard
    Then the guard should pass

    Examples:
      | backend          |
      | in-memory        |
      | local filesystem |

  # Error case: an empty .git directory holds no files but is still refused
  Scenario: An empty .git directory is refused
    Given an empty local filesystem storage backend
    And an OCFL storage root with the object "ab/obj-1"
    And an empty directory at "ab/obj-1/.git"
    When I run the storage root guard
    Then the guard should refuse an empty directory at "ab/obj-1/.git"

  # Error case: empty directories
  Scenario: An empty directory in the storage hierarchy is refused
    Given an empty local filesystem storage backend
    And an OCFL storage root with the object "ab/obj-1"
    And an empty directory at "ab/obj-1/v1/content/empty"
    When I run the storage root guard
    Then the guard should refuse an empty directory at "ab/obj-1/v1/content/empty"

  # Error case: soft links
  Scenario: A soft link in the storage hierarchy is refused
    Given an empty local filesystem storage backend
    And an OCFL storage root with the object "ab/obj-1"
    And a soft link at "ab/obj-1/v1/content/link.txt" to a file outside the root
    When I run the storage root guard
    Then the guard should refuse a soft link at "ab/obj-1/v1/content/link.txt"

  # Error case: hard links
  Scenario: A hard link in the storage hierarchy is refused
    Given an empty local filesystem storage backend
    And an OCFL storage root with the object "ab/obj-1"
    And the file "ab/obj-1/inventory.json" is hard linked from outside the root
    When I run the storage root guard
    Then the guard should refuse a hard link at "ab/obj-1/inventory.json"

  # Source
  # - apps/filesquid/crates/ocfl/resources/srs.md OCFL-S-001
  # - resources/prd.md F-042
  # - resources/architecture/016-rust-ocfl-library.md (Layout constraints)
