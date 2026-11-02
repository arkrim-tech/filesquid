# API-S-027: Display and download names — the file-facts route also answers the file's fixity
# (state verified, failed with the OCFL code of the open error finding, or not_checked, and
# checked_at, from the latest validation of the file's object, API-S-022) and its resources (the
# original content files of the head version, never the metadata record, PROV-O records or journal
# files, each with display name, extension, size and SHA-256, at most 100, marked truncated).
# Mockup 35 (35-panel-file-info.svg): Fixity "Verified {time}" and RESOURCES IN THIS FILE.
# Owner decision 2026-11-02, verbatim: "make sure each API row issues gets a fix in MVP".
# Scenarios only; Task API-F2.33 adds their step definitions (unmatched steps are reported as skipped).
Feature: Fixity and resources in the file facts
  As a library owner
  I want to see when a file's content was last verified and which files it holds
  So that I can trust that what I keep is intact

  # Happy path
  Scenario: A validated file is verified at the time of its check
    Given fixity file "facade-study.jpg" whose object a validation checked at "2026-11-02T10:00:00Z" with no finding
    When fixity owner reads the file facts
    Then fixity answer should carry state verified and checked_at "2026-11-02T10:00:00Z"

  Scenario: A RAW+JPEG pair lists both originals
    Given fixity file "facade-study.jpg" whose head version holds a paired original "facade-study.cr3"
    When fixity owner reads the file facts
    Then fixity resources should list "facade-study.jpg" and "facade-study.cr3" with extension, size and SHA-256
    And fixity resources should not list the metadata record, PROV-O records or journal files

  # Edge cases
  Scenario: A file never validated is not checked
    Given fixity file whose object no validation has checked
    When fixity owner reads the file facts
    Then fixity answer should carry state not_checked and a null checked_at

  Scenario: More than 100 resources are truncated
    Given fixity file whose head version holds 101 original content files
    When fixity owner reads the file facts
    Then fixity resources should list 100 and be marked truncated

  # Error case
  Scenario: A digest finding marks the fixity failed
    Given fixity file whose object has an open error finding for a content digest
    When fixity owner reads the file facts
    Then fixity answer should carry state failed with that finding's OCFL code
