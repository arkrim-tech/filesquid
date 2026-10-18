# F-061: Activity journal and live health monitoring
# SRS-F-061: Activity journal and live health monitoring (Journal store, Health)
Feature: Native journal store and data-directory checks
  As an owner or administrator
  I want the host's activity journal kept in its own operational store with no log files
  So that the journal is structured product data and the host's health reflects its storage

  # Happy path (moved from the API's "No event is lost under load" scenario with the store)
  Scenario: The SQLite journal store writes no log file
    Given a host data directory with the SQLite journal store
    When an import is journaled
    Then no log file should exist anywhere in the host's data directory

  # Happy path
  Scenario: A host monitor opens its store in the data directory
    Given an empty host data directory
    When the host monitor opens and stops
    Then the data directory should hold the SQLite operational store
    And the journal should record the host starting and stopping

  # Error case
  Scenario: A data directory that cannot be written fails health
    Given a host data directory that does not exist
    When the data-directory checks run in the host's health report
    Then the data directory should be reported as not writable
    And health should be failing with the reason "data_dir_not_writable"
