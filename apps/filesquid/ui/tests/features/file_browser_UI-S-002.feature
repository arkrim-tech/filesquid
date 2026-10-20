# UI-S-002: The default UI MUST provide a Drive-style browser with navigation, breadcrumb, list and
# grid views, preview, upload, create folder, rename or move, remove and restore, and sidebar panel
# slots. UI-S-019: Every component MUST show a file's display name from the API and MUST NOT show its
# stored name. UI-S-014: a content handle is the only credential the UI MAY put in a URL, and only as
# a media `src`. UI-S-016: accessibility contracts. Task UI-F2.2 (global Task FS-2.2).
# Executable proof: tests/ui/file-browser*.test.ts (jsdom) and tests/e2e/preview/file-browser.spec.ts
# (production build served by vite preview).
Feature: File browser
  As a library owner
  I want to browse, preview and change the files of a library like in a drive
  So that every change is one recorded version and I only ever see the names I gave my files

  Background:
    Given the built UI is served by a FileSquid host
    And an object holds the folders "Contracts" and "Drawings" and files stored under unique names
    And each file has a display name that differs from its stored name

  # Happy path
  Scenario: The listing shows folders first in the list layout
    When I open the file browser on the object's root
    Then the "Files" table should list "Contracts" and "Drawings" before the files
    And the columns should be "Name", "Size", "Type", "Modified" and "Version"
    And the list layout should be the pressed layout

  Scenario: Every name is the display name and never the stored name
    When I open the file browser, open a folder, open a file and rename a file
    Then no row, tile, heading, breadcrumb segment, status message, accessible name or attribute should contain a stored name
    And each file should be named by the display name the API returned

  Scenario: A folder opens from the listing and from the breadcrumb
    When I open the folder "Contracts"
    Then the breadcrumb should list "All files" and "Contracts" with "Contracts" as the current page
    And the address should name the folder
    When I go back
    Then the root listing should be shown again

  Scenario: A file opens in a preview and closes back to its row
    When I open "readme.txt"
    Then the viewer should show "readme.txt" with its position among the files of the folder
    When I press Escape
    Then the listing should be shown with focus on the row "readme.txt"

  Scenario: The grid layout shows the same files as tiles
    When I choose the grid layout
    Then each file and folder should be a tile named by its display name
    And the grid layout should be the pressed layout

  # Writes: each is one API call that commits one OCFL version
  Scenario: Uploading files commits one version per file
    When I choose a file "scan.pdf" to upload into the current folder
    Then one upload call should be sent for "scan.pdf"
    And the status should say the upload committed a new version
    And "scan.pdf" should be listed

  Scenario: A new folder stays pending until a file is added
    When I create the folder "Photos"
    Then the folder should be listed as pending until a file is added
    And no version should be committed

  Scenario: Renaming or moving is one call that keeps the stored name unless the name changes
    When I rename "readme.txt" to "notes-final.txt"
    Then one move call should be sent and the status should name the committed version
    When I move a file to the folder "Drawings" without changing its name
    Then the move should keep the stored name

  Scenario: Removing is reversible and there is no delete forever
    When I remove "readme.txt"
    Then one remove call should be sent and the status should offer Undo
    When I press Undo
    Then one restore call should be sent from the version before the removal
    And "readme.txt" should be listed again
    And no control should delete a file permanently

  # Edge cases
  Scenario: A file over 64 MiB is refused before it is sent
    When I choose a file larger than 64 MiB to upload
    Then no upload call should be sent
    And the alert should name the file and the limit

  Scenario: A file of exactly 64 MiB is accepted
    When I choose a file of exactly 64 MiB to upload
    Then one upload call should be sent

  Scenario: A name that cannot be stored is refused before any call
    When I try to create a folder named "a/b", or with only spaces, or longer than 255 bytes
    Then the form should say why and no call should be sent

  Scenario: Dropped files upload into the folder shown
    When I drag files over the listing
    Then the listing should say "Drop to upload into" the folder
    When I drop the files
    Then each file should be uploaded into that folder

  # Virtualised listing
  Scenario: A long folder keeps only the rows near the viewport in the page
    Given a folder of 1,000 files
    When I open the folder
    Then fewer than 60 rows should be in the page
    And more pages of 200 entries should load as I scroll to the end
    And a row that stays in view should keep its element
    And a row that has focus should stay in the page when it scrolls away

  Scenario: Loading stops at the page limit and says so
    Given a folder with more entries than the page limit allows
    When I scroll to the end of what is loaded
    Then the browser should say how many items it shows

  # Error cases
  Scenario: A failed listing offers a retry
    Given the host cannot list the folder
    When I open the file browser
    Then an alert "Something went wrong" should be announced with a "Try again" button
    When the host recovers and I press "Try again"
    Then the listing should be shown

  Scenario: A refused write is shown with its label and changes nothing
    Given the host refuses removals as a conflict
    When I remove "readme.txt"
    Then the alert should say the files changed or another change is in progress
    And "readme.txt" should still be listed

  Scenario: Permission denied, offline and empty are told apart
    Given the host denies listing with the reason "not_permitted"
    Then the browser should show the permission denied state with "Not permitted"
    Given the browser is offline and the host cannot be reached
    Then the browser should show the offline state
    Given the folder holds nothing
    Then the browser should show the empty state and no error

  # Previews through content handles (UI-S-014)
  Scenario Outline: A native preview loads through a content handle
    When I open a file previewed as <kind>
    Then the <element> should be named by the display name
    And its source should be a content handle URL on the host's base
    And the content handle should not be in the page address, history, Web Storage, a cookie, rendered text or a link

    Examples:
      | kind  | element |
      | image | img     |
      | video | video   |
      | audio | audio   |

  Scenario: A media error mints one new handle and resumes
    Given a video is paused at 1 second
    When the media element reports an error
    Then exactly one new content handle should be minted
    And the video should resume at the same position, still paused

  Scenario: A second failure shows the error
    Given the new handle is refused too
    Then the preview should show "This file could not be loaded" and mint no third handle

  Scenario: Text is read with one ranged read of 64 KiB
    When I open a text file longer than 64 KiB
    Then one ranged read for the first 64 KiB should be sent
    And the preview should say it shows the first 64 KB only

  Scenario: A format it cannot show says so
    When I open "model-export.blend"
    Then the preview should say there is no preview for this format
    And no viewer should be shown

  # Accessibility (UI-S-016)
  Scenario: The listing is one Tab stop with list keys
    When I focus a row and press ArrowDown, End, Home and ArrowUp
    Then focus should move between rows and stop at the ends
    And exactly one name control in the listing should be in the Tab order

  Scenario: The layout holds at 320 CSS px and 200 % text size
    When the page is 320 CSS px wide at 200 % root font size, in list and in grid
    Then the page should not scroll sideways

  Scenario: Motion stops under reduced motion and focus is drawn
    When the user prefers reduced motion
    Then each button's transition duration should be 0s
    And a keyboard-focused button should have a 2px solid primary outline offset by 2px

  Scenario: Colours come from the theme tokens in light and dark
    When I switch between the light and the dark theme
    Then the upload button border, the row text and the table background should equal the primary, on-surface and surface tokens

  # Source
  # - resources/prd.md F-055, F-002, F-008, F-010
  # - apps/filesquid/ui/resources/srs.md UI-S-002, UI-S-014, UI-S-016, UI-S-019
  # - resources/architecture/028-filesquid-ui-packages-and-panel-composition.md
  # - resources/architecture/030-filesquid-api-and-transports.md
