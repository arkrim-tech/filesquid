# OCFL-S-025: Copy-guard warnings name the object. The warning a refused copy (OCFL-S-019) emits MUST name the object the copy concerns by its identifier, and its version wherever the call knows it, beside the destination and the declaring root, so a host can journal it with an object reference.
# F-061: Activity journal and live health monitoring
# F-051: OCFL module
Feature: Copy-guard warnings name the object
  As a host that journals every warning of the OCFL engine
  I want a refused copy's warning to name the object and version it concerns
  So that the journal entry carries an object reference, not only a storage path

  Background:
    Given an OCFL object "obj" committed at "v1" through the engine writer

  # Happy path: a copy out of a version knows the object and the version
  Scenario: A refused file copy names the source object and version
    When I copy "b.txt" of "v1" into "obj/v1/content/c.txt" while warnings are recorded
    Then the copy should be refused and one copy-guard warning recorded
    And the warning should name the destination "obj/v1/content/c.txt" and the root "obj"
    And the warning should name the object "urn:example:obj" at version "v1"

  # Edge case: a checkout of a whole version into the object root
  Scenario: A refused checkout names the object and version
    When I check out "v1" under "obj/out" while warnings are recorded
    Then the copy should be refused and one copy-guard warning recorded
    And the warning should name the destination "obj/out/a.txt" and the root "obj"
    And the warning should name the object "urn:example:obj" at version "v1"

  # Edge case: staging knows its object but no version
  Scenario: A refused staging write names the object without a version
    When I stage "d.txt" in a staging area under "obj/staging" while warnings are recorded
    Then the copy should be refused and one copy-guard warning recorded
    And the warning should name a destination under "obj/staging/" and the root "obj"
    And the warning should name the object "urn:example:obj" without a version

  # Source
  # - resources/prd.md F-061, F-051
  # - apps/filesquid/crates/ocfl/resources/srs.md OCFL-S-025, OCFL-S-019
  # - apps/filesquid/crates/api/resources/srs.md API-S-031
