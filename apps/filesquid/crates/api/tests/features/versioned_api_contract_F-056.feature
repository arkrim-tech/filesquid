# F-056: Single FileSquid API
# SRS-F-056: Single FileSquid API
# API-S-002: Versioned contract owned by filesquid-api with generated TypeScript types and a drift test
# API-S-009: Module and API boundaries return errors the UI can show; a failed request does not crash a host
Feature: Versioned API contract
  As a UI developer building on FileSquid
  I want one versioned contract with typed operations and typed errors
  So that every UI and distribution calls the same API and can show every failure

  Scenario: The API serves its own versioned contract
    Given a FileSquid API router
    When a client asks for the API contract
    Then the contract should be OpenAPI 3.1 at the crate's API version

  Scenario: Every routed operation is documented in the contract
    Given a FileSquid API router
    Then every path the router serves should be documented in the contract
    And every documented path should be served by the router or marked as a host route

  Scenario: Operation schemas carry idempotency, revision, page size and retryability
    Given the API contract
    Then a command envelope should require an operation id, an expected revision, a scope and a schema version
    And a query page size should be bounded between 1 and 1000
    And an API error should carry a code, a message and whether it is retryable

  Scenario: A failed request returns an error the UI can show
    Given a FileSquid server API that requires an administrator token
    When a client asks for the activity journal without a token
    Then the response should be an "unauthorized" error with a message
    And the error should not be retryable

  Scenario: A request before the host is ready is a retryable error
    Given a FileSquid API router whose monitor has not started
    When a client asks for the health report
    Then the response should be an "unavailable" error with a message
    And the error should be retryable

  Scenario: The host keeps answering after a failed request
    Given a FileSquid server API that requires an administrator token
    When an administrator asks for the activity journal with a malformed cursor
    Then the response should be an "invalid_request" error with a message
    When an administrator asks for the activity journal
    Then the response should be a journal page

  # Source
  # - resources/srs.md SRS-F-056: Single FileSquid API
  # - resources/prd.md F-056: Single FileSquid API
  # - apps/filesquid/crates/api/resources/srs.md API-S-002, API-S-009
  # - resources/architecture/030-filesquid-api-and-transports.md Decisions 2 and 6
