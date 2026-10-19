# OCFL-S-012: The engine exposes a serializable UI view API (folder listing by logical-path prefix with paging, file details, version history, size, fixity, ranged reads, version-producing writes) that returns storage facts only; an extension-based type label is a display hint only.
# OCFL-S-016: Concurrent commits (the view API's version-producing writes use the change model; there is no second write path)
# OCFL-S-018: Write order and unpublished versions (a head without its version inventory stays refused)
# F-055: Default UI and panel composition
# C-055: File facts have one owner (storage facts only)
Feature: UI view API
  As a FileSquid UI showing an OCFL object like a drive
  I want listings, details, history, fixity and ranged reads of storage facts, and writes that each commit one version
  So that every UI browses and edits objects through one engine API without a second write path

  Background:
    Given an object whose "v1" holds "readme.md" and "docs/a.txt" and whose "v2" changes "docs/a.txt"

  # Happy path
  Scenario: The root lists implied folders before files
    When the root folder of "v2" is listed
    Then the listing should show "docs/" then "readme.md"
    And "docs/a.txt" should show its size, digest and the version that last changed it

  Scenario: Adding a file beneath a new folder persists the folder
    When a file "pending/first.txt" is added through the view API
    Then the object head should be "v3" through the view API
    And the root folder of "v3" should show "pending/"

  Scenario: Moving a folder commits one version and writes no bytes
    When the folder "docs" is moved to "archive/docs" through the view API
    Then the object head should be "v3" through the view API
    And the view write should have written nothing
    And "archive/docs/a.txt" should exist at "v3"

  # Edge case
  Scenario: A listing is paged with an opaque cursor
    When the root folder of "v2" is listed one entry per page
    Then the first page should show "docs/" and give a cursor
    And the next page should show "readme.md" and give no cursor

  # Error cases
  Scenario: A view write on a head without its version inventory is refused and not repaired
    Given the head version inventory was removed
    When a file "x.txt" is added through the view API
    Then the view write should be refused for the missing version inventory
    And the staging area should be empty

  Scenario: Ranged reads are bounded
    When a range larger than the read bound is requested
    Then the read should be refused as an invalid request
