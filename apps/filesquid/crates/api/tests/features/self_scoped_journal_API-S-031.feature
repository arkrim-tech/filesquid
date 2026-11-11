# API-S-031: Self-scoped journal — GET /api/v1/journal (action journal.read_own, granted to every
# accepted credential) answers only the entries whose actor is the caller, with the filters, paging
# and redaction of the admin journal route, resolving names only for files the caller may read, and
# grants none of the admin routes (ADR 033). On the desktop the caller is the owner.
# Owner decision 2026-10-31, verbatim: "accept recommendations" (decision 3).
# Step definitions: tests/support/own_journal_steps.rs (Task API-F2.27). Server accounts arrive
# later, so "ana" is the authoriser's owner principal under a policy that grants it nothing and
# "ben" is the administrator; the journal names each by its principal's actor.
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

  Scenario: The caller cannot choose whose entries to read
    When own-journal caller "ana" reads the self-scoped journal filtered by actor "ben"
    Then own-journal answer should be refused as an invalid request naming the actor parameter

  # Error and privacy cases
  Scenario: A call without an accepted credential is refused
    When own-journal is read without a credential and with a credential the host does not accept
    Then own-journal answers should be unauthorized and forbidden, with no entry

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
