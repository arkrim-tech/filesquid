# UI-S-013: UI tests run against production artifacts (vite build plus vite preview) with strict DOM
# assertions: custom elements defined, labels from the active label table, computed token values in
# light and dark, no console errors. ADR 017: Allure and JUnit results stay in each app's
# tests/allure-results/, cleared before each run (test-cleanup rule). NFR-008: verified engineering.
Feature: Production-artifact UI test suite with Allure reporting
  As a maintainer of FileSquid and its distributions
  I want every UI proof to run against the built artifact and report to Allure
  So that a regression in the shipped UI fails a test and leaves a clear report

  Background:
    Given the UI package has been built with "vite build"

  # Happy path
  Scenario: The static build mounts with every custom element defined
    Given the build is served by "vite preview" on the workspace's preview port
    When the page loads
    Then the elements "fs-app-shell", "fs-theme-control", "fs-states", "fs-nav-tree", "fs-breadcrumb" and "fs-monitor" should be defined
    And the page should load only hashed script files from "/assets/"
    And the page should contain no inline script
    And the browser console should show no error

  Scenario: Every visible word comes from the active label table
    Given the build is served by "vite preview" on the workspace's preview port
    When the page loads
    Then the navigation entries should read as the label table's "libraries", "collections", "virtualCollections", "virtualExhibits" and "monitor" words
    And the API status line should use the label table's "apiStatus" word
    And the theme control legend should use the label table's "theme" word

  Scenario Outline: Computed token values are exact in each colour scheme
    Given the build is served by "vite preview" on the workspace's preview port
    And the browser prefers the <scheme> colour scheme
    When the page loads
    Then the computed "--fs-primary" colour should be "<primary>"
    And the computed "--fs-on-primary" colour should be "<on_primary>"
    And the page background should be "<surface>"

    Examples:
      | scheme | primary | on_primary | surface |
      | light  | #0b7a32 | #ffffff    | #ffffff |
      | dark   | #16ac49 | #0d2b18    | #141414 |

  Scenario Outline: A distribution runs the same suite shape with its own tokens
    Given the "<distribution>" build is served by "vite preview" on port <port>
    And the browser prefers the <scheme> colour scheme
    When the page loads
    Then the computed "--fs-primary" colour should be "<primary>"
    And the computed "--fs-on-primary" colour should be "<on_primary>"

    Examples:
      | distribution | port | scheme | primary | on_primary |
      | ImageSquid   | 4174 | light  | #005db6 | #ffffff    |
      | ImageSquid   | 4174 | dark   | #a9c7ff | #003063    |

  # Edge cases
  Scenario: The server-backed and static-preview suites never share a port at the same time
    Given the server-backed suite and the static-preview suite use the same workspace port
    When "npm run test:e2e" runs
    Then the server-backed suite should finish before the static-preview suite starts

  Scenario: Preview specs stay out of the server-backed run
    Given the server-backed Playwright configuration
    Then it should ignore the specs under "tests/e2e/preview/"

  # Error cases
  Scenario: A refused API call is shown as an alert and is the only tolerated console line
    Given the build is served by "vite preview" on the workspace's preview port
    And the API answers the health call with status 503
    When the page loads
    Then the status line should be an alert that starts with the label table's "apiUnavailable" word
    And the browser console should show no error other than the browser's own network line for status 503

  Scenario: A label key that is not in the table fails the test
    Given a test asks for the label "noSuchLabel"
    When the label is read from the active label table
    Then the test should fail with a message naming the missing key

  # Reporting
  Scenario: Results are cleared before each run
    Given results from an earlier run exist under "tests/allure-results/"
    When "npm test" starts
    Then "tests/allure-results/" should be removed first
    When "npm run test:e2e" starts
    Then only the Playwright results under "tests/allure-results/" should be removed first

  Scenario: Every runner leaves Allure raw results and JUnit beside them
    When the unit, UI and end-to-end suites have run
    Then "tests/allure-results/vitest/" and "tests/allure-results/playwright/" should hold Allure raw results
    And "tests/allure-results/" should hold the Vitest and Playwright JUnit files

  Scenario: An Allure report is generated from the results
    Given the suites have run
    When "npm run report" runs
    Then "tests/allure-report/index.html" should exist
    And "tests/allure-report/" should not be tracked by version control
