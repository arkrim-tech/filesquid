# UI-S-032: Monitor and Logs composition (the Monitor button and route offered only to a caller the
# API grants the admin routes: the desktop owner or a server administrator)
# UI-S-026: the permission state (stateDenied)
# PRD F-061: only the desktop owner or server administrators may open the Monitor.
# Owner decision 2026-10-31, verbatim: "accept recommendations" (decision 3).
# Task UI-F2.21 writes the Playwright tests behind these scenarios against "vite build" plus "vite preview".
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
