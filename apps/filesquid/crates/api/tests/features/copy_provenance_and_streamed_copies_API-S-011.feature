# API-S-011: Copy — a copy's first version carries a PROV-O record that derives the copy's file
# entity from the source's file entity at the copied version (prov:wasDerivedFrom across objects,
# SRS-F-067), and dcterms:source names the source. A copy of a file version of any size streams
# through bounded ranged reads and writes and never holds the file in memory (NFR-003), on every
# host, the desktop included.
# Owner decisions 2026-10-31, verbatim: "accept recommendations" (decision 10) and "build streamed copies".
# Scenarios only; Tasks API-F2.23 and API-F2.26 add their step definitions (unmatched steps are reported as skipped).
Feature: Copy provenance and streamed copies
  As a library owner
  I want a copy of any size that remembers where it came from
  So that large files can be copied and the history explains every copy

  # Happy path
  Scenario: A copy's record derives it from its source
    Given copy-history file "report.pdf" at version v3 in collection "Work"
    When copy-history caller copies it into collection "Archive"
    Then copy-history first version record of the copy should derive its file entity from "report.pdf" at v3
    And copy-history record should validate against filesquid-prov/1

  Scenario Outline: A copy of any size streams with bounded memory
    Given copy-history file of <size> in collection "Work"
    When copy-history caller copies it into collection "Archive"
    Then copy-history copy should have the source's SHA-256 and size
    And copy-history copy should never hold the whole file in memory

    Examples:
      | size    |
      | 1 MiB   |
      | 65 MiB  |
      | 600 MiB |

  # Edge case
  Scenario: The history route of the copy names its source
    Given copy-history copy of "report.pdf" at version v3
    When copy-history caller reads the history of the copy
    Then copy-history first activity should name "report.pdf" at v3 as its source

  # Error case
  Scenario: A source that changes during a streamed copy fails with nothing left staged
    Given copy-history file of 65 MiB that changes while it is read
    When copy-history caller copies it into collection "Archive"
    Then copy-history copy should fail with nothing committed and nothing left staged
