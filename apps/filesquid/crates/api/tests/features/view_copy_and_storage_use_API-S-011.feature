# API-S-011: View operation, copy and storage use per collection (Task API-F2.2, part 2)
# SRS-F-062 / PRD F-062: drive-style views, sorting and storage use; API-S-027 display names
Feature: View operation, copy and storage use per collection
  As the owner of a FileSquid library
  I want to browse my library's nodes in a stable order, copy files and see what each collection stores
  So that the file browser can sort, page and copy without ever exposing stored names

  Background:
    Given a library with the collection "Holidays" and the virtual collection "Favourites"
    And the files "beta.jpg" of 4 bytes, "Alpha.jpg" of 9 bytes and "gamma.png" of 2 bytes in "Holidays"

  Scenario: A copy is a new file that keeps the display name and names its source
    When the owner copies "beta.jpg"
    Then the copy should have a new file identifier and be shown as "beta.jpg"
    And the copy's record should name the source as dcterms:source
    And the copy should be journaled as "file.copy" with opaque IDs only

  Scenario: A collection view sorts by size in both directions and pages stably
    When the owner pages through "Holidays" by "size" "ascending" one row at a time
    Then the rows should be "gamma.png, beta.jpg, Alpha.jpg"
    When the owner pages through "Holidays" by "size" "descending" one row at a time
    Then the rows should be "Alpha.jpg, beta.jpg, gamma.png"

  Scenario: A library view lists its collections as folders and no unfiled file
    Given the file "Zebra.txt" of another collection
    When the owner pages through the library by "name" "descending" one row at a time
    Then the rows should be "Holidays, Favourites"

  Scenario: A virtual collection lists its members
    Given "Alpha.jpg" and "gamma.png" are in "Favourites"
    When the owner pages through the virtual collection by "name" "ascending" one row at a time
    Then the rows should be "Alpha.jpg, gamma.png"

  Scenario: Last opened by me follows only my own openings
    Given the owner downloaded "Alpha.jpg"
    And another user opened "gamma.png" later
    When the owner pages through "Holidays" by "opened_by_me" "descending" one row at a time
    Then the rows should be "Alpha.jpg, beta.jpg, gamma.png"

  Scenario: Storage use per collection counts the history of its deleted files too
    Given "gamma.png" was deleted
    When the owner asks for the storage use of "Holidays"
    Then it should count 3 files and 6 contents

  Scenario: Views that are not built yet are refused
    When the owner opens the "favorites" view of the library
    Then the view should be refused as "invalid_request"
