# API-S-011: Collection moves, renames and folders — a collection move takes the expected revision
# of the file's place, a revision that changes whenever the file's place changes, and refuses a
# mismatch as HTTP 409 with a conflict outcome, changing nothing (ADR 016: conflicting writers
# rebase or report a conflict, never silently overwrite). A rename also moves the collection path,
# and a retry completes a relocation that failed after the name commit. A collection folder is
# answered as pending and stores nothing; a successful one is not journaled. The in-object shapes
# stay accepted. Reserved folders are reserved at a collection's root only.
# Owner decision 2026-10-31, verbatim: "accept recommendations" (decision 7).
# Scenarios only; Task API-F2.25 adds their step definitions (unmatched steps are reported as skipped).
Feature: Collection moves compare the expected revision
  As a library owner using two devices
  I want a move made from an old view to be refused, not applied over a newer move
  So that a file never ends up somewhere I did not choose last

  Background:
    Given move-revision file "beach.jpg" in collection "Summer" read at its current place revision

  # Happy path
  Scenario: A move with the current place revision applies once
    When move-revision caller moves it to collection "Archive" with that revision
    Then move-revision file should be in collection "Archive" with a new place revision

  # Edge cases
  Scenario: A retried move answers its first outcome
    Given move-revision caller moved it to "Archive" under operation ID "op-9"
    When move-revision caller sends the same move under "op-9" again
    Then move-revision answer should repeat the first outcome and nothing more change

  Scenario: A new collection folder is answered as pending and not journaled
    When move-revision caller asks for folder "Trips" in collection "Summer"
    Then move-revision answer should be the free path "Trips" as pending, with nothing stored and no journal entry

  # Error case
  Scenario: A second move made from the same read is a conflict
    Given move-revision first caller moved it to "Archive" with that revision
    When move-revision second caller moves it to "Work" with that same revision
    Then move-revision answer should be HTTP 409 with a conflict outcome
    And move-revision file should still be in collection "Archive"
    And move-revision refusal should be journaled once as failed
