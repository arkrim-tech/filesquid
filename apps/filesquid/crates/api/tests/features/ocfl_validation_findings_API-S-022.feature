# API-S-022: OCFL validation findings MUST reach health and the Monitor
# SRS-F-061 / PRD F-061: activity journal and live health monitoring
# SRS-F-042 / PRD F-042: arbitrary external edits are detected and reported, not adopted
# OCFL-S-018 "Known gap": a fenced-out writer can leave a stray file under vN/ (E023)
Feature: OCFL validation findings in health and the Monitor
  Every host runs the OCFL validator over every object of every open library: on a schedule,
  on request, and after a commit that took over a reservation. Error findings keep health
  degraded until a later validation no longer reports them; warnings are listed and do not
  change health. Every finding, error or warning, is journaled once when it opens. The
  journal and health carry opaque object IDs, versions and codes, never a path or content.

  Scenario: A stray file under vN/ degrades health until it is removed
    Given an open library with a valid object and an object with a stray file under its head version
    When the owner requests a validation run
    Then health should be "degraded" with reason "ocfl_validation_errors" naming "E023" in 1 object
    And one "ocfl.validation.finding" entry for "E023" should be journaled without paths
    When the stray file is removed and the library is validated again
    Then health should be "ok"

  Scenario: An object without a version inventory is a warning only
    Given an open library with an object whose head has no version inventory
    When the owner requests a validation run
    Then the findings should list "W010" as a warning
    And one "ocfl.validation.finding" entry for "W010" should be journaled without paths
    And health should be "ok"

  Scenario: A takeover commit validates that object
    Given an open library with a valid object and an object with a stray file under its head version
    When a commit on the object with the stray file takes over an expired reservation
    Then health should be "degraded" with reason "ocfl_validation_errors" naming "E023" in 1 object

  Scenario: The administrator sees findings without locations
    Given an open library with a valid object and an object with a stray file under its head version
    When the owner requests a validation run
    Then the owner should see where the "E023" finding is
    And the server administrator should see the "E023" finding without its location

  Scenario: A capped run reports what it did not check and the next run resumes
    Given an open library with 3 valid objects and a run cap of 2
    When the owner requests a validation run
    Then the run should be "capped" with 2 objects checked and 1 not checked
    When the owner requests a validation run
    Then the cursor should have wrapped to the first object

  Scenario: Deleting an object closes its findings
    Given an open library with a valid object and an object with a stray file under its head version
    When the owner requests a validation run
    And the object with the stray file is deleted and the library is validated again
    Then one "ocfl.validation.finding_closed" entry for "E023" should be journaled without paths
    And health should be "ok"
