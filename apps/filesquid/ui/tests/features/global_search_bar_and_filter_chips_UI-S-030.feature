# F-063: File search query language and filter chips
# SRS-F-063: File search query language and filter chips
# UI-S-007: The global search bar offers chips, typed suggestions, results and unsupported-operator states
# UI-S-030: Search composition (mockups 20, 21 and 53)
# UI-S-024: Visual constants of the active mockup suite
# API-S-012: Search operation and operator grammar
Feature: Global search bar and filter chips in the FileSquid default UI
  As a user with a library of files
  I want one search bar with chips, suggestions and a full results list
  So that I can narrow a search the way I already know and be told plainly when an operator cannot be used

  Background:
    Given I am using the FileSquid default UI on the desktop host with one library I may read
    And the library holds collections and files whose names contain "harbour"

  # Field and chips
  Scenario: The search field widens and shows the drop-down while it has focus
    When I focus the search field
    Then the field should be 620 px wide with the focus border and the drop-down should open under it
    And the drop-down should offer the Type, Location and Modified filters, the operators this device answers and the line "Enter shows all results"
    And the drop-down should not offer a People filter, because the desktop has no accounts

  Scenario: Chips and text layer in one field
    When I apply the Type chip "Document" and the Modified chip "last 30 days" and type "harbour"
    Then both chips should appear inside the field before the text, each with its own remove control
    And Clear all should appear at the right of the field
    And the query asked should be the chips' terms followed by "harbour"

  Scenario: A chip is removed on its own
    Given the chips "Type: Document" and "Modified: last 30 days" are applied
    When I remove "Type: Document"
    Then only "Modified: last 30 days" should remain and the live suggestions should be asked again

  Scenario: Clear all empties the field without searching
    Given a chip and the text "harbour" are in the field
    When I choose Clear all
    Then the field should be empty and no search should have run

  Scenario: Backspace on empty text removes the last chip
    Given a chip is applied and the text is empty
    When I press Backspace
    Then the last chip should be removed

  Scenario Outline: Each filter chip offers typed choices
    When I choose the "<filter>" filter in the drop-down
    Then the suggestions should list the choices of "<filter>" as operator, value and hint
    And choosing one should apply it as a chip, replacing any chip of the same filter

    Examples:
      | filter   |
      | Type     |
      | Location |
      | Modified |

  Scenario: Modified chips count back from the caller's own calendar and zone
    When I apply the Modified chip "last 30 days"
    Then the search should carry "after:" the date 30 days back in my zone
    And the request should name my zone, so no instant is invented

  # Suggestions and live files
  Scenario: Typed text brings typed suggestions
    Given the collection "Harbour project" is shown
    When I type "harbour"
    Then the suggestions should offer "in:" the collection, "type:" a class and "title:" the text, each with its hint

  Scenario: The drop-down lists the first matching files as I pause
    When I type "harbour" and pause
    Then the drop-down should list at most three matching files with their badge, display name and when they changed
    And a newer query should replace an older answer that arrives late

  Scenario: A refusal while typing shows no files and no error
    When I type a query holding an operator the device cannot answer
    Then the drop-down should list no files
    And the results page, not the drop-down, should explain the refusal when I press Enter

  Scenario: Choosing a file opens it
    Given the drop-down lists "harbour-masterplan-v7.pdf"
    When I choose it
    Then the file should open as it does from the Drive list and no search should run

  # Results
  Scenario: Enter shows the full list with sortable columns
    Given the chips and text are in the field
    When I press Enter
    Then the address should become the results of that query
    And the list should show the title "Search results", the scope chip "In: Family archive" and a count chip with the number of files and their size
    And the footer should say how many results there are

  Scenario: The order starts at relevance and a column sort is this search's own
    Given the results are shown
    Then no column head should carry a sort mark
    When I choose the Size column
    Then the results should be asked in that order and the column should carry the sort mark
    When I choose the Size column twice more
    Then the order should return to relevance
    And my saved browsing sort should be unchanged

  Scenario: The address restores the bar
    Given I searched for "type:document harbour"
    When I go Back and then Forward
    Then the field should be empty on All files and show the "Type: Document" chip and "harbour" again

  # Unsupported operators
  Scenario: An operator the device cannot answer is refused and named
    When I search for "harbour sharedwith:me"
    Then nothing should have been searched and no files listed
    And the page should say "sharedwith: is not available on this device."
    And it should say the search was refused, not run without the operator
    And it should list the operators this device answers and the footer should read "0 results"

  Scenario: An operator that is not in the grammar is an invalid request
    When I search for "harbour colour:red"
    Then the page should say "colour: is not a search operator."
    And it should say the search was refused as an invalid request and list the operators below

  Scenario: The server's operators are not offered on the desktop
    When I open the drop-down
    Then it should not list "sharedwith:", "to:", "from:" or "pendingowner:" among the operators

  Scenario: A refusal that names no operator is an ordinary failure
    When the host refuses a search for a reason that names no operator
    Then the view should show its error or unavailable state with Try again, not the operator box

  # Sidebar filter
  Scenario: The sidebar filter narrows only the current view
    Given I am in a collection
    When I type "harbour" in the sidebar filter
    Then only that view's rows should be narrowed
    And the scope chip should read the view's name, not "search everywhere"
    And the count chip should show the number of files and their total size
    And no search should have run

  # Bounds and accessibility
  Scenario: Query bounds are kept
    When I type more than the API accepts
    Then the query asked should be at most 1,024 bytes and no more than eight chips should be applied

  Scenario: The field is a combobox I can use with the keyboard
    When I press the Down arrow in the field
    Then an option should become active and be named by the field
    When I press Enter
    Then the active option should be applied instead of searching
    When I press Escape
    Then the drop-down should close and my text should stay

  Scenario: Colours follow the theme tokens in light and dark
    When I switch between the light and the dark theme
    Then the field, chips, drop-down, results chips and refusal box should take their colours from the theme tokens

  # Source
  # - resources/prd.md F-063
  # - resources/srs.md SRS-F-063
  # - apps/filesquid/ui/resources/srs.md UI-S-007, UI-S-030
  # - apps/filesquid/crates/api/resources/srs.md API-S-012
  # - resources/ui-mockups/v3/20-search-suggestions.svg, 21-search-results.svg, 53-search-unknown-operator.svg
