# UI-S-026: Browser views, menus and dialogs (the Version column, Download, the bulk bar's
# "{inside} files inside", Recent with its actions, move or copy into a collection, and the ZIP
# dialog's statement that the owner's own ZIP keeps the metadata in its files)
# UI-S-030: Search composition (the collection of each live file and result, from the row)
# API-S-011 and API-S-012: the row facts the API answers (library, head version, collection)
# PRD F-062 and F-063. Owner decision 2026-10-31, verbatim: "accept recommendations" (decisions 2, 14 and 15 (a)).
# Tasks UI-F2.20 and API-F2.22 write the Playwright tests behind these scenarios against "vite build" plus "vite preview".
Feature: Row facts in the Drive
  As a library owner
  I want each row to show its version and collection and to act on its own library
  So that downloads, moves and copies work wherever the file lives

  Background:
    Given two open libraries "Family" and "Work", each with files in collections

  # Happy path
  Scenario: The Version column shows each file's head version
    When I open the list view of collection "Summer" in "Family"
    Then each file row should show its head version from the API in the Version column

  Scenario: Download sends the row's own library and version
    When I choose Download on "budget.xlsx" of library "Work"
    Then the download request should name library "Work" and the row's head version

  Scenario: Search results and live files name their collection
    When I type "beach" in the search field
    Then each live file should show the collection title its row carries

  Scenario: The bulk bar counts the files inside selected folders
    When I select the folder "Trips" of 3 files and one file of 2 KB
    Then the bulk bar should read "1 file and 1 folder (3 files inside) · {size}" from fileSelectionScopeInside

  Scenario: Recent shows what I did with each file
    When I open Recent
    Then each row should name its action, changed, opened or uploaded, under its day group, newest first

  Scenario: Move or copy into a collection lists the file under the answered path
    When I copy "beach.jpg" into collection "Archive" folder "2026" with the destination picker
    Then the copy request should send collection_id and folder "2026"
    And the copy should be listed under the path the API answered

  Scenario: Storage used names the history it includes
    Given the API reports a library total of 2 GB with history bytes of 300 MB
    When the sidebar shows Storage used
    Then the caption should read "incl. 300 MB history" from fileStorageCaptionHistory (owner decision 2026-10-31, verbatim: "well do a")

  Scenario: The ZIP dialog says the owner's own ZIP keeps metadata
    When I open Download ZIP for 3 files
    Then the dialog should show "Metadata kept" and "Files are downloaded as they are stored. Location and identity details in them are not removed. Sharing applies your privacy settings." from the label table
    And no "exif:GPS* removed" chip should be shown

  # Edge cases
  Scenario: A folder count the API marks incomplete is worded as incomplete
    When I select a folder whose file count the API marks incomplete
    Then the bulk bar should not state an exact number of files inside

  Scenario: A file in no collection shows no collection name
    When a search result row carries a null collection
    Then no collection name should be shown for it

  # Error case
  Scenario: A file action uses the library named on its row, not a fallback
    Given the row of "notes.md" names library "Work" while "Family" is also open
    When I rename "notes.md"
    Then the rename request should name library "Work"
