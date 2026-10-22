# SRS-NFR-006: Accessibility and usability. Web interfaces target WCAG 2.2 AA; keyboard-only
# tasks, native screen readers, visible focus, scalable text and high-contrast and
# reduced-motion preferences are supported on every host and in every distribution.
# UI-S-016: accessibility contracts every @filesquid/ui custom element meets: keyboard reach,
# skip control, landmarks and accessible names from the label table, a 2px --fs-focus-ring focus
# ring at 3:1 with no contrast exception, focus recovery, reduced motion, contrast preferences, text scaling and RTL hooks.
Feature: Accessibility contracts in the shell
  As a library owner who uses a keyboard, a screen reader or larger text
  I want every part of the app to be reachable, named and readable
  So that I can manage my library whatever way I use my computer

  Background:
    Given the built UI is served by a FileSquid host

  # Happy path
  Scenario: The shell is reachable from the keyboard in a predictable order
    When I press Tab repeatedly from the top of the page
    Then focus should move to "Skip to main content", then the chosen theme option, then "Monitor"
    And every focused control should show a visible focus ring in the focus-ring colour at 3 to 1 or more

  Scenario: A screen reader hears named landmarks and controls
    When I list the page's landmarks and controls
    Then I should find the banner, the "Main navigation" navigation and the main area
    And the product name should be the only top-level heading
    And every control and image should have a name from the active label table

  Scenario: Skipping straight to the main area
    Given the skip control has focus
    When I activate it
    Then focus should be on the main area
    And the page address should not change

  # User interaction
  Scenario: Opening the Monitor moves focus to the main area
    Given the "Monitor" link has focus
    When I activate it
    Then the Monitor should be shown
    And focus should be on the main area, not lost

  Scenario: Signing in to the Monitor keeps focus in the Monitor
    Given the Monitor asks for the admin token and the token field has focus
    When I type the token and press Enter
    Then focus should move to the Monitor's first section heading

  # Edge cases
  Scenario: Reduced motion turns off transitions
    Given my operating system asks for reduced motion
    When I open the app
    Then no component should animate or transition

  Scenario: High contrast keeps borders and the chosen option visible
    Given my operating system asks for more contrast
    When I open the app
    Then borders should use the secondary text colour
    When my operating system forces its own colours
    Then the chosen theme option should use the system highlight colours

  Scenario Outline: A narrow window, large text, or both together never need horizontal scrolling
    Given the window is <width> CSS pixels wide and text is at <scale> percent
    When I open the app
    Then the page should not scroll horizontally
    And the product name should not be clipped

    # 320 px at 200 % is the narrowest window with the largest text, both together.
    Examples:
      | width | scale |
      | 320   | 100   |
      | 320   | 200   |
      | 640   | 200   |

  Scenario: Large text on a wide window keeps the side-by-side layout
    Given the window is 1280 CSS pixels wide and text is at 200 percent
    When I open the app
    Then the page should not scroll horizontally
    And the navigation should sit beside the main area

  Scenario: A right-to-left language mirrors the layout
    Given the distribution's locale is right-to-left
    When I open the app
    Then the theme control should sit at the left end of the header
    And the navigation should sit on the right of the main area

  # Error cases
  Scenario: A refused Monitor token keeps focus on a usable field
    Given I am signing in to the Monitor from the keyboard
    When the API refuses the token
    Then focus should be on the token field again
    And the reason should be announced as an alert

  Scenario: A distribution whose focus-ring colour fails 3:1 is rejected
    Given a distribution's "--fs-focus-ring" colour is below 3:1 on its sidebar colour
    When its token test runs
    Then the test should fail and name the focus pair and theme

  # Source
  # - ../../../../../resources/srs.md SRS-NFR-006
  # - ../../resources/srs.md UI-S-016
  # - ../../../../../resources/prd.md NFR-006, NFR-007, RC-005
