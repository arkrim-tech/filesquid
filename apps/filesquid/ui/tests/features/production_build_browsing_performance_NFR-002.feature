# NFR-002 and RC-002 (PRD), U-11 (product vision), NFR-003 (the clauses that bind browsing) and UI-S-035 (the measurement procedure
# and the profile limits Task UI-F3.1 defines): the production UI build, served by "vite preview" and measured in this machine's
# Chromium, meets the interactive budgets with the real shell and real components, as assertions. Playwright is the runner
# (tests/performance/*.perf.spec.ts, "npm run test:perf"). The numbers are the UI's own contribution over a fake host with no
# modelled latency; the packaged macOS desktop is measured by Task DE-F3.2 and "filesquid-server" by Task SV-F3.2. Composition:
# UI-S-024 (constants), UI-S-025 (shell), UI-S-026 (list), UI-S-031 (preview and file panel), mockups 05 and 22 of the v3 suite.
Feature: Production-build browsing performance
  As a library owner with a very large collection
  I want browsing and previewing to feel as quick as in a small one
  So that the budgets of FileSquid U-11 are measured on the shipped UI and a regression fails a test

  Background:
    Given the UI package has been built with "vite build"
    And the build is served by "vite preview" on the workspace's preview port
    And the browser is Chromium at a 1440 x 900 viewport with a device scale factor of 2
    And the host is faked in process with no modelled latency, the seeded catalogue answering in the contract's own shape

  # Happy path
  Scenario Outline: The first useful folder view is ready within 3 seconds on a cold load
    Given a "<profile>" catalog whose navigation lists <albums> collections
    And the folder under test holds the largest view the API answers
    When the folder is opened from the address in a new browser context 20 times
    Then each load should show the viewport's rows with the host's first row, none of them a placeholder
    And the 95th percentile and the longest of the 20 loads should be at most 3000 milliseconds
    And the load should be reported by its parts: response, script, first view request and ready

    Examples:
      | profile | albums |
      | small   | 100    |
      | medium  | 5000   |
      | large   | 50000  |

  Scenario Outline: A warm interaction gives visible feedback within 100 milliseconds
    Given the Large catalog is open on its biggest folder and 20 warm-up interactions were thrown away
    When I <interaction> 120 times with real input
    Then the 95th percentile of the time to visible feedback should be at most 100 milliseconds
    And the 99th percentile should be at most 200 milliseconds
    And the sample size, warm-up and percentiles should be reported

    Examples:
      | interaction                                  |
      | tick and untick a row                        |
      | move the active row with the arrow keys      |
      | sort by Size, Modified and Name in turn      |
      | switch between the grid and the list         |
      | open an unseen folder and go back            |
      | narrow the view with the sidebar filter      |

  # KNOWN GAP (owner decision, Task UI-F3.1 Walkthrough, Finding 4): on the Large catalog the "open an unseen folder and go back"
  # row misses the result gate (p95 about 120 to 135 ms against 100 ms) because the navigation tree draws every node of a level,
  # up to its cap of 10,000 nodes. The Playwright test is marked "expected to fail" so the budget stays an assertion and the
  # suite turns red the day it passes, as the cue to remove the mark. The Small catalog meets the gate (p95 about 70 ms).

  Scenario: A prepared image paints the correct image within 100 milliseconds
    Given a folder of 60 different 1280 x 720 images is open in the viewer with the file panel
    When I step through the folder with Next and Previous 120 times after 20 warm-up steps
    Then each sample should end when the image named by the new file is loaded and decoded
    And the 95th percentile should be at most 100 milliseconds
    And every image of the folder should have been previewed, and the coverage reported

  Scenario: Search reports query feedback and completed results separately
    Given the Large catalog is open and 20 warm-up queries were thrown away
    When I type 120 varied queries into the header search bar and press Enter
    Then the query feedback should be visible within 100 milliseconds at the 95th percentile
    And the host's top hit should show in the drop-down within 2000 milliseconds at the 95th percentile
    And the first page of results in the host's order should be painted within 2000 milliseconds at the 95th percentile

  Scenario Outline: Scrolling holds 60 Hz in both layouts
    Given the Large catalog's biggest folder is open in the "<layout>" layout
    When it is scrolled at 6000 pixels per second for four runs of 8 seconds, two of them reversing and one starting with a jump
    Then at least 1500 frames should be measured with the first 5 of each run discarded
    And at most 5 percent of the frames should take longer than 25.05 milliseconds
    And the 99th percentile frame should be at most 50 milliseconds
    And frames longer than 100 milliseconds should be at most 0.1 percent of the frames

    Examples:
      | layout |
      | list   |
      | grid   |

  # Edge cases
  Scenario: A twelve-megapixel original has its own limit
    Given a folder of six 4000 x 3000 JPEG originals is open in the viewer
    When I step through it 120 times after 20 warm-up steps
    Then the 95th percentile of the time to the decoded image should be at most 2000 milliseconds
    And a cold first preview in a new context should be at most 3000 milliseconds
    And the first useful and the final image should be reported as one, because no prepared derivative exists

  Scenario: A text preview reads one ranged request of at most 64 KiB
    Given a folder of four 150 KiB text files is open in the viewer
    When I step through it 120 times after 20 warm-up steps
    Then every read should send "Range: bytes=0-65535"
    And the 95th percentile of the time to the text painted should be at most 100 milliseconds

  Scenario: The DOM holds a bounded window however many rows are loaded
    Given the biggest view the API answers is loaded in full, 11,000 rows over 55 pages
    Then the rows in the DOM should be at most the visible rows plus two overscans of 5
    And the scrolling area should be no taller than the window
    And every folder and file should be counted once in the footer
    And the Large catalog should say that only the first items are shown

  Scenario: The heap does not grow after warm-up
    Given the Medium catalog is open and the interactions were warmed up
    When 600 mixed interactions run
    Then the JS heap after a forced collection should have grown by at most 10 percent
    And the number of DOM elements before and after should be reported

  # Error cases
  Scenario: Content that cannot be read shows the error state, never a spinner
    Given every content request answers 410
    When I open the next file 20 times
    Then the error alert should show within 2000 milliseconds at the 95th percentile

  Scenario: A busy machine is reported, not judged
    Given an idle page is measured for 10 seconds before the app is
    When more than 1 percent of its frames take longer than 25.05 milliseconds
    Then the run should fail naming the environment as unusable
    And the route round trip of the harness should be reported beside every result

  # Load and pressure
  Scenario: The gates hold while background work streams in
    Given the event channel pushes 5 navigation changes per second
    When I tick rows and open folders 120 times each
    Then the same feedback and result gates should hold

  Scenario: Resource pressure is reported
    Given the CPU is slowed 4 times
    When I tick rows and sort 100 times each
    Then the percentiles should be reported with their sample size and not asserted

  # Composition (UI-S-024, UI-S-025, UI-S-026, UI-S-031, UI-S-013)
  Scenario Outline: The measured page is the v3 composition
    Given the browser prefers the <scheme> colour scheme
    And the Large catalog is loaded
    Then the sidebar should be 240 pixels wide, the header 60 pixels and the footer 40 pixels, each within 2 pixels
    And the list header should be 38 pixels tall and the rows 44 pixels tall
    And the page title should be Georgia 30 pixels at weight 700 in the heading colour
    And every word should come from the active label table
    And the custom elements should be defined and the console should show no error

    Examples:
      | scheme |
      | light  |
      | dark   |
