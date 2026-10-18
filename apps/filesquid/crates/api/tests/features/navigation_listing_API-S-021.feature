# F-004: Collections, folders and navigation
# SRS-F-004: Collections, folders and navigation (the navigation tree and breadcrumb come from the API)
# F-049: Product component terminology and item type
# API-S-021: Navigation listing: one level of the tree per call, ancestors for the breadcrumb, permission-filtered paging
# API-S-019: Library domain commands and queries (the source of libraries, collections and virtual collections)
# API-S-005: Responses carry opaque handles, never raw filesystem paths or SQL
#
# Step definitions arrive with Task API-F1.11; until then these scenarios are reported as skipped.
Feature: Navigation listing
  As a person browsing a FileSquid library
  I want the API to list the levels of my navigation tree and the path to where I am
  So that the tree and breadcrumb show my repository, libraries, collections, virtual collections and folders without a UI guessing them

  Background:
    Given a navigation fixture with library "Family" holding collection "Photos" and virtual collection "Best of 2025"
    And collection "Photos" holds files at logical paths "2025/summer/beach.jpg" and "2025/winter/snow.jpg"
    And library "Family" holds an unfiled file at logical path "inbox/note.txt"

  # Happy path
  Scenario: The root level is the host's repository
    When the owner lists the navigation tree without a node
    Then the answer should hold exactly one node of kind "repository"

  Scenario: A library lists its collections, virtual collections and unfiled folders in kind order
    When the owner lists the children of library "Family"
    Then the kinds should be "collection", "virtual_collection" and "folder" in that order
    And the folder should be named "inbox"

  Scenario: A collection lists the folders its files imply
    When the owner lists the children of folder "2025" in collection "Photos"
    Then the answer should hold folders "summer" and "winter"
    And no file should be listed

  Scenario: One call fills the breadcrumb
    When the owner lists the children of folder "2025/summer" in collection "Photos"
    Then the ancestors should be the repository, "Family", "Photos", "2025" and "summer" in that order

  # Edge cases
  Scenario: A virtual collection has no children here
    When the owner lists the children of virtual collection "Best of 2025"
    Then the answer should be empty
    And the virtual collection should report no children

  Scenario: A folder implied only by hidden or deleted files is not listed
    Given every file under "2025/winter" in collection "Photos" is deleted
    When the owner lists the children of folder "2025" in collection "Photos"
    Then the answer should hold only folder "summer"

  Scenario: Paging is stable across many siblings
    Given collection "Photos" holds 1001 top-level folders
    When the owner pages through the children of collection "Photos" 200 at a time
    Then every folder should appear exactly once in a stable order

  Scenario: Node handles are opaque
    When the owner lists the children of folder "2025" in collection "Photos"
    Then no node handle should contain a filesystem path, storage key, OCFL content path, SQL or token
    And no node handle should be longer than 512 bytes

  Scenario: A change names the parents whose children changed
    Given a client subscribed to the event channel
    When the owner renames collection "Photos" to "Pictures"
    Then the client should receive one "navigation.change" event naming the handle of library "Family"

  # Error cases
  Scenario Outline: An unusable page request is refused
    When the owner lists the children of library "Family" with <request>
    Then the request should be refused as "invalid_request"

    Examples:
      | request                 |
      | a malformed cursor      |
      | a limit of 0            |
      | a limit of 1001         |

  Scenario: Unknown and unreadable nodes get the same answer
    Given a second user who may not read collection "Photos"
    When that user lists the children of collection "Photos" and of a node that does not exist
    Then both answers should be the same "not_found" error
    And library "Family" should report children to that user only if another child is readable
