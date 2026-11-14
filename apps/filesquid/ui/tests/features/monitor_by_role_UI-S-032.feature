# UI-S-032: Monitor and Logs composition (the Monitor button and route offered only to a caller the
# API grants the admin routes: the desktop owner or a server administrator)
# UI-S-026: the permission state (stateDenied)
# PRD F-061: only the desktop owner or server administrators may open the Monitor.
# Owner decision 2026-10-31, verbatim: "accept recommendations" (decision 3).
# Task UI-F2.21 proves these scenarios with tests/e2e/preview/monitor-by-role.spec.ts (Playwright against
# "vite build" plus "vite preview"), tests/ui/monitor-by-role.test.ts and tests/unit/admin-access.test.ts.
Feature: Monitor link and route by role
  As a user who is not an administrator
  I want the app not to offer me a dashboard I may not open
  So that the header shows only what I can use

  # Happy path
  Scenario: The desktop owner sees the Monitor
    Given the API grants the caller the admin routes
    When the shell loads
    Then the header should show the Monitor button
    And opening it should show the Monitor dashboard

  # Edge case
  Scenario: A direct visit by a caller who is not an administrator shows the permission state
    Given the API answers the admin health route with 403
    When I open "#/monitor" directly
    Then the page should show "You do not have permission to see this" from stateDenied
    And no further admin route should be called

  # Error case
  Scenario: A caller the admin routes refuse sees no Monitor button
    Given the API answers the admin health route with 403
    When the shell loads
    Then the header should show no Monitor button

  # Edge case
  Scenario: The Logs row follows the same role check as the Monitor button
    Given the API answers the admin health route with 403
    When the shell loads on the desktop host
    Then the sidebar should show no Logs row

  # Edge case
  Scenario: A caller with no credential yet keeps the Monitor sign-in
    Given the page holds no credential
    When I open "#/monitor" directly
    Then the header should show the Monitor button
    And the Monitor should ask for the administrator token

  # User interaction
  Scenario: A role refusal met by the Monitor's own read hides the button
    Given the page holds a credential the admin routes refuse
    When the Monitor reads the admin routes
    Then the page should show "You do not have permission to see this" from stateDenied
    And the header should show no Monitor button
