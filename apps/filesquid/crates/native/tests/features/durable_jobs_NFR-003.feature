# SRS-NFR-003: Bounded processing and migration endurance (DE-S-014, API-S-033; ADR 007)
Feature: Durable jobs in the host's operational store
  As a library owner
  I want long work to survive quitting the app or a crash
  So that a restart resumes where it stopped without redoing completed work

  Background:
    Given a host data directory with the SQLite job store

  # Happy path
  Scenario: A job suspended at quit resumes from its checkpoint after a restart
    Given a running job that checkpointed "lib-a" as done
    When the app quits
    And the host starts again
    Then the job should be resumable with the checkpoint "lib-a"

  # Edge case
  Scenario: A crashed run's lease is released once it expires
    Given a running job whose host crashed
    When the lease expires and the host recovers its jobs
    Then the job should be pending again
    And the journal should record the job as recovered

  # Error case
  Scenario: An expired worker cannot write after another worker took the job over
    Given a running job whose lease expired
    When another worker takes the job over
    Then a checkpoint from the expired worker should be refused as "fenced"

  # Error case
  Scenario: A job whose source changed is not resumed
    Given a job suspended at quit for the source "installation:a"
    When the host starts it for the source "installation:b"
    Then the job should fail with "source_changed"
