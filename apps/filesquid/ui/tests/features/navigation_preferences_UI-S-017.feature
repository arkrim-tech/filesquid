# F-004: Collections, folders and navigation (navigation choices persist across restarts)
# SRS-F-004: Collections, folders and navigation
# F-062: Drive-style views, sorting, selection and layout preferences
# UI-S-017: Navigation preferences persist in the API's per-user preferences, or on the device in Web Storage
Feature: Navigation preferences
  As a person who organises a FileSquid library
  I want my pinned destinations, section order and collapsed sections kept between sessions
  So that the navigation tree opens the way I left it

  # Happy path
  Scenario: Pins, order and collapse survive a restart on the device
    Given a host whose API does not serve per-user UI preferences
    When the user pins collection "Photos", moves the Virtual collections section above Collections and collapses library "Archive"
    And the user restarts the application
    Then "Photos" should be pinned, the section order kept and "Archive" collapsed
    And device storage key "filesquid.navigation" should hold version 1

  Scenario: The device value moves to the API once
    Given device storage key "filesquid.navigation" holds pins, a section order and collapsed nodes
    And a host whose API serves per-user UI preferences
    When the UI starts
    Then the choices should be written to the per-user preferences once
    And device storage key "filesquid.navigation" should be removed

  # Edge cases
  Scenario: Only opaque handles are stored
    Given a host whose API does not serve per-user UI preferences
    When the user pins folder "2025/summer"
    Then the stored value should hold the folder's node handle and kind
    And it should hold no name, title, logical path, token or content handle

  Scenario Outline: An unusable stored value is ignored
    Given device storage key "filesquid.navigation" holds <value>
    When the UI starts
    Then the navigation tree should use its defaults
    And the console should say why the stored value was ignored

    Examples:
      | value                                   |
      | text that is not JSON                   |
      | a value with version 2                  |
      | a value with 101 pinned destinations    |
      | a value with 1,001 collapsed nodes      |

  Scenario: Refused device storage still applies the choice for this page
    Given device storage refuses access
    When the user collapses library "Archive"
    Then "Archive" should stay collapsed until the page closes
    And the console should say why the choice was not saved

  # Error cases
  Scenario: A pin whose node is gone is shown as unavailable
    Given collection "Old" is pinned
    And the API answers "not_found" for collection "Old"
    When the navigation tree loads
    Then the "Old" pin should be shown as unavailable with a remove action

  # User interactions
  Scenario: Sections are reordered from the keyboard
    Given the navigation tree has focus on the Collections section heading
    When the user presses the keyboard shortcut to move the section down
    Then the Collections section should follow the Virtual collections section
    And the new order should be saved

  # UI-F2.8: per-user preferences replace the device key
  Scenario: The device value moves to the API once
    Given the device holds "filesquid.navigation" with one pinned collection
    And the API serves per-user preferences with no navigation choices
    When the UI starts
    Then the choices should be copied to the API once
    And the device key should be removed
    And later changes should be saved through the API, not on the device

  Scenario: The API's own choices win over a device value
    Given the API already holds navigation choices
    And the device holds "filesquid.navigation"
    When the UI starts
    Then the API's choices should apply
    And the device key should be removed
