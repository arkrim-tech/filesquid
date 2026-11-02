# API-S-031: Self-scoped journal — GET /api/v1/journal (action journal.read_own, granted to every
# accepted credential) answers only the entries whose actor is the caller, with the filters, paging
# and redaction of the admin journal route, resolving names only for files the caller may read, and
# grants none of the admin routes (ADR 033). On the desktop the caller is the owner.
# Owner decision 2026-10-31, verbatim: "accept recommendations" (decision 3).
# Scenarios only; Task API-F2.27 adds their step definitions (unmatched steps are reported as skipped).
Feature: Self-scoped journal route
  As a user who is not an administrator
  I want to read the record of my own activity
  So that I can see what I did without seeing anyone else's activity

  Background:
    Given own-journal entries by user "ana" and by user "ben"

  # Happy path
  Scenario: A caller reads only their own entries
    When own-journal caller "ana" reads the self-scoped journal
    Then own-journal answer should hold only entries whose actor is "ana"

  Scenario: The desktop owner reads the owner's entries
    Given own-journal entries by the desktop owner and by the host itself
    When own-journal desktop owner reads the self-scoped journal
    Then own-journal answer should hold the owner's entries and no entry of the host itself

  # Edge case
  Scenario: The admin journal filters and paging apply
    When own-journal caller "ana" reads the self-scoped journal filtered by operation "file.upload" one page of 2 at a time
    Then own-journal pages should hold only "ana"'s file.upload entries, with no repeat or gap

  # Error and privacy cases
  Scenario: The self-scoped route grants no admin route
    When own-journal caller "ana" reads the admin journal
    Then own-journal answer should be refused as forbidden and journaled as api.auth.denied

  Scenario: The self-scoped answer keeps the journal's redaction
    Given own-journal entry of "ana" about an upload of a file with a title and a path
    When own-journal caller "ana" reads the self-scoped journal
    Then own-journal entry should carry opaque IDs only, with no title, path, content or credential, exactly as the admin journal route would answer it

  Scenario: Names are resolved only for files the caller may read
    Given own-journal entry of "ana" about a file "ana" may no longer read
    When own-journal caller "ana" reads the self-scoped journal with names
    Then own-journal entry should carry no name for that file
