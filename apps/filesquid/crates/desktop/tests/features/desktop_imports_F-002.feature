# F-002: Safe file, folder and device import
# SRS-F-002: Safe file, folder and device import
# API-S-036: Import route: the host's own dialog, one durable import job per operation ID, no path in any request, answer or journal entry
# DE-S-019: Desktop file and folder import: native dialog, durable resumable job, one object per file with its import record, registered in the destination collection and folder
# API-S-011: Same logical path: a taken path is numbered beside it
# DE-S-018: Library routes over owner-chosen folders: the folder listing and the objects of a navigation scope answered from the index
# DE-S-006: The webview receives opaque handles and API responses, never raw filesystem paths or SQL
# DE-S-020: Desktop job routes: the import runner and controls, honoured after the file in hand
# API-S-037: Job routes and job events: pause, resume and cancel, each journaled once; per-file progress and retries
# API-S-036 Review: the plan before any byte is staged, started only when confirmed, discarded on request
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
    And the owner edits both picked files
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

  # Job controls (Task DE-1.9)
  Scenario: A paused import stops after the file in hand and resumes where it stopped
    Given the owner will pick a folder of 2 files
    And the import worker will be held on its first file
    When the app asks to import a folder under the operation "op-paused"
    And the owner pauses the import while its first file is in hand
    Then the import job should be paused with 1 of 2 files done
    And the job control "job.paused" should be journaled once
    When the owner resumes the import job
    Then the import job should be completed with 2 of 2 files done
    And 2 files should be imported, each as its own object
    And no journal entry should name a picked file or folder

  Scenario: A cancelled import ends after the file in hand and reads no further file
    Given the owner will pick a folder of 2 files
    And the import worker will be held on its first file
    When the app asks to import a folder under the operation "op-stopped"
    And the owner cancels the import while its first file is in hand
    Then the import job should be cancelled with 1 of 2 files done
    And the job control "job.cancel_requested" should be journaled once
    And only the file in hand should be imported

  # Review and per-file progress (Task DE-1.8)
  Scenario: An import is reviewed before any byte is copied and starts only when confirmed
    Given the owner will pick a folder of 2 files
    When the app asks to review a folder import under the operation "op-review"
    Then the plan should count 2 new files and 9 bytes to copy with nothing staged
    When the owner confirms the plan
    Then 2 files should be imported, each as its own object

  Scenario: A discarded plan imports nothing
    Given the owner will pick a folder of 2 files
    When the app asks to review a folder import under the operation "op-dropped"
    And the owner discards the plan
    Then confirming the plan should be refused as not found
    And no file should be imported

  Scenario: A file that failed keeps its code and is retried on its own
    Given the owner will pick a folder of 2 files
    And the import worker will be held on its first file
    When the app asks to review a folder import under the operation "op-failing"
    And the owner confirms the plan
    And the second picked file disappears while the first is in hand
    Then the second file should have failed as "source_missing" with the job waiting for a retry
    When the second picked file comes back and the owner retries it
    Then the import job should be completed with 2 of 2 files done
