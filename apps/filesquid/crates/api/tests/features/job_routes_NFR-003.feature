# NFR-003: Bounded processing and migration endurance
# SRS-NFR-003: Bounded processing and migration endurance
# API-S-036: Job routes and job events (ADR 007; CORE-S-006)
# Bound in tests/support/job_steps.rs (Task API-F1.6).
Feature: Durable jobs are reported and controlled through the job routes
  As a library owner
  I want to pause, resume and cancel long work and see its progress
  So that an interrupted import or scan picks up where it stopped

  Background:
    Given a host with a job service and a counting worker

  # Happy path
  Scenario: An interrupted job resumes without repeating completed work
    Given a counting job of 6 units that has done 3
    When I pause the job through the job routes
    And the worker reaches its next checkpoint
    Then the job should be paused with 4 units done
    When I resume the job through the job routes
    Then the job should complete
    And every unit should have been done exactly once

  # Edge case
  Scenario: A paused job that waits does not start until it is resumed
    Given a counting job of 6 units that waits
    When I pause the job through the job routes
    Then the worker should not be able to start it
    When I resume the job through the job routes
    Then the job should complete

  # Edge case
  Scenario: A classified kind is held to its cap
    Given transfers are capped at 1
    And two waiting transfer jobs
    When the first transfer starts
    Then the second transfer should be refused as "slots_full"
    And the second transfer should still wait

  # Edge case
  Scenario: Every stored change of a job is announced on the event channel
    Given a counting job of 6 units that waits
    When I pause the job through the job routes
    Then its "job.update" events should show it waiting, then paused
    And no event should carry the job's source

  # Error case
  Scenario: A finished job refuses every control
    Given a counting job of 6 units that waits
    When I cancel the job through the job routes
    Then the job should be cancelled
    And pausing, resuming and cancelling it again should each be refused as a conflict
