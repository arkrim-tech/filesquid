# F-056: Single FileSquid API
# SRS-F-056: Single FileSquid API (every API call authorised before files, SQL or keys are touched; deny-by-default)
# F-057: Desktop and server hosts
# API-S-004: Every call is authenticated and authorised before files, SQL or keys are touched: per-launch token on desktop; accounts, groups and collection-level permissions on the server
# API-S-009: Module and API boundaries return errors the UI can show
# API-S-015: The liveness probe is the only public route and says only that the process is up
# API-S-017: Every authorisation refusal and every credential issue is journaled
Feature: Deny-by-default authorisation
  As a library owner
  I want every FileSquid API call refused unless it is explicitly allowed
  So that nothing reaches my files, database or keys without my app's permission

  Background:
    Given a desktop FileSquid API with a per-launch token

  # Happy path
  Scenario: The owner's own webview reaches every protected route with the per-launch token
    When the owner's webview calls every protected API route with the per-launch token
    Then every call should be answered

  # Error cases
  Scenario: Every route refuses a call without the per-launch token
    When a client calls every protected API route without a token
    Then every call should be refused as "unauthorized" because of "missing_credentials"
    And no refusal should be retryable

  Scenario: Every route refuses a call with a wrong token
    When a client calls every protected API route with a token from another launch
    Then every call should be refused as "forbidden" because of "invalid_credentials"
    And no refusal should be retryable

  Scenario: The health route and the event channel are not public
    When a client asks for the health status without a token
    And a client opens the event channel without a token
    Then every call should be refused as "unauthorized" because of "missing_credentials"

  # Edge cases
  Scenario: A liveness probe needs no token and learns only that the process is up
    When a monitoring probe asks whether the process is up without a token
    Then the probe should be told only that the process is up

  Scenario: An unknown path does not reveal the route table to an anonymous caller
    When a client asks for an API path that does not exist without a token
    Then every call should be refused as "unauthorized" because of "missing_credentials"

  Scenario: A host that installed no authoriser refuses every call
    Given a FileSquid API router with no authoriser installed
    When the owner's webview calls every protected API route with the per-launch token
    Then every call should be refused as "forbidden" because of "invalid_credentials"

  Scenario: A browser opens the event channel with the token as a WebSocket subprotocol
    When the owner's webview opens the event channel offering the token as a subprotocol
    Then every call should be answered

  Scenario: A signed-in principal the policy does not permit is refused
    Given a desktop FileSquid API whose policy does not permit reading the monitor
    When the owner's webview asks for the monitor health with the per-launch token
    Then every call should be refused as "forbidden" because of "not_permitted"

  # Journal
  Scenario: Every refusal is journaled without the presented credential
    When a client calls every protected API route with a token from another launch
    Then the activity journal should record one "api.auth.denied" entry per refused call
    And no journal entry should contain the presented token

  # Source
  # - resources/srs.md SRS-F-056: Single FileSquid API
  # - resources/prd.md F-056: Single FileSquid API, F-057: Desktop and server hosts
  # - apps/filesquid/crates/api/resources/srs.md API-S-004, API-S-009, API-S-015, API-S-017
  # - resources/architecture/030-filesquid-api-and-transports.md Decision 5
