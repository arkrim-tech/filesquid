# UI-S-031: Preview and file panel composition (the preview's top bar and zoom chip of mockup 22; the filmstrip is deferred)
# UI-S-024: Visual constants of the active mockup suite; UI-S-019: display names; UI-S-005: label tables
# UI-S-035: a prepared viewer image paints at p95 <= 100 ms
# F-003 and F-055 (PRD): browsing, selection and viewing; default UI and panel composition
# Owner decision 2026-10-30, verbatim: "Okay implement all these recommendations" (build the top bar and zoom chip in the MVP,
# defer the filmstrip, keep the media frame on the 8 px --fs-radius token).
# Task UI-F2.19 writes the Playwright tests behind these scenarios against "vite build" plus "vite preview".
Feature: Preview top bar and zoom chip
  As a library owner looking through a folder of images
  I want the preview to say where I am and how the image is scaled
  So that I can step through a folder without losing my place

  Background:
    Given the UI package has been built with "vite build" and is served by "vite preview"
    And the browser is Chromium at a 1440 x 900 viewport
    And collection "Harbour project" holds 11 files and "facade-study.jpg" is the third

  # Happy path
  Scenario Outline: The top bar names the collection, the file and its place
    Given the browser prefers the <scheme> colour scheme
    When I open "facade-study.jpg" in the preview
    Then the top bar should show "Harbour project", the JPG badge, "facade-study.jpg" and "3 of 11"
    And the display name should be 14 pixels at weight 600 in the heading colour
    And the collection should be 13 pixels in the secondary colour and the position 13 pixels in the muted colour
    And each string should come from the active label table and the console should show no error

    Examples:
      | scheme |
      | light  |
      | dark   |

  Scenario: The zoom chip states the fitted scale
    When I open "facade-study.jpg" in the preview
    Then the zoom chip under the media should read "Fit · {percent}%"
    And the percent should be the shown width over the image's natural width, rounded to a whole percent
    And the media should keep the 8 pixel radius of the --fs-radius token

  Scenario: The full-size image reports that it is loading
    Given the full-size image of "facade-study.jpg" has not finished loading
    When I open "facade-study.jpg" in the preview
    Then the zoom chip should show "Loading full size" with its progress bar
    And the text should go away when the full-size image has loaded

  # User interactions
  Scenario: Next and previous move through the collection
    When I open "facade-study.jpg" in the preview
    And I choose Next
    Then the top bar should show the fourth file and "4 of 11"
    When I choose Previous
    Then the top bar should show "facade-study.jpg" and "3 of 11"

  # Edge cases
  Scenario: A location without a collection shows no collection name
    Given the browser shows a location that names no collection
    When I open a file in the preview
    Then the top bar should show no collection name and never a guessed one

  Scenario: No filmstrip is drawn
    When I open "facade-study.jpg" in the preview
    Then no filmstrip of the folder's files should be shown under the media

  # Performance
  Scenario: The top bar and chip keep the viewer gate
    Given a folder of 60 different 1280 x 720 images is open in the viewer
    When I step through it with Next and Previous 120 times after 20 warm-up steps
    Then the 95th percentile time to the correct decoded image should be at most 100 milliseconds

  # Error cases
  Scenario: Content that cannot be read keeps the bar and shows the error state
    Given every content request answers 410
    When I open "facade-study.jpg" in the preview
    Then the top bar should still name the file and its place
    And the error alert should show within 2000 milliseconds
