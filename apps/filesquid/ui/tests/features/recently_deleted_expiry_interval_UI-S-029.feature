# UI-S-029: Settings, Recently deleted — "Check for expired files every" with a number field (default
# 60) and "minutes", and "FileSquid checks at launch and then at this interval. 5 to 1,440 minutes."
# (settingsTrashExpiryInterval, settingsTrashExpiryIntervalHelp, proposed), after the expiry options,
# editable only by the admin, a value outside the bounds shown as the field's error. Mockup 49
# (49-settings-recently-deleted.svg) does not draw the field. API-S-019: expiry_interval_minutes.
# Owner decision 2026-11-02, verbatim: "RE: DE-S-014 - "at launch and then every hour" is the
# default, this should be a configurable setting".
# Scenarios only; Task UI-F2.17 binds them with jsdom tests and Playwright against vite build plus
# vite preview at 1440 x 900, in light and dark.
Feature: The expiry interval in Settings, Recently deleted
  As the owner of a desktop installation
  I want to set how often FileSquid checks for expired deleted files
  So that deleted files expire on the schedule I choose

  Background:
    Given the built default UI is served by vite preview at a 1440 x 900 viewport
    And the Settings page is open at Recently deleted

  # Happy path
  Scenario Outline: The interval field shows the stored interval
    Given the <theme> theme is shown
    And the host answers expiry_interval_minutes 60
    Then the field "Check for expired files every" should show 60 "minutes"
    And it should read "FileSquid checks at launch and then at this interval. 5 to 1,440 minutes."

    Examples:
      | theme |
      | light |
      | dark  |

  Scenario: The owner changes the interval
    When I set the interval to 15 minutes and save
    Then the settings route should receive expiry_interval_minutes 15

  # Edge cases
  Scenario: A caller who is not the admin sees the interval read-only
    Given the caller may not change the settings
    Then the interval field should be shown read-only

  # Error case
  Scenario Outline: An interval outside the bounds is shown as the field's error
    When I set the interval to <minutes> minutes
    Then the field should show its error in the error colour
    And nothing should be sent

    Examples:
      | minutes |
      | 4       |
      | 1441    |
