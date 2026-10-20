# SRS-NFR-002: Benchmark envelope and responsiveness; SRS-NFR-003: Bounded processing (DE-S-016; ADR 006, ADR 007)
Feature: Bounded, out-of-process media workers
  As a library owner
  I want thumbnails made by bounded libvips workers in their own processes
  So that browsing stays responsive and a bad file never takes down the app

  # Happy path
  Scenario: The libvips worker makes a thumbnail within the requested bound
    Given a still of 64 by 48 pixels
    When the host asks for a thumbnail of at most 16 pixels
    Then the thumbnail should be 16 by 12 pixels

  # Error case
  Scenario: ImageMagick is never a fallback
    Given a still that only ImageMagick could decode
    When the host asks for a thumbnail of at most 16 pixels
    Then the request should be refused as "unsupported_format"

  # Edge case
  Scenario: At most two workers run at once
    Given workers that each take a moment
    When six thumbnails are asked for at once
    Then no more than 2 workers should have run at the same time

  # Error case
  Scenario: A crashed worker fails only its own run
    Given a worker that crashes
    When the host asks for a thumbnail of at most 16 pixels
    Then the request should be refused as "worker_crashed"
    And the next worker run should succeed
