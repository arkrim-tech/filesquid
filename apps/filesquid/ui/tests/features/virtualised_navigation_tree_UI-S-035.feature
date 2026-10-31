# UI-S-035: Production-build browsing performance (the navigation tree holds only its window plus overscan in the DOM)
# UI-S-017: Navigation tree sections, its cap of 10,000 nodes a level, and pins kept by handle
# UI-S-016: Accessibility contracts (tree roles, aria-level, aria-setsize, aria-posinset, keyboard)
# NFR-002 and RC-002 (PRD): warm browse result p95 <= 100 ms; F-004 (PRD): collections, folders and navigation
# Owner decision 2026-10-30, verbatim: "Okay implement all these recommendations" (virtualise the tree; keep the cap for the MVP).
# Task UI-F3.4 writes the Playwright and unit tests behind these scenarios against "vite build" plus "vite preview".
Feature: Virtualised navigation tree
  As a library owner with tens of thousands of collections
  I want the navigation tree to stay quick however many nodes a level holds
  So that opening a folder and going back feel as quick as in a small library

  Background:
    Given the UI package has been built with "vite build" and is served by "vite preview"
    And the browser is Chromium at a 1440 x 900 viewport with a device scale factor of 2
    And the Large catalog's navigation lists 50,000 collections, answered as the first 10,000 of the level

  # Happy path
  Scenario: The tree draws only its window
    When the tree shows the level of 10,000 collections
    Then the tree items in the DOM should be at most the visible items plus two overscans of 5
    And the tree's scrolling area should be no taller than the sidebar

  Scenario: Opening a folder and going back meet the result gate on the Large catalog
    Given 20 warm-up interactions were thrown away
    When I open an unseen folder and go back 120 times with real input
    Then the 95th percentile of the time to the correct rows should be at most 100 milliseconds
    And the 99th percentile should be at most 200 milliseconds
    And no record of the class should be declared a known gap

  Scenario: The gate holds while background work streams in
    Given the event channel pushes 5 navigation changes per second
    When I open an unseen folder and go back 120 times with real input
    Then the result and feedback gates should hold with no known gap declared

  # Edge cases
  Scenario: Tree semantics count the whole level, not the drawn items
    When the tree shows the level of 10,000 collections scrolled to its middle
    Then every drawn item should have the tree item role and its aria-level
    And its aria-setsize should be 10000 and its aria-posinset its place in the whole level

  Scenario: A level stops at its cap and says so
    Given a level that answers more than 50 pages of 200 nodes
    When the tree reads the level
    Then it should show the first 10,000 nodes
    And the console should say that only the first 50 pages of the level are shown
    And a short line under the last node shown should say "Showing the first 10,000 only. Use search to find the rest." from the label key navigationLevelLimit (owner decision 2026-10-31, verbatim: "accept recommendations")

  Scenario: A pin beyond the cap still opens
    Given collection "Harbour 48000" is pinned and is not among the first 10,000 nodes of its level
    When I choose the pinned destination
    Then the destination should open by its handle

  # User interactions
  Scenario: The keyboard reaches nodes that are not drawn
    Given the first collection of the level has focus
    When I press End
    Then the last of the 10,000 collections should be drawn and have focus
    And exactly one tree item of the Collections section should be in the tab order, as each section's tree keeps one tab stop

  Scenario: Collapse state and pins survive a reload
    Given 1,000 nodes are collapsed and 3 destinations are pinned
    When the page reloads
    Then the same nodes should be collapsed and the same destinations pinned
    And the tree should still hold only its window plus overscan in the DOM

  # Error cases
  Scenario: A pinned destination that is gone is still shown as unavailable
    Given a pinned collection that the API answers "not_found"
    When the tree loads
    Then the pin should be shown as unavailable with a remove action
