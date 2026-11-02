# API-S-019: Routes and expiry runs — expiry runs over every open library at the host's launch and
# then at the installation's expiry interval: a whole number of minutes from 5 to 1,440 (proposed
# bounds, owner to confirm), default 60, read by the settings GET as expiry_interval_minutes and
# changed by the admin only through the settings PUT; a changed interval takes effect for the next
# run without a restart. DE-S-014: the desktop host keeps the interval in its operational store and
# its expiry monitor takes a changed interval for the next run.
# Owner decision 2026-11-02, verbatim: "RE: DE-S-014 - "at launch and then every hour" is the
# default, this should be a configurable setting".
# Scenarios only; Task API-F2.30 adds their step definitions (unmatched steps are reported as
# skipped). The desktop harness fails on skipped steps, so the desktop monitor's scenarios go there
# with their step definitions in the same task.
Feature: Configurable expiry interval for Recently deleted
  As the admin of an installation (on the desktop, the owner)
  I want to choose how often FileSquid checks for expired deleted files
  So that expiry happens as promptly as I need without a fixed hourly schedule

  Background:
    Given expiry-interval installation with one open library holding a deleted file

  # Happy path
  Scenario: The interval is one hour by default
    When expiry-interval caller reads the Recently deleted settings
    Then expiry-interval answer should carry expiry_interval_minutes 60

  Scenario Outline: The admin sets an interval within its bounds
    When expiry-interval admin sets the expiry interval to <minutes> minutes
    Then expiry-interval settings should hold <minutes> minutes
    And expiry-interval change should be journaled once as settings.recently_deleted.set without its values

    Examples:
      | minutes |
      | 5       |
      | 60      |
      | 1440    |

  Scenario: A changed interval takes effect for the next run without a restart
    Given expiry-interval host ran expiry at launch with the default interval
    When expiry-interval admin sets the expiry interval to 15 minutes
    Then expiry-interval next run should fall due 15 minutes after the last run started
    And expiry-interval host should not need a restart

  # Edge cases
  Scenario: An interval that has already passed runs at once
    Given expiry-interval last run started 30 minutes ago with the default interval
    When expiry-interval admin sets the expiry interval to 10 minutes
    Then expiry-interval next run should start at once

  Scenario: Setting the current interval stores nothing
    When expiry-interval admin sets the expiry interval to 60 minutes
    Then expiry-interval store should be unchanged and nothing journaled

  # Error cases
  Scenario Outline: An interval outside the bounds is refused
    When expiry-interval admin sets the expiry interval to <value>
    Then expiry-interval answer should be refused as invalid_request
    And expiry-interval settings should still hold 60 minutes

    Examples:
      | value        |
      | 4 minutes    |
      | 1441 minutes |
      | 1.5 minutes  |

  Scenario: Only the admin changes the interval
    When expiry-interval caller who is not the admin sets the expiry interval to 30 minutes
    Then expiry-interval answer should be refused as forbidden
    And expiry-interval settings should still hold 60 minutes
