# UI-S-027: Library page — on the desktop host, Location "{folder} · on {volume}" (libraryFactLocation,
# proposed) after Free space, from the library facts' folder_name and volume_name (API-S-038), left
# out when the host answers neither name. Mockup 41 (41-library-page.svg) does not draw this row; it
# takes the label/value style of the rows above it. UI-S-024: the visual constants.
# Owner decision 2026-11-02, verbatim: "RE: 5 - the desktop user can see folder name and volume so
# they can better manage their own private desktop".
# Scenarios only; Task UI-F2.22 binds them with jsdom tests and Playwright against vite build plus
# vite preview at 1440 x 900, in light and dark.
Feature: The library page names the library's folder and volume on the desktop
  As the owner of a desktop installation
  I want the library page to say which folder and volume hold my library
  So that I can find and manage my library on my own computer

  Background:
    Given the built default UI is served by vite preview at a 1440 x 900 viewport

  # Happy path
  Scenario Outline: The Location fact follows Free space
    Given the <theme> theme is shown
    And the desktop host reports the folder "Family archive" on the volume "Photos SSD" for the library "Family archive"
    When I open the library page of "Family archive"
    Then the fact "Location" should read "Family archive · on Photos SSD" directly after "Free space"
    And its label and value should use the label/value style of the other facts

    Examples:
      | theme |
      | light |
      | dark  |

  # Edge cases
  Scenario: Only the folder name is known
    Given the desktop host reports the folder "Family archive" and no volume name
    When I open the library page of "Family archive"
    Then the fact "Location" should read "Family archive"

  Scenario: A host that answers no names shows no Location fact
    Given the host answers no folder or volume name for the library
    When I open the library page
    Then no "Location" fact should be shown
    And no filesystem path should be shown anywhere on the page
