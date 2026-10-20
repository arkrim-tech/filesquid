# F-061: Activity journal and live health monitoring
# SRS-F-061: Activity journal and live health monitoring
# API-S-022: OCFL validation findings MUST reach health and the Monitor
# UI-S-005: Every distribution's label table defines every default key
# UI-S-016: Accessibility contracts (Monitor tables named by their headings)
# Task UI-F2.14: OCFL validation findings in the Monitor
# Executable proofs: tests/ui/ocfl-findings.test.ts, tests/ui/monitor.test.ts,
#   tests/e2e/preview/ocfl-findings.spec.ts and tests/e2e/ocfl-findings.spec.ts
Feature: OCFL validation findings in the Monitor
  As the desktop owner or a server administrator
  I want the Monitor to list what OCFL validation found in each open library
  So that a stray file or a broken object is noticed, never silent

  Background:
    Given I am signed in to the Monitor as the desktop owner or a server administrator

  # Happy path
  Scenario: An E023 finding shows with its code beside the degraded health reason
    Given validation found an unlisted file under an object's head version (E023)
    When the Monitor loads
    Then the health section should say "Degraded" with the OCFL validation errors reason naming "E023" and one object
    And the errors table of the OCFL validation section should list the library, object, version, code "E023" and location

  Scenario: The finding disappears after a clean revalidation, without a reload
    Given the errors table lists an E023 finding
    When a later validation no longer reports it and a health.change event arrives
    Then the errors table should say there are no open errors
    And the health section should say "OK"
    And the page should not have reloaded

  Scenario: The last and next run times come from the findings route
    Given a library that last ran from "2026-10-20T10:00:00Z" to "2026-10-20T10:00:04Z" and runs next at "2026-10-27T10:00:00Z"
    When the Monitor loads
    Then the validation runs table should show those times, the outcome, and the objects checked and not checked
    And the schedule should show the time between runs and the objects per run in force

  # Edge cases
  Scenario: A W010 warning is listed apart from errors and health stays ok
    Given validation found an object whose head has no version inventory (W010)
    When the Monitor loads
    Then the warnings table should list "W010" and the errors table should say there are no open errors
    And the health section should say "OK"
    And a note should say that the library's files in that object can be read but not changed

  Scenario: The W010 note links to the repair offer when the distribution has one
    Given the distribution gives the address of the version-inventory repair offer
    And validation found a W010 warning
    When the Monitor loads
    Then the read-only note should link to that library's repair offer

  Scenario: No location is shown that the API withholds
    Given the findings route withholds a finding's location from me
    When the Monitor loads
    Then the finding's location cell should say "Not shown"
    And no storage path should appear anywhere in the section

  Scenario: More findings are open than the route lists
    Given the findings route says its list is truncated
    When the Monitor loads
    Then the section should say that more findings are open than are listed

  Scenario: A host with no open library
    Given the server host has no OCFL library open
    When the Monitor loads
    Then the validation runs table should say there is nothing open to validate

  # User interactions
  Scenario: Validate now starts a run of every open library
    When I press "Validate now"
    Then a run of every open library should be requested
    And the section should say how many runs started and how many did not
    And the section should read the findings again

  Scenario: A validation run's progress updates the section live
    Given the findings section is shown
    When an "ocfl.validation" process update arrives on the event channel
    Then the section should read the findings again without a reload

  # Error cases
  Scenario: The findings route is unavailable
    Given the findings route answers 503 unavailable
    When the Monitor loads
    Then the OCFL validation section should show the unavailable state with Retry
    And the other Monitor sections should still show their data

  Scenario: The API refuses the request to start runs
    Given the run route answers 403 "not_permitted", for example because my session ended
    When I press "Validate now"
    Then the section should say that validation could not start and why, in words from the label table

  Scenario: A library's run is not started
    Given a run of one open library is already going
    When I press "Validate now"
    Then the section should name that library and the reason "already_running"

  Scenario: The event channel asks for a resync
    Given the findings section is shown
    When an "events.resync" event arrives
    Then the Monitor should read health, processes, the journal and the findings again

  Scenario: A truncated list gives a lower bound for read-only objects
    Given the findings route says its list is truncated and lists a W010 warning
    When the Monitor loads
    Then the read-only note should say "at least" that many objects
