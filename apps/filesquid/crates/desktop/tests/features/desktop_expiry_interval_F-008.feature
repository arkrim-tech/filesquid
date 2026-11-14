# F-008: Deletion, hidden files and duplicate review
# SRS-F-008: Deletion, hidden files and duplicate review
# DE-S-014: Durable jobs and checkpoint-on-quit (the expiry monitor)
# API-S-019: Routes and expiry runs (the installation's expiry interval)
# Owner decision 2026-11-02, verbatim: "RE: DE-S-014 - "at launch and then every hour" is the
# default, this should be a configurable setting".
Feature: The desktop's expiry monitor follows the stored expiry interval
  As the owner of a FileSquid desktop
  I want the expiry of my deleted files to run as often as I chose
  So that a changed interval applies without relaunching the app

  Background:
    Given desktop-expiry host launched with its Recently deleted settings in the operational store

  # Happy path
  Scenario: Expiry runs at launch and then one stored interval after the last run started
    When desktop-expiry monitor ticks at launch
    Then desktop-expiry monitor should have completed 1 expiry job
    And desktop-expiry monitor should not run 59 minutes after the last run started
    And desktop-expiry monitor should run 60 minutes after the last run started
    And desktop-expiry monitor should have completed 2 expiry jobs

  # Happy path
  Scenario: A changed interval is taken for the next run without a relaunch
    Given desktop-expiry monitor ran at launch
    When desktop-expiry owner sets the expiry interval to 15 minutes
    Then desktop-expiry next run should fall due 15 minutes after the last run started
    And desktop-expiry monitor should not run 14 minutes after the last run started
    And desktop-expiry monitor should run 15 minutes after the last run started

  # Edge case
  Scenario: A new interval that has already passed runs at once
    Given desktop-expiry monitor ran at launch
    And desktop-expiry 30 minutes have passed
    When desktop-expiry owner sets the expiry interval to 10 minutes
    Then desktop-expiry monitor should run at its next tick

  # Edge case
  Scenario: The stored interval survives a relaunch
    Given desktop-expiry owner sets the expiry interval to 5 minutes
    When desktop-expiry host is relaunched
    Then desktop-expiry settings should answer expiry_interval_minutes 5
    And desktop-expiry monitor should run at its next tick
    And desktop-expiry monitor should run 5 minutes after the last run started

  # Error case
  Scenario Outline: An interval outside the bounds is refused and the monitor keeps its interval
    Given desktop-expiry monitor ran at launch
    When desktop-expiry owner sets the expiry interval to <minutes> minutes
    Then desktop-expiry answer should be refused as invalid_request
    And desktop-expiry next run should fall due 60 minutes after the last run started

    Examples:
      | minutes |
      | 4       |
      | 1441    |
      | 1.5     |
