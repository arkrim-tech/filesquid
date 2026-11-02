# API-S-002: the metadata write routes (statement set PUT /api/v1/files/metadata/statements,
# statement removal POST /api/v1/files/metadata/removals and namespace registration
# POST /api/v1/files/metadata/registrations) take a CommandEnvelope and answer a CommandOutcome for
# the same operation ID, so a retry under the same operation ID never commits a second version.
# Owner decision 2026-10-31, verbatim: "accept recommendations" (decision 9).
# Scenarios only; Task API-F2.24 adds their step definitions (unmatched steps are reported as skipped).
Feature: Command envelopes on the metadata write routes
  As a library owner
  I want a retried metadata edit to be applied once
  So that a lost answer never leaves a second version in my history

  Background:
    Given metadata-envelope file "report.pdf" whose metadata record is at version "v7"

  # Happy path
  Scenario Outline: A metadata write answers a command outcome for its operation ID
    When metadata-envelope caller sends <route> under operation ID "op-1"
    Then metadata-envelope answer should be a committed outcome for "op-1"

    Examples:
      | route                  |
      | a statement set        |
      | a statement removal    |
      | a namespace registration |

  # Edge cases
  Scenario: A retry under the same operation ID commits no second version
    Given metadata-envelope caller set "dcterms:title" under operation ID "op-2"
    When metadata-envelope caller sends the same statement set under "op-2" again
    Then metadata-envelope answer should repeat the first outcome
    And metadata-envelope file should have gained exactly one version

  Scenario: A write that changes nothing stores no receipt
    When metadata-envelope caller sets "dcterms:title" to its current value under "op-3"
    Then metadata-envelope answer should be committed with no new version and no receipt

  # Error cases
  Scenario: A write without an envelope is refused
    When metadata-envelope caller sends a statement set with a plain body
    Then metadata-envelope answer should be refused as invalid_request
    And metadata-envelope refusal should be journaled once with its error code and opaque IDs only

  Scenario: A stale expected revision is a conflict
    When metadata-envelope caller sends a statement set expecting revision "v6"
    Then metadata-envelope answer should be HTTP 409 with a conflict outcome and nothing committed
