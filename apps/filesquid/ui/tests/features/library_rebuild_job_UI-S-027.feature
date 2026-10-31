# UI-S-027: Library lifecycle screens, Recover from a copy (mockup 04-recover-library.svg). API-S-025:
# the rebuild as a durable job. API-S-037: job routes and job events. UI-S-024: the visual constants.
# PRD F-001, F-009, F-042.
# Owner answer 9, verbatim: "Yes, rebuild should be a durable job". Library revisions restart at 1 after a
# host restart (API-S-025; owner 2026-10-31 accepted "no change for the MVP"), so the page never assumes
# a revision persists.
# Executable proof: tests/ui/library-rebuild.test.ts and tests/e2e/preview/first-use.spec.ts
# (Playwright against vite build plus vite preview at 1440 x 900, in light and dark).
Feature: The rebuild from files runs as a durable job the owner can follow
  As a FileSquid owner who recovers a library from a copy
  I want to see the rebuild's progress and to pause, resume or cancel it
  So that a long rebuild never blocks me and never repairs anything behind my back

  Background:
    Given the built default UI is served by vite preview at a 1440 x 900 viewport
    And the host runs the rebuild as a durable job and reports its progress in objects

  # Happy path
  Scenario Outline: Recover shows the running job without the shell
    Given the <theme> theme is shown
    When I recover a library from a copy
    Then the page "Recover from a copy" should be shown without the sidebar, header and footer
    And a raised card of 1040 x 112 px should show "Rebuild catalog from files" with the chip "running"
    And the card should read "1,204 of 2,418 objects read · started 10:41 · resumes from its checkpoint after you quit"
    And the progress bar should be 4 px tall and about 50% full
    And "Pause" and "Cancel" should be secondary buttons of 92 x 36 px
    And the note "Nothing in the copy is repaired or deleted. Objects with errors are listed in the Monitor under OCFL validation." should be shown

    Examples:
      | theme |
      | light |
      | dark  |

  Scenario: Pause holds the job and Resume continues it from its checkpoint
    Given the rebuild job is running
    When I press "Pause"
    Then the chip should read "paused" and the button "Resume" should replace "Pause"
    When I press "Resume"
    Then the chip should read "running" again

  Scenario: Cancel stops the rebuild and says nothing was changed
    Given the rebuild job is running
    When I press "Cancel"
    Then the chip should read "cancelled" and the controls should be removed
    And the page should say "The rebuild was cancelled. Nothing in the library's files was changed."

  Scenario: The finished job shows its report and the errors found
    Given the rebuild job is running
    When the host reports the job completed
    Then the page should read "Read 1 object(s) from files. Not readable: 0. With validation errors: 1."
    And it should read "The copy is open as a library. Its catalog was rebuilt from the OCFL files."
    And the eyebrow "VALIDATION ERRORS SO FAR" should list each object with errors on one mono line in a 1 px error box

  # Edge cases
  Scenario: A library's own rebuild is followed inside the shell
    Given I am on the page of an open library
    When I press "Rebuild catalog from files"
    Then the host should start one rebuild job for the library at its current revision
    And the rebuild page should open inside the shell and follow that job

  Scenario: The job is followed live and re-read when events are missed
    Given the rebuild job is running
    When the host sends a job update, or the event channel resynchronises
    Then the page should show the newest progress without a reload
    And while the job is active the page should also re-read it from time to time, a bounded number of times

  Scenario: A control in flight blocks a second press
    Given I pressed "Pause" and the host has not answered
    When I press "Pause" or "Cancel" again
    Then no second request should be sent

  # Error cases
  Scenario: A job that failed is shown as failed without the host's free text
    When the host reports the job failed with an error code
    Then the chip should read "failed" outlined in the error colour
    And the page should say "The action did not finish. Nothing was reported as changed."

  Scenario: A refused control is worded and the controls stay
    Given the host refuses "Pause" as a conflict
    When I press "Pause"
    Then the page should say "The action did not finish. Nothing was reported as changed."
    And "Pause" and "Cancel" should still be available

  Scenario: A job that cannot be read offers Try again, and an unknown job is not found
    Given the host cannot answer the job read
    Then the page should offer "Try again" that reads the job again
    And a job the host does not know should be reported as no longer available

  Scenario: Nothing private is shown
    When the rebuild page is shown in any state
    Then it should show no filesystem path, no job ID and no library ID
