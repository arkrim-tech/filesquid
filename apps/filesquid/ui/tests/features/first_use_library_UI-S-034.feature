# UI-S-034: First use (mockup 57-first-use-library.svg). UI-S-027: the recent libraries of the welcome
# screen (mockup 01-welcome.svg) and the library page's Free space fact (mockups 41 and 09).
# UI-S-024: the visual constants. API-S-038: library facts and recent libraries. PRD F-001, F-004, F-009.
# Owner answer 4, verbatim: "The first collection should be created automatically upon install or first
# use if it doesnt already exists. Prompt the user for a name and location." Owner confirmation
# 2026-10-22, verbatim: "1. confirmed": the name and location are the new library's name and folder.
# Owner answer 10, verbatim: "yes add library facts to v3". Owner answer 13, verbatim: "Yes, proposed
# strings are approved".
# Executable proof: tests/ui/first-use.test.ts, tests/ui/library-recent.test.ts and
# tests/e2e/preview/first-use.spec.ts (Playwright against vite build plus vite preview at 1440 x 900,
# in light and dark).
Feature: First use creates the first library and its first collection
  As a new FileSquid owner on the macOS desktop
  I want to name my first library, choose its folder and start with a collection
  So that I can upload straight away without learning the library model first

  Background:
    Given the built default UI is served by vite preview at a 1440 x 900 viewport
    And the host has a native folder dialog and keeps a library registry

  # Happy path
  Scenario Outline: With no library known the welcome screen opens the first-use dialog
    Given the <theme> theme is shown
    And the host knows no library
    When I open the library start page
    Then a dialog "Create your first library" of 600 x 470 px should open over the welcome screen
    And the dialog should read "FileSquid keeps your files in a library, in a folder you choose."
    And the dialog should read "Its first collection is created for you, so you can upload straight away."
    And the name field should have the focus
    And the note "First collection: untitled collection" should be shown on the raised surface
    And "Create library" should be the only filled primary button, 38 px tall

    Examples:
      | theme |
      | light |
      | dark  |

  Scenario: Create makes the library with its name and then its first collection
    Given the host knows no library
    And the first-use dialog is open
    When I type the name "Family archive"
    And I choose a folder in the host's folder dialog
    And I press "Create library"
    Then the host should receive the folder only as an opaque location ID
    And the library should be created with the name "Family archive" before its first collection
    And the first collection should be created with the title "untitled collection"
    And the library page "Family archive" should open
    And no filesystem path should be shown or sent

  Scenario: A blank name lets the host name the library
    Given the first-use dialog is open and a folder is chosen
    When I leave the name blank and press "Create library"
    Then the library should be created without a name

  # Edge cases
  Scenario: Create waits for the folder
    Given the first-use dialog is open
    When no folder has been chosen
    Then "Create library" should be shown disabled
    And pressing it should send nothing

  Scenario: A cancelled folder dialog changes nothing
    Given the first-use dialog is open
    When I cancel the host's folder dialog
    Then the dialog should say "No folder was chosen. Nothing changed."
    And "Create library" should stay disabled

  Scenario: Cancel closes the dialog and Create opens it again
    Given the first-use dialog is open
    When I press "Cancel"
    Then the dialog should close and nothing should be sent
    And pressing the Create card of the welcome screen should open the dialog again

  Scenario: The import dialog can follow first use
    Given the first-use dialog creates the library and its first collection
    Then the dialog should announce that first use is done with the library ID and the collection ID
    And it should not offer an import entry point of its own

  # Error cases
  Scenario: A failure after the library exists keeps the library
    Given the first-use dialog is open and a folder is chosen
    And the host refuses the first collection
    When I press "Create library"
    Then the library should be kept and the dialog should say so
    And the error should offer "Try again" for the collection
    And pressing "Try again" should send only the collection command, under the same operation ID
    And the folder dialog should not open again and the library should not be created twice

  Scenario: A refused library is worded and the folder stays chosen
    Given the first-use dialog is open and a folder is chosen
    And the host refuses the library because the folder is not empty
    When I press "Create library"
    Then the dialog should say the folder cannot become a library and nothing was written
    And pressing "Create library" again should not open the folder dialog

  # Launch
  Scenario: A launch with no address of its own goes to first use
    Given the host knows no library
    When the app opens with no address of its own
    Then the welcome screen with the first-use dialog should be shown

  Scenario: A host without a library registry shows no first use
    Given the host answers "unavailable" for the recent libraries
    When I open the library start page
    Then the three choices should be shown with no dialog and no recent libraries

  # Recent libraries
  Scenario Outline: The welcome screen lists the recent libraries with the free space of their disks
    Given the <theme> theme is shown
    And the host has opened "Family archive" today with 1.4 TB free and "Studio work" earlier with 197 GB free
    When I open the library start page
    Then the eyebrow "RECENT LIBRARIES" should be 11 px weight 400
    And each library should show its name at 14 px weight 600 and "Opened {when} · {free} free on its disk" at 13 px
    And each row should have a ghost "Open" button of 76 x 24 px and rows should be 60 px apart
    And no path, library ID or location ID should be shown

    Examples:
      | theme |
      | light |
      | dark  |

  Scenario: A volume that cannot be read shows no free figure
    Given the host cannot read the volume of "Old disk"
    When I open the library start page
    Then the row of "Old disk" should read "Opened {when} · free space unknown"

  Scenario: Open goes to a library that is open and reopens one that is not by its folder
    Given "Family archive" is open on the host and "Studio work" is only remembered
    When I press "Open" on "Family archive"
    Then the library page of "Family archive" should open
    When I press "Open" on "Studio work"
    Then the host's folder dialog should ask for the folder and the open route should receive its location ID

  # Library page facts
  Scenario: The library page is titled with the library's name and shows its free space
    Given the host reports 1.4 TB free on the volume that holds "Family archive"
    When I open the library page of "Family archive"
    Then the title and the breadcrumb should read "Family archive"
    And the fact "Free space" should read "1.3 TB on the disk that holds this library"

  Scenario: The library page does not guess free space it was not given
    Given the host cannot read the volume that holds the library
    When I open the library page
    Then the fact "Free space" should say it is not known and show no figure
