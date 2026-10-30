# API-S-011: Destination — a move MUST name collection_id (and MAY name a folder); a request
# without it MUST be refused as destination_required; a move publishes one snapshot version,
# keeps the file's name, rewrites no bytes and is journaled once as file.move with opaque IDs
# only. Same logical path (ADR 041 Decision 12): a move, a rename or a folder creation
# (/api/v1/files/folders) at a taken collection logical path MUST be numbered beside it
# (directoryY-1, then directoryY-2; Harbour-1.jpg), never refused or overwritten, and
# answered with the path used; after 9,999 numbers it MUST be refused as
# path_suffix_exhausted with no change. Each change raises navigation.change for the folders
# it changed (API-S-021). A folder is only the prefix of its files' paths (ADR 041
# Decision 3), so a new folder writes nothing and stays pending until a file is placed in it.
# Task API-F2.14 part 2 (contract 0.35.0).
Feature: Moves, renames and folders in the home collections
  As a library owner
  I want to move, rename and add folders in my collections without losing anything
  So that a file never overwrites another and each item sits where I put it

  Background:
    Given a library whose collection "Holidays" receives uploads

  # Happy path
  Scenario: A move gives a file another folder and keeps its name
    Given the owner has uploaded "beach.jpg" into the folder "Summer"
    When the owner moves it into the folder "Archive"
    Then the file should now be registered at "Archive/beach.jpg" in "Holidays"
    And the answer should name the collection and the path "Archive/beach.jpg"
    And the move should be journaled once without its name or folder
    And both folders should be announced as changed

  Scenario: A rename gives a file its new name in its folder
    Given the owner has uploaded "IMG_0001.jpg" into the folder "Summer"
    When the owner renames it to "Harbour.jpg"
    Then the file should now be registered at "Summer/Harbour.jpg" in "Holidays"
    And the answer should carry the path "Summer/Harbour.jpg"
    And the folder should be announced as changed

  Scenario: A new folder is answered with its path and writes nothing
    When the owner creates the folder "filepathX/Fresh" in "Holidays"
    Then the folder should be answered at "filepathX/Fresh" and pending
    And nothing should be registered

  # Edge cases
  Scenario: A move into a taken path is numbered beside it
    Given the owner has uploaded "beach.jpg" into the folder "Summer"
    And the path "Archive/beach.jpg" is already taken in "Holidays"
    When the owner moves it into the folder "Archive"
    Then the file should now be registered at "Archive/beach-1.jpg" in "Holidays"

  Scenario: A rename to a taken name is numbered beside it
    Given the owner has uploaded "IMG_0001.jpg" into the folder "Summer"
    And the path "Summer/Harbour.jpg" is already taken in "Holidays"
    When the owner renames it to "Harbour.jpg"
    Then the file should now be registered at "Summer/Harbour-1.jpg" in "Holidays"

  Scenario Outline: A new folder at a taken path is numbered beside it
    Given the path "<taken>" is already taken in "Holidays"
    When the owner creates the folder "<asked>" in "Holidays"
    Then the folder should be answered at "<numbered>" and pending
    And nothing should be registered

    Examples:
      | taken                              | asked                | numbered               |
      | filepathX/directoryY/a.jpg         | filepathX/directoryY | filepathX/directoryY-1 |
      | Summer/notes                       | Summer/notes         | Summer/notes-1         |

  Scenario: A second folder at the same path takes the next number
    Given the path "filepathX/directoryY/a.jpg" is already taken in "Holidays"
    And the path "filepathX/directoryY-1/b.jpg" is already taken in "Holidays"
    When the owner creates the folder "filepathX/directoryY" in "Holidays"
    Then the folder should be answered at "filepathX/directoryY-2" and pending

  # Error cases
  Scenario: A move without a collection is refused
    Given the owner has uploaded "beach.jpg" into the folder "Summer"
    When the owner moves it without naming a collection
    Then the move should be refused as "destination_required" and journaled once
    And the file should now be registered at "Summer/beach.jpg" in "Holidays"

  Scenario: A rename that cannot be placed changes nothing
    Given the owner has uploaded "a.txt" into the folder "Summer"
    And the path "Summer/full.txt" and every number up to 9999 beside it are taken in "Holidays"
    When the owner renames it to "full.txt"
    Then the rename should be refused as "path_suffix_exhausted" and journaled once
    And the file should now be registered at "Summer/a.txt" in "Holidays"

  Scenario: A new folder without a collection is refused
    When the owner creates the folder "Summer" without naming a collection
    Then the new folder should be refused as "destination_required" and journaled once

  Scenario: A new folder inside a reserved folder is refused
    When the owner creates the folder "provenance" in "Holidays"
    Then the new folder should be refused as "invalid_path" and journaled once
