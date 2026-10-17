# F-056: Single FileSquid API
# SRS-F-056: Single FileSquid API
# API-S-003: Transport-agnostic router mounted unchanged by every host, with one event channel for job progress and change notifications; clients do not depend on receiving every event
Feature: Transport-agnostic routes and one event channel
  As a UI developer building on FileSquid
  I want the same routes on every host and one bounded event channel of hints
  So that my UI works on the desktop and the server and stays correct when it misses events

  # Happy path
  Scenario: The same router answers through the desktop scheme and over HTTP
    Given a FileSquid API router
    When a client asks for the health report through each host's address form
    Then every address form should get the same answer

  Scenario: A subscriber receives a published process update
    Given an event channel with one subscriber
    When a process update is published
    Then the subscriber should receive a "process.update" event

  # Edge cases
  Scenario: A subscriber exactly one buffer behind misses nothing
    Given an event channel with one subscriber that is not reading
    When as many events as the buffer holds are published
    Then the subscriber should receive every event in order

  Scenario: A subscriber that falls further behind is told to reread
    Given an event channel with one subscriber that is not reading
    When one more event than the buffer holds is published
    Then the subscriber should first receive an "events.resync" hint for 1 missed event
    And then the newest events the buffer holds

  Scenario: An event larger than the size cap is replaced by a hint
    Given an event channel with one subscriber
    When an event one byte over the size cap is published
    Then the subscriber should first receive an "events.resync" hint for 1 missed event

  # Error cases
  Scenario: The channel refuses a subscriber over the cap with a retryable error
    Given an event channel with as many subscribers as the cap allows
    When one more client subscribes
    Then the subscription should be refused with a retryable "unavailable" error
    When one subscriber disconnects
    Then a new client should be able to subscribe

  # Source
  # - resources/srs.md SRS-F-056: Single FileSquid API
  # - resources/prd.md F-056: Single FileSquid API
  # - apps/filesquid/crates/api/resources/srs.md API-S-003
  # - resources/architecture/030-filesquid-api-and-transports.md Decisions 3 and 4, event channel limits
