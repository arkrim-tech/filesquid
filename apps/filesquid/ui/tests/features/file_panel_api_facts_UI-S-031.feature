# UI-S-031: File info — Fixity "Verified {time}" and the eyebrow RESOURCES IN THIS FILE with each
# resource's badge, name and short digest (mockup 35-panel-file-info.svg); panel error states with
# the mono code and the shortened operation ID, "{code} · op {id}", and View in Logs (mockup
# 25-panel-states.svg). UI-S-020: the Provenance tab lists activities newest first.
# API-S-027 (fixity, resources), API-S-009 (operation_id on errors), API-S-029 (order newest_first).
# Owner decision 2026-11-02, verbatim: "make sure each API row issues gets a fix in MVP".
# Scenarios only; Task UI-F2.22 binds them with jsdom tests and Playwright against vite build plus
# vite preview at 1440 x 900, in light and dark.
Feature: The file panel shows fixity, resources, operation IDs and the newest activities
  As a library owner
  I want the file panel to show what the mockups draw
  So that I can see when a file was verified, what it holds and which operation failed

  Background:
    Given the built default UI is served by vite preview at a 1440 x 900 viewport
    And the file "facade-study.jpg" is shown in the file panel

  # Happy path
  Scenario: File info shows the fixity check and the resources
    Given the file facts answer fixity verified 2 hours ago and the resources "facade-study.jpg" and "facade-study.cr3"
    When I open the File info tab
    Then the row "Fixity" should read "Verified 2 hours ago"
    And the eyebrow "RESOURCES IN THIS FILE" should list both resources with badge, name and short digest

  Scenario: A panel error names its operation
    Given the history read is refused with the code "not_found" and an operation ID
    When I open the Provenance tab
    Then the error state should show "not_found · op" with the shortened operation ID in mono
    And View in Logs should open the Logs view filtered to that operation

  Scenario: The Provenance tab lists the newest activities of a long history
    Given the file has 5,001 activities
    When I open the Provenance tab
    Then the history should be asked for newest first
    And the first activity shown should be the newest

  # Edge cases
  Scenario: A file not checked yet says so
    Given the file facts answer fixity not_checked
    When I open the File info tab
    Then the row "Fixity" should say the file has not been checked yet

  Scenario: A network failure shows its code alone
    Given the history read fails without an answer
    When I open the Provenance tab
    Then the error state should show the code without an operation ID

  # Error case
  Scenario: A failed fixity check is shown as an error
    Given the file facts answer fixity failed with an OCFL code
    When I open the File info tab
    Then the row "Fixity" should show the failure and its code in the error colour
