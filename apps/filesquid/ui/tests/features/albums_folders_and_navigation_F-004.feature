# SRS-F-004: Albums, folders and navigation
Feature: Albums, folders and navigation
  As a library owner
  I want albums, folders and navigation
  So that the product matches the SRS

  Scenario: Happy path for F-004
    Given the feature is enabled on a supported profile
    When I use it as specified in the PRD
    Then the MUST criteria for that feature should hold

  Scenario: Adding a photo to a second album creates a hand-picked virtual collection
    Given a file in collection "Coastal Days"
    When I choose Add To "Family Time"
    Then "Family Time" should be a hand-picked virtual collection containing the file
    And the file should still live in collection "Coastal Days"
    And the UI should state once in this session that the second membership is a virtual collection

  Scenario: Hand-picked virtual collection order and cover persist
    Given a hand-picked virtual collection with files in order "a,b,c"
    When I drag "c" before "a" and choose "c" as the cover
    And I restart the application
    Then the order should be "c,a,b"
    And the cover should be "c"

  Scenario: Undo restores an album removal by name
    Given three selected files in collection "Coastal Days"
    When I remove them from the collection
    Then the Edit menu should offer "Undo Remove 3 Files from Collection"
    When I choose Undo from the keyboard
    Then the three files should be members of "Coastal Days" again
    And the removal and its inverse should both be in the activity journal and the version history

  Scenario: Undo restores a deleted collection
    Given a collection "Scenic Views" with five members
    When I delete the collection and confirm
    Then the Edit menu should offer "Undo Delete Collection"
    When I choose Undo
    Then "Scenic Views" should exist with the same five members in the same order

  Scenario: Limitation is visible for F-004
    Given a platform cannot provide a capability
    When I view that capability
    Then I should see an explicit unsupported or waiting state

  Scenario: Failure does not corrupt originals for F-004
    Given an operation fails
    When I inspect the library
    Then originals and committed OCFL snapshots should remain

  # FileSquid default UI: the navigation tree (UI-F2.1a). The tree reads one level at a time
  # from an injected navigation source shaped on the API's navigation listing (API-S-021); the
  # choices are kept on the device under "filesquid.navigation" (UI-S-017).

  # Happy path
  Scenario: The navigation tree groups a library's destinations into sections
    Given library "Family" holds collections "Photos" and "Hobbies", virtual collection "Best of 2025" and folder "inbox"
    When the navigation tree is shown
    Then the sections should be headed "Collections", "Virtual collections" and "Folders" from the label table
    And "Photos" should be a closed tree item whose folders are read when it opens

  Scenario: Pinning a destination adds it to a Pinned section
    Given the navigation tree shows collection "Hobbies"
    When I move to "Hobbies" and press "Pin"
    Then a "Pinned" section should list "Hobbies"
    And the Pin toggle for "Hobbies" should be pressed
    And device storage should hold the pin's handle and kind but not its name

  Scenario: Navigation choices persist across restarts
    Given libraries "Family" and "Archive"
    And I collapsed library "Archive", pinned "Photos" and moved the Collections section above Pinned
    When I restart the application
    Then the sections should read "Collections", "Pinned", "Virtual collections", "Folders"
    And "Archive" should be collapsed and "Photos" pinned

  # User interaction
  Scenario: The tree follows the ARIA treeview keys
    Given focus is on "Photos"
    When I press the right arrow
    Then "Photos" should open and keep focus
    When I press the right arrow again
    Then focus should move to "2025"
    When I press the left arrow
    Then focus should return to "Photos"
    When I type "h"
    Then focus should move to "Hobbies"

  Scenario: Sections are reordered from the keyboard
    Given focus is in the Collections section
    When I press Alt+Shift+Down, or activate "Move Collections down"
    Then the Collections section should follow the Virtual collections section
    And focus should stay where it was
    And the new order should be saved

  # Edge cases
  Scenario: The first section cannot move up
    Given "Collections" is the first section
    Then "Move Collections up" should be marked unavailable
    And activating it should not change the order

  Scenario: A malformed stored value is ignored
    Given device storage key "filesquid.navigation" holds a value with version 2
    When the navigation tree is shown
    Then the tree should use its defaults
    And the console should say why the stored value was ignored

  # Error cases
  Scenario: A pin whose destination is gone is shown as unavailable
    Given collection "Old" is pinned
    And the navigation source answers "not_found" for it
    When the navigation tree is shown
    Then the Pinned section should show "Collection (unavailable)" with a "Remove" action
    When I activate "Remove"
    Then the pin should be removed from the stored choices

  Scenario: Refused device storage keeps the choices for this session
    Given the browser refuses access to device storage
    When I move the Collections section down
    Then the new order should apply until the page closes
    And the console should say that the navigation choices will not be kept

  # Live navigation data (UI-F2.1b part 2; API-S-021)
  # Happy paths
  Scenario: The tree shows the connected host's navigation
    Given the host lists repository "Home" with library "Family" holding collection "Photos"
    When the default UI loads
    Then the navigation tree should list "Photos" in the Collections section
    And every level should be read through the navigation listing of the FileSquid API

  Scenario: A destination opens with its breadcrumb
    Given the host lists folder "2025" in collection "Photos" of library "Family"
    When I open folder "2025" from the navigation tree
    Then the breadcrumb should read the repository, "Family", "Photos", "2025"
    And "2025" should be the current page in the breadcrumb and in the tree
    And the breadcrumb should come from one listing's ancestors

  # Edge cases
  Scenario: A long level is read page by page
    Given library "Family" holds more collections than one page carries
    When the default UI loads
    Then the tree should follow next_cursor until it is null
    And every collection should be listed once, in the listing's order

  Scenario: A change reads only the levels it names
    Given the tree shows library "Family"
    When the host announces a navigation change naming library "Family"
    Then only the children of "Family" should be read again
    And the tree should show the new names without a page reload

  Scenario Outline: The whole tree is read again
    Given the tree shows library "Family"
    When <event>
    Then the whole visible tree should be read again

    Examples:
      | event                                                     |
      | the host announces a navigation change for all nodes      |
      | the event channel delivers an "events.resync" hint        |
      | the event channel opens again after a disconnect          |
      | the host announces a navigation change naming the repository |

  # Error cases
  Scenario: The host has no navigation service yet
    Given the host answers the navigation listing with HTTP 503 "unavailable"
    When the default UI loads
    Then the navigation tree should show the unavailable state with "Try again"
    And it should not show an empty tree
    When the host installs its navigation service and I choose "Try again"
    Then the tree should show the host's navigation

  Scenario: A destination that is gone
    Given the address names a destination the host answers "not_found" for
    When the destination view loads
    Then it should say that the destination is no longer available
    And no breadcrumb should be shown

  Scenario: An answer that breaks the contract is not shown
    Given the host answers a listing with a node of an unknown kind
    When the navigation tree loads
    Then the tree should show its error state

  # Source
  # - srs.md SRS-F-004
  # - prd.md F-004
  # - apps/filesquid/ui/resources/srs.md UI-S-001, UI-S-012, UI-S-016
  # - apps/filesquid/ui/resources/srs.md UI-S-017; apps/filesquid/crates/api/resources/srs.md API-S-021
