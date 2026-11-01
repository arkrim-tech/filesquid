# API-S-012: Search operation and operator grammar (Task API-F2.3)
# SRS-F-063 / PRD F-063: file search query language; SRS-NFR-004: permission filtering before ranking
# The multi-user scenarios of file_search_query_language_F-063.feature need filesquid-server (SV-F1.4).
Feature: Search operation and operator grammar
  As the owner of a FileSquid library
  I want to find files with free text and a small operator grammar
  So that search is fast and familiar and never guesses what an unknown operator meant

  Background:
    Given a library whose files are "budget.pdf" titled "Budget 2026", "img_0001.jpg" described as "sunset over the harbour" and "Client deck.pptx"

  Scenario: Free text matches the name and Dublin Core values
    When the owner searches for "harbour"
    Then the results should be "img_0001.jpg"

  Scenario: Quoted phrases match exactly and a leading minus excludes
    Given the further files "deck for client.txt" and "Client deck draft.pptx"
    When the owner searches for "\"Client deck\" -draft"
    Then the results should be "Client deck.pptx"

  Scenario Outline: Operators narrow the result set
    When the owner searches for "<query>"
    Then the results should be "<results>"

    Examples:
      | query                  | results          |
      | title:budget           | budget.pdf       |
      | type:image             | img_0001.jpg     |
      | size<10 -type:folder   | Client deck.pptx |
      | holi type:folder       | Holidays         |

  Scenario: Unknown operators return an error, never an empty-filter match
    When the owner searches for "colour:blue"
    Then the search should be refused as "invalid_request"

  Scenario: is:unfiled is retired, because no file is ever unfiled (ADR 041 Decision 8)
    When the owner searches for "is:unfiled"
    Then the search should be refused as "invalid_request"
    And the refusal message should name "is:unfiled"

  Scenario: generator: is not part of the grammar without the c2pa module
    When the owner searches for "generator:\"Camera App\""
    Then the search should be refused as "invalid_request"

  Scenario: Operators this host cannot answer are refused, not ignored
    When the owner searches for "sharedwith:public"
    Then the search should be refused as "unavailable"

  Scenario: Results are filtered by permission before ranking
    Given a file "Budget 2026 secret.pdf" in a collection the owner may not read
    When the owner searches for "budget"
    Then the results should be "budget.pdf"

  Scenario: Case folding follows the declared contract
    When the owner searches for "BUDGET"
    Then the results should be "budget.pdf"
