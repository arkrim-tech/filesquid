# UI-S-024: Visual constants of the active mockup suite (resources/ui-mockups/v3/).
# UI-S-025: Shell composition. UI-S-026: Browser views, menus and dialogs. UI-S-027: Library
# lifecycle screens. UI-S-028: Destination, drop target, import and jobs. UI-S-029: Recently
# deleted, collection deletion, restore, duplicate names, write refusals and their settings.
# UI-S-030: Search composition. UI-S-031: Preview and file panel composition. UI-S-032: Monitor and
# Logs composition. UI-S-033: Settings page composition. UI-S-034: First use.
# Owner decision 2026-10-22, verbatim: "yes, make v3 the active suite in that PR"; the owner's answers to the
# v3 README questions of the same day are quoted in the v3 README.
# Executable proof arrives with the tasks that build each surface (UI-F2.18 first, then UI-1.2,
# UI-F2.3, UI-F2.4, UI-F2.8, UI-F2.9, UI-F2.11, UI-F2.15, UI-F2.16, UI-F2.17): Playwright against
# vite build plus vite preview at 1440 x 900, in light and dark. UI-F2.18's proof is
# tests/e2e/preview/mockup-fidelity.spec.ts (the "Built surfaces" scenarios below).
Feature: The default UI matches the active mockup suite
  As a FileSquid owner on the macOS desktop
  I want every screen to look and read as the approved mockups do
  So that the product I use is the product the owner signed off

  Background:
    Given the built default UI is served by vite preview at a 1440 x 900 viewport
    And the API answers with the sample library "Family archive" of the mockups

  # Happy path
  Scenario Outline: The shell regions have the mockup sizes in both themes
    Given the <theme> theme is shown
    When I open the collection "Harbour project"
    Then the sidebar should be 240 px wide on the card surface
    And the header should be 60 px tall with the search field, the host badge and the Monitor button
    And the footer should be 40 px tall with the summary "Folders: 3 · Files: 11 · Loaded size: 1.5 GB"
    And the commit state should read "All changes committed to OCFL"

    Examples:
      | theme |
      | light |
      | dark  |

  Scenario Outline: Text and surface colours come from the UI-S-024 tokens
    Given the <theme> theme is shown
    When I open the collection "Harbour project"
    Then the page title should be 30 px Georgia weight 700 in <heading>
    And the sidebar logo mark should be 24 px tall
    And the list names should be 14 px in <body>
    And the eyebrow "LIBRARIES" should be 11 px weight 400 upper case in <muted>
    And the active sidebar row should be on <tint> with its text in <active>

    Examples:
      | theme | heading | body    | muted   | tint    | active  |
      | light | #000000 | #111111 | #777777 | #f0faf4 | #0f8a39 |
      | dark  | #f5f5f5 | #e0e0e0 | #6a6a6a | #0d2b18 | #4aca74 |

  Scenario: The tree is in sections by kind with one Recently deleted last
    Given the navigation tree of 55-navigation-tree-built.svg
    When I open the sidebar
    Then the sections should be "PINNED", "COLLECTIONS", "VIRTUAL COLLECTIONS" and "FOLDERS" in that order
    And the last row of the tree should be the single "Recently deleted" entry for every library this host has opened that I may read
    And there should be no separate "Deleted files" item

  Scenario: Buttons follow the design guide
    When I open the library page of "Family archive"
    Then every button label should be 14 px weight 600
    And buttons should have a 7 px radius
    And at most one filled primary button should be shown besides the New button

  # Edge cases
  Scenario: Upload and import are disabled at a library's top level
    When I open the empty library "Family archive"
    Then "Upload files" and "Upload folder…" should be disabled on the raised surface with placeholder text
    And "Open a collection to upload into it." should be shown

  Scenario: The storage caption shows history and free space on two lines
    When I open the collection "Harbour project"
    Then the storage caption should read "incl. 4.2 GB history" and "1.4 TB free on disk"

  Scenario: A day or less left is shown as a warning
    When I open "Recently deleted" of "Family archive"
    Then "1 day left" should be shown in the error colour
    And each row should name its library
    And every Restore button should be a secondary button

  Scenario: Delete now asks first
    When I choose "Delete now" on "minutes-2026-09.pdf" in "Recently deleted"
    Then a dialog should ask "Delete minutes-2026-09.pdf now?"
    And "Cancel" should have the focus

  Scenario: No API information on browsing screens
    When I open the collection "Harbour project"
    Then the header should not show the API status
    And Settings, About should show "API: ok · contract 0.21.0"

  # Error cases
  Scenario: A search operator the desktop cannot answer is refused
    When I search for "harbour sharedwith:me"
    Then the page should say "sharedwith: is not available on this device."
    And no result should be listed

  Scenario: A write refusal is worded from the label table
    Given the API refuses an upload with "destination_required"
    When I upload "report.pdf"
    Then an error card should say "Not uploaded: report.pdf"
    And it should say "Choose a collection or a folder inside one. Files cannot go to a library's top level."

  # User interactions
  Scenario: The host badge opens its popover from the keyboard
    When I focus the host badge and press Enter
    Then a popover should say "Your files stay on this device. Only you can read them."
    And pressing Escape should close it and return focus to the badge

  # Built surfaces (Task UI-F2.18, tests/e2e/preview/mockup-fidelity.spec.ts)
  Scenario Outline: The shell frame matches the mockups in both themes
    Given the <theme> theme is shown
    When I open the Monitor
    Then the sidebar should be 240 x 900 px on <card> with a 1 px <outline> edge
    And the New button should be 208 x 38 px, filled <primary> with "New" in <on_primary>
    And the view filter should be 208 x 34 px with the placeholder "Filter this view"
    And the search field should be 520 x 36 px with a 1.5 px <control> border and radius 6 px
    And the host badge should be 28 px tall on <raised> reading "This device"
    And the header should be 60 px tall and the footer 40 px tall
    And the header should show no theme control

    Examples:
      | theme | card    | outline | primary | on_primary | control | raised  |
      | light | #ffffff | #e0e0e0 | #16ac49 | #ffffff    | #8e8e8e | #f5f5f5 |
      | dark  | #1c1c1c | #2a2a2a | #16ac49 | #ffffff    | #757575 | #242424 |

  Scenario Outline: The welcome screen draws the three cards without the shell
    Given the <theme> theme is shown
    When I open the library start page
    Then the sidebar, header and footer should be hidden
    And the title "Your files. Private." should be 56 px Georgia weight 700
    And three cards of 332 x 248 px should be shown, the Create card with a 2 px <primary> border
    And the privacy box should state "Your files stay on this device. Only you can read them." from the host descriptor
    And the footer line should read "Open source under the MIT licence · Version 0.1.0 · macOS"

    Examples:
      | theme | primary |
      | light | #16ac49 |
      | dark  | #16ac49 |

  Scenario: The library page draws the facts, the actions and the W010 note
    When I open the library page of a library whose objects lack a version inventory
    Then the facts should be 13 px label and value rows with the values 160 px from the labels
    And the library ID should be shown in the monospace font
    And "Copy library to…" and "Rebuild catalog from files" should be secondary buttons
    And a 448 px note with a 1 px error border should say "Their latest version has no version inventory (W010)."
    And "Review repair" should be a ghost button

  Scenario: The repair consent dialog sits on the black 32 % scrim
    When I review the repair of 2 objects
    Then a 600 px dialog on the overlay surface should be titled "Repair 2 object(s)?" in 22 px Georgia weight 700
    And the scrim should be black at opacity 0.32
    And "Cancel" should have the focus
    And the confirm button should be the dialog's one primary button, 38 px tall

  Scenario: A view state is one centred treatment
    Given the folder chooser answers that it is unavailable
    When I try to open a library
    Then the state title should be 15 px weight 600 with its 24 px icon in the error colour
    And the reason should be 13 px in the secondary text colour
    And "Try again" should be a 36 px secondary button with a 7 px radius
    And the three choices should stay on the page

  Scenario: The Monitor draws its page head, cards, chips and stalled state
    Given a stalled process and an open E023 error and W010 warning
    When I open the Monitor
    Then the title "Monitor" should be 30 px with "Only you can open this on this device." and the "Live" chip on the active tint
    And "Download JSON Lines" should be a secondary button
    And each section should be a card with a 1 px subtle border and a 10 px radius
    And the health word should be 22 px Georgia weight 700 in the error colour
    And the stalled process's state should be in the error colour
    And the E023 and W010 codes should both be chips outlined in the error colour
