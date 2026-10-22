# SRS-UI-007: Light and dark themes. Tokens are CSS custom properties with the --fs- prefix and
# the Arkrim role names; every distribution defines every required token for light and dark;
# custom elements style colour only through tokens; the theme follows the OS colour scheme by
# default; a theme control switches light and dark and the choice persists across launches;
# primary actions and body text meet WCAG 2.2 AA in both themes in every distribution; no
# mockup pixel matching.
# UI-S-015: the shell shows the active distribution's product name and logo (with a
# dark-surface variant when it has one) and the theme control; the choice is kept on the device.
Feature: Light and dark themes and official branding
  As a library owner
  I want the app to look right in light and dark and to carry its distribution's brand
  So that it is comfortable to read in either theme and I know which product I am using

  Background:
    Given the built UI is served by a FileSquid host

  # Happy path
  Scenario: The theme follows the operating system by default
    Given I have never chosen a theme on this device
    And my operating system prefers dark
    When I open the app
    Then the theme control should show "System" as chosen
    And the shell should use the dark token values

  Scenario: Choosing dark keeps dark after a relaunch
    Given my operating system prefers light
    When I choose "Dark" in the theme control
    Then the shell should use the dark token values
    When I reopen the app
    Then the theme control should show "Dark" as chosen
    And the shell should use the dark token values

  Scenario: Choosing System returns to the operating system scheme
    Given I chose "Dark" earlier on this device
    And my operating system prefers light
    When I choose "System" in the theme control
    Then the shell should use the light token values
    And no theme choice should be kept on the device

  # User interaction
  Scenario: The theme control works from the keyboard
    Given the theme control shows "Dark" as chosen
    When I move to the previous option with the arrow key
    Then "Light" should be chosen and focused
    And the shell should use the light token values

  Scenario: The FileSquid default shows the official mark in both themes
    Given the FileSquid default distribution is active
    When I open the app in the light theme
    Then the header should show the FileSquid mark with a black outline and the product name "FileSquid"
    When I switch to the dark theme
    Then the header should show the FileSquid mark with a white outline

  Scenario: ImageSquid shows its own name, mark and primary colours
    Given the ImageSquid distribution is active
    When I open the app in the light theme
    Then the header should show the ImageSquid mark and the product name "ImageSquid"
    And the primary colour should be "#005db6" with "#ffffff" text
    When I switch to the dark theme
    Then the primary colour should be "#a9c7ff" with "#003063" text

  # Edge cases
  Scenario Outline: Text pairs meet WCAG 2.2 AA except the owner's listed design-guide pairs
    Given the <distribution> distribution is active in the <theme> theme
    When I measure the contrast of each text colour on its surface
    Then every pair not in FileSquid's exempt list (all pairs for ImageSquid) should reach at least 4.5 to 1
    And every FileSquid exempt pair should use exactly its listed design-guide colours
    And the focus ring should reach at least 3 to 1 on every surface

    Examples:
      | distribution | theme |
      | FileSquid    | light |
      | FileSquid    | dark  |
      | ImageSquid   | light |
      | ImageSquid   | dark  |

  Scenario: Device storage is unavailable
    Given the app cannot use device storage
    When I choose "Dark" in the theme control
    Then the shell should use the dark token values until the app closes
    And the console should explain that the choice will not be kept

  # Error cases
  Scenario: A distribution forgets a dark token
    Given a distribution token file without a dark value for "--fs-error"
    When the token test runs
    Then it should fail naming "--fs-error" for the dark theme

  Scenario: A distribution chooses a primary colour below AA that is not an exempt value
    Given a distribution token file whose primary is "#5cd685" with "#ffffff" text
    When the token test runs
    Then it should fail naming the primary pair at 1.84 to 1

  Scenario: The design guide's primary with white text is accepted only at its listed values
    Given a distribution token file whose primary is "#16ac49" with "#ffffff" text
    When the token test runs
    Then it should not report the primary pair at 2.99 to 1
    When the primary is changed to "#17ad4a" with "#ffffff" text
    Then it should fail naming the primary pair

  Scenario: The focus ring has no contrast exception
    Given a distribution token file whose focus ring is "#16ac49" on a "#ffffff" surface
    When the token test runs
    Then it should fail naming the focus-ring pair at 2.99 to 1

  Scenario: A component uses a literal colour
    Given a custom element stylesheet with a literal colour value
    When the component style test runs
    Then it should fail naming that component

  # Source
  # - resources/srs.md SRS-UI-007, SRS-F-058
  # - apps/filesquid/ui/resources/srs.md UI-S-005, UI-S-015
  # - apps/imagesquid/resources/srs.md IS-S-008, IS-S-009
  # - resources/prd.md NFR-006, F-058
