# API-S-034: Metadata property routes
# API-S-026: one codec for the per-file metadata record; uploads create the file's own object
# API-S-027: the read-only import-record projection
# SRS-F-052 / PRD F-052: metadata module; MD-S-002, MD-S-012, MD-S-013, MD-S-014
Feature: Metadata property routes
  As the owner of a FileSquid library
  I want to describe my files with Dublin Core, Schema.org and my own namespaces through the API
  So that every description is kept with the file in its OCFL object and survives an ordinary copy

  Scenario: An upload creates the file's object with its original and metadata record
    When the owner uploads "Holiday 2023.JPG" as a new file
    Then the file's object should hold the original and its metadata record in version "v1"
    And the record should name "Holiday 2023.JPG" with the client's facts and the receiving host's system, architecture and FileSquid version
    And every source, volume and attribute fact the upload did not supply should be listed as unavailable

  Scenario: An upload whose declared size differs commits nothing
    When the owner uploads "a.txt" declaring 9 bytes but sending 5
    Then the metadata answer should be the error "invalid_request"
    And no file object should have been created

  Scenario: Statements are set, read and removed, one version each
    Given the owner uploaded "harbour.jpg"
    When the owner sets "dcterms:title" to "Harbour"
    And the owner sets "dc:creator" to "Ana"
    And the owner removes "dcterms:creator"
    Then the file should show only "dcterms:title" as "Harbour"
    And the file's object should be at version "v4"

  Scenario: Unknown Dublin Core names and unregistered prefixes are refused
    Given the owner uploaded "a.txt"
    When the owner sets "dcterms:colour" to "red"
    Then the metadata answer should be the error "invalid_request"
    When the owner sets "rico:title" to "Fonds"
    Then the metadata answer should be the error "invalid_request"
    And the file's object should be at version "v1"

  Scenario: A registered namespace accepts its own terms
    Given the owner uploaded "a.txt"
    When the owner registers the namespace "rico" with the term "title"
    And the owner sets "rico:title" to "Fonds"
    Then the file should show only "rico:title" as "Fonds"

  Scenario: A bare term name is offered Dublin Core first
    Given the owner registered the namespace "rico" with the term "title"
    When the owner asks where "title" can go
    Then the answer should offer "dcterms:title", "schema:title" and "rico:title" in that order

  Scenario: The import record's facts show no absolute path, host or account
    Given the owner uploaded "notes.txt"
    When the owner reads the file's facts
    Then the facts should name "notes.txt" with its size and method "api_upload"
    And the facts should hold no absolute path, import root, host or account name

  Scenario: Content answers use the record's display name and media type
    Given the owner uploaded "Café.md" as "text/markdown"
    When the owner reads the file's original
    Then the answer should carry the media type "text/markdown" and the name "Café.md"
