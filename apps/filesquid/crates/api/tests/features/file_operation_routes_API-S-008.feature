# F-056: Single FileSquid API
# F-051: OCFL module
# API-S-004: Every call is authenticated and authorised before files, SQL or keys are touched
# API-S-005: Responses carry opaque handles, never raw filesystem paths or SQL
# API-S-008: Every content or organisation change commits an OCFL version through the change model; there is no second write path
# API-S-017: Every authorisation refusal is journaled; successful calls are not journaled one by one
# OCFL-S-012: The UI view API lists folders by logical-path prefix with paging, reads ranges and makes version-producing writes
# Owner requirement (2026-10-19): users always see and download a file under its original name, never its sanitized stored name
#
# Step definitions: tests/support/file_steps.rs (Task API-F1.3), against a real OCFL root on the
# local filesystem.
Feature: File operation routes
  As a library owner
  I want to browse and change the files of my library through the one FileSquid API
  So that every client sees the same files and every change is a recoverable OCFL version

  Background:
    Given the owner's library holds an object with "photos/a.jpg", "photos/b.jpg" and "notes.txt"

  # Happy path
  Scenario: The owner lists a folder by its logical-path prefix
    When the owner lists the folder "photos"
    Then the listing should name "photos/a.jpg" and "photos/b.jpg"
    And no answer should carry a filesystem path or a storage key

  Scenario: An upload publishes exactly one OCFL version
    When the owner uploads "photos/c.jpg"
    Then the object should have exactly one new version
    And the activity journal should record one "files.upload" entry with opaque identifiers only

  Scenario: A move and a restore each publish one version
    When the owner moves "photos" to "pictures"
    And the owner restores "photos" from version v1
    Then the object should have exactly two new versions
    And "photos/a.jpg" and "pictures/a.jpg" should both be readable

  Scenario: Users see and download a file under its original name
    Given the owner's original name of "photos/a.jpg" is "Été à Nice 2023.jpg"
    When the owner lists the folder "photos"
    And the owner downloads "photos/a.jpg"
    Then the listing should show "photos/a.jpg" as "Été à Nice 2023.jpg"
    And the download should be named "Été à Nice 2023.jpg" with an ASCII fallback

  # Edge cases
  Scenario: A new empty folder stays pending until a file is added beneath it
    When the owner creates the folder "albums"
    Then the folder should be pending and no version should be published
    When the owner uploads "albums/first.jpg"
    Then the folder "albums" should be listed

  Scenario: Folder listings page with an opaque cursor
    When the owner lists the root one entry at a time
    Then every entry should be listed once, folders first

  Scenario: A ranged read is capped at 16 MiB
    Given the object holds a 20 MiB file "big.bin"
    When the owner reads "big.bin" without a range
    Then the answer should be 206 with the first 16 MiB

  # Error cases
  Scenario: A write that changes nothing is refused without a version
    When the owner uploads "notes.txt" with its current content
    Then the write should be refused as "conflict"
    And no version should be published
    And the activity journal should record one refused "files.upload" entry

  Scenario: A client without the token cannot read or write files
    When a client without a token reads "notes.txt"
    And a client without a token removes "notes.txt"
    Then both calls should be refused as "unauthorized"
    And no version should be published
