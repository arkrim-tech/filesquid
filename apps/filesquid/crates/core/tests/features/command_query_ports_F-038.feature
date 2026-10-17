# SRS-F-038: Open interfaces and product exit
# NFR-001: Preservation and transactional integrity
Feature: Versioned command and query ports
  As a host crate
  I want versioned command and query envelopes
  So that desktop IPC and hosted APIs share one portable contract

  Scenario: A command envelope carries version, operation id, expected revision, and scope
    Given an operation id "op-1"
    When I construct a command envelope with expected revision "rev-3" and scope "library:lib-1"
    Then the command schema version should be 1
    And the command operation id should be "op-1"
    And the expected revision should be "rev-3"
    And the command scope should be "library:lib-1"

  Scenario: A query envelope requires a bounded page size
    When I construct a query envelope with page size 50 and scope "library:lib-1"
    Then the query page size should be 50
    And the query schema version should be 1

  Scenario: Zero page size is rejected
    When I construct a query envelope with page size 0 and scope "library:lib-1"
    Then page size construction should fail

  Scenario: The portable maximum page size of 1000 is accepted
    When I construct a query envelope with page size 1000 and scope "library:lib-1"
    Then the query page size should be 1000

  Scenario: Page size above the portable maximum is rejected
    When I construct a query envelope with page size 1001 and scope "library:lib-1"
    Then page size construction should fail

  Scenario: Empty operation id is rejected
    Given an operation id ""
    When I construct a command envelope with expected revision "rev-1" and scope "library:lib-1"
    Then operation id construction should fail

  Scenario: A committed outcome is durable completion evidence
    When I record a committed command outcome for operation "op-1"
    Then the command outcome should be "committed"
    And the outcome should be durable completion evidence

  Scenario: Pending, error, and conflict outcomes are explicit
    When I record a pending command outcome for operation "op-1"
    Then the command outcome should be "pending"
    When I record a conflict command outcome for operation "op-1"
    Then the command outcome should be "conflict"
    When I record an error command outcome for operation "op-1" with retryable code "busy"
    Then the command outcome should be "error"
    And the outcome should not be durable completion evidence
    And the port error code should be "busy"
    And the port error should be retryable

  Scenario: UI progress is not durable completion evidence
    When I record UI progress for operation "op-1"
    Then UI progress should not be durable completion evidence

  # Source
  # - resources/srs.md SRS-F-038: Open interfaces and product exit
  # - resources/prd.md F-038: Open interfaces and product exit
  # - resources/prd.md NFR-001: Preservation and transactional integrity
  # - resources/architecture.md 3.3 Backend Architecture
