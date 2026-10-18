# SRS-F-055: Default UI and panel composition. The default UI shows distinct empty, loading,
# error, offline and permission-denied states and a breadcrumb, worded from the label table
# (UI-S-001) and meeting the accessibility contracts (UI-S-016). Task UI-F2.1a.
Feature: View states and breadcrumb
  As a library owner
  I want to know whether a view is loading, empty, failing, offline or not mine to see, and where I am
  So that I never mistake a missing answer for an empty library

  Background:
    Given the built UI is served by a FileSquid host

  # Happy path
  Scenario: A view that is loading says so politely
    When a view is waiting for its data
    Then a status message "Loading…" should be announced
    And no alert should be shown

  Scenario: The breadcrumb shows where I am
    Given I am in folder "2025" of collection "Holidays" in library "Family" of an unnamed repository
    Then the "Breadcrumb" navigation should list exactly "Repository", "Family", "Holidays", "2025" in that order
    And "2025" should be marked as the current page

  # Edge cases
  Scenario: An empty view is not an error
    When a view has loaded and holds nothing
    Then a status message "Nothing here yet" should be shown
    And no alert should be shown

  Scenario: Without a trail the breadcrumb is hidden
    Given no destination is current
    Then no breadcrumb should be shown

  # Error cases
  Scenario: A failed load offers a retry
    When a view fails to load while online
    Then an alert "Something went wrong" should be announced with a "Try again" button
    When I press "Try again"
    Then the view should ask for its data again

  Scenario: Offline is told apart from a failure
    Given the browser is offline
    When a view fails to load
    Then an alert "You are offline" should be announced with a "Try again" button

  Scenario: A permission refusal names its reason
    When a view is refused with the reason not permitted
    Then an alert "You do not have permission to see this" should be announced with "Not permitted"
    And no "Try again" button should be offered

  # User interaction
  Scenario: Retrying from the keyboard keeps my place
    Given focus is on "Try again"
    When I press it and the view starts loading again
    Then focus should move to the loading message, not be lost

  Scenario: The states use the distribution's words and colours
    Given the ImageSquid distribution in the dark theme
    When a view is offline
    Then the alert should read "You are offline" in the ImageSquid error colour

  # Source
  # - resources/prd.md F-055
  # - resources/srs.md SRS-F-055
  # - apps/filesquid/ui/resources/srs.md UI-S-001, UI-S-016
