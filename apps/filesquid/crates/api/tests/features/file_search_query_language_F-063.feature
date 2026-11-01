# F-063: File search query language and filter chips
# SRS-F-063: File search query language and filter chips
# API-S-012: Search operation and operator grammar
Feature: File search query language and filter chips
  As a user
  I want to find files with chips and a small query language across everything I may read
  So that search is fast and familiar without leaking what I may not see

  Background:
    Given a filesquid-server with users Ana and Ben
    And Ana owns a library with files titled "Budget 2026", "Holiday notes" and "Client deck"
    And Ben may read only "Client deck"

  # Permissions
  Scenario: Results are filtered by permission before ranking
    When Ben searches for "2026"
    Then the result list should be empty
    And no suggestion should contain "Budget 2026"

  Scenario: Results are paged with stable ordering
    Given Ana has 500 files whose title contains "report"
    When Ana searches for "report" and pages through the results
    Then every page should carry a cursor and no file should repeat or disappear between pages

  # Free text and operators
  Scenario: Free text matches filename and Dublin Core fields
    Given a file named "img_0001.jpg" whose dcterms:description is "sunset over the harbour"
    When Ana searches for "harbour"
    Then "img_0001.jpg" should be in the results

  Scenario: Quoted phrases match exactly and a leading minus excludes
    When Ana searches for "\"Client deck\" -draft"
    Then every result should contain the phrase "Client deck" and none should contain "draft"

  Scenario Outline: Operators narrow the result set
    Given one file Ana may read for which "<condition>" holds and one for which it does not
    When Ana searches for "<query>"
    Then exactly the file for which "<condition>" holds should be returned

    Examples:
      | query                      | condition                                                    |
      | type:image                 | the metadata module identifies the format as an image        |
      | title:Budget               | dcterms:title contains Budget                                |
      | owner:ana                  | Ana owns the file                                            |
      | creator:"Ana Example"      | dcterms:creator equals Ana Example                           |
      | tag:finance                | dcterms:subject contains finance                             |
      | is:favorite                | Ana marked the file as a favorite                            |
      | is:deleted                 | the file is recoverable in deleted files                     |
      | after:2026-01-01           | the latest OCFL version was created on or after 2026-01-01   |
      | createdbefore:2020-01-01   | the original creation date is before 2020-01-01              |
      | source:example.org         | the acquisition origin recorded by the clipping module       |
      | size>100MB                 | the file is larger than 100 MB                               |
      | in:Campaign                | the file's logical path is under the folder Campaign         |
      | sharedwith:public          | the file is reachable through a public link or exhibit       |
      | pendingowner:me            | an ownership transfer to Ana is pending                      |
      | to:ben                     | Ana shared the file with Ben                                 |
      | from:ben                   | Ben shared the file with Ana                                 |

  Scenario: Date boundaries respect the caller's zone and never invent instants
    Given a file with a capture date "2026-03-01" that has no time zone
    When Ana searches for "createdafter:2026-02-28" with her zone set to UTC-8
    Then the file should match by calendar date
    And the API should not have assigned it an invented UTC instant

  Scenario: Unknown operators return an error, never an empty-filter match
    When Ana searches for "colour:blue"
    Then the API should return an explicit unknown-operator error
    And no results should be returned

  Scenario: generator: is not part of the grammar without the c2pa module
    Given the c2pa module is not enabled
    When Ana searches for "generator:\"Camera App\""
    Then the API should return an explicit unknown-operator error
    And no results should be returned

  Scenario: generator: matches the C2PA claim generator once the c2pa module is enabled
    Given the c2pa module is enabled
    And one file Ana may read whose C2PA claim generator is "Camera App" and one whose generator is not
    When Ana searches for "generator:\"Camera App\""
    Then exactly the file whose claim generator is "Camera App" should be returned

  Scenario: Case folding and Unicode normalisation follow the declared contract
    Given a file titled "Café Résumé"
    When Ana searches for "CAFÉ RÉSUMÉ"
    Then the file should match, because the contract declares NFC normalisation and simple case folding
    When Ana searches for "cafe resume"
    Then the file should not match, because the contract does not declare diacritic folding

  # Ordering and indexes
  Scenario: Relevance is the default order and any view sort key can replace it
    When Ana searches for "deck"
    Then results should be ordered by relevance
    When Ana switches the order to "modified" and back to relevance
    Then both orders should be honoured without a new query being required

  Scenario: Indexes are rebuildable from canonical state
    Given the operational store is deleted while the OCFL root is intact
    When the host rebuilds its indexes
    Then every previous search should return the same results

  Scenario: Content search is declared per format and runs only on the authorised endpoint
    Given full-text indexing covers PDF and plain text and never media files, as the capability register declares
    When Ana searches for a phrase that appears inside a PDF and inside a DNG's embedded text
    Then the PDF should match and the DNG should be reported as unsupported for content search
    And no content indexing should have run on the ciphertext-only hosted service

  # AI
  Scenario: Natural-language answers go only through the user's own connection
    Given Ana has no BYO AI connection configured
    When Ana types the question "What did I spend in March?"
    Then the search should run as plain text and no answer model should be invoked
    Given Ana configures her own provider and opts in for this task
    When Ana asks the same question
    Then the answer should list the files used as sources

  # Default UI
  Scenario: Filter chips combine with text and can be removed
    When I open the global search bar and type "pres"
    Then a Type chip for presentations should be suggested
    And Location, People and Modified chips should be available
    When I select the chip and type "Client"
    Then the drop-down should show only presentations matching "Client"
    When I press Enter
    Then the full result list should open with sortable column headers
    When I remove the chip
    Then the results should widen to every type

  Scenario: The sidebar filter narrows only the current view
    Given I am in the favorites view
    When I type "deck" in the sidebar filter field
    Then only favorites matching "deck" should remain
    And the scope chip should read the favorites view name, not "search everywhere"
    And a result summary should show the count and total size

  Scenario: Unsupported operators are shown as unsupported in the UI
    Given the host does not support the "sharedwith:" operator
    When I search for "sharedwith:public"
    Then the UI should say the operator is unsupported on this host

  # Source
  # - resources/prd.md F-063
  # - resources/srs.md SRS-F-063
  # - apps/filesquid/crates/api/resources/srs.md API-S-012
  # - apps/filesquid/ui/resources/srs.md UI-S-007
