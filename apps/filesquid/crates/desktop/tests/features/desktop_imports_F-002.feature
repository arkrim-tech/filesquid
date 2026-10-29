# F-002: Safe file, folder and device import
# SRS-F-002: Safe file, folder and device import
# API-S-036: Import route: the host's own dialog, one durable import job per operation ID, no path in any request, answer or journal entry
# DE-S-019: Desktop file and folder import: native dialog, durable resumable job, one object per file with its import record, registered in the destination collection and folder
# API-S-011: Same logical path: a taken path is numbered beside it
# DE-S-018: Library routes over owner-chosen folders: the folder listing and the objects of a navigation scope answered from the index
# DE-S-006: The webview receives opaque handles and API responses, never raw filesystem paths or SQL
Feature: Desktop file and folder import
  As the owner using the FileSquid desktop app
  I want to import files and folders I pick in my system's own dialog
  So that every file is kept safely in my library, even when the app quits mid-import

  Background:
    Given the desktop host has started its import route over an open library

  # Happy path
  Scenario: A picked folder is imported as one job, one object per file
    Given the owner will pick a folder of 2 files
    When the app asks to import a folder under the operation "op-holidays"
    Then the answer should name a started import job
    And 2 files should be imported, each as its own object
    And no journal entry should name a picked file or folder

  # Edge cases
  Scenario: Repeating an operation answers the same job without a second dialog
    Given the owner will pick a folder of 2 files
    When the app asks to import a folder under the operation "op-holidays"
    And the app asks to import a folder under the operation "op-holidays" again
    Then both answers should name the same import job
    And the dialog should have been shown once
    And 2 files should be imported, each as its own object

  Scenario: A cancelled dialog makes no job
    Given the owner will close the dialog without picking anything
    When the app asks to import files under the operation "op-cancel"
    Then the answer should say the import was cancelled
    And no file should be imported

  # Library model (ADR 041): every import names its destination
  Scenario: Imported files appear under the collection and folder the import names
    Given the owner will pick a folder of 2 files
    When the app asks to import a folder under the operation "op-trip"
    Then 2 files should be imported, each as its own object
    And the collection should list the folder "trips/Holidays" holding "Beach day.jpg" and "Notes.txt"

  Scenario: A folder imported again sits beside the first under a numbered name
    Given the owner will pick a folder of 2 files
    When the app asks to import a folder under the operation "op-first"
    And the import has finished with 2 files
    And the app asks to import a folder under the operation "op-again"
    Then the collection should list the folders "Holidays" and "Holidays-1" in "trips"

  # Error cases
  Scenario: An import that names no destination collection is refused before any dialog
    Given the owner will pick a folder of 2 files
    When the app asks to import a folder without naming a destination collection
    Then the import should be refused as an invalid request naming "destination_required"
    And the dialog should not have been shown
    And no file should be imported

  Scenario: A library that is not open is refused and journaled once without a path
    Given the owner will pick a folder of 2 files
    When the app asks to import into a library that is not open
    Then the import should be refused as not found
    And the dialog should not have been shown
    And the journal should record one import request without any path

  # Owner interaction
  Scenario: An import the host could not run now runs at the next launch
    Given the owner will pick a folder of 2 files
    And the import worker has stopped for the app to quit
    When the app asks to import a folder under the operation "op-later"
    Then the import should be refused as unavailable
    And after the app restarts 2 files should be imported, each as its own object
