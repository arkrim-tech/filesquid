# F-061: Activity journal and live health monitoring
# SRS-F-061: Activity journal and live health monitoring
Feature: Activity journal and live health monitoring
  As an owner or administrator
  I want a complete record of everything FileSquid did and a live view of its health
  So that nothing happens unrecorded and I can act on stalled or failing processing

  Scenario: Every domain operation is journaled with the change
    Given the desktop host with an empty activity journal
    When I import 3 files, rename one, and edit its dcterms:title
    Then the journal should contain one entry per import, the OCFL commit of each, the rename and the metadata write
    And each entry should have the time, actor, operation, outcome and opaque file ids

  Scenario: A failed operation is journaled too
    Given a storage root that is read-only
    When I import a file
    Then the import should fail with a visible error
    And the journal should contain the attempt with outcome "failed" and an error code

  Scenario: No event is lost under load
    Given the journal queue capacity is N events
    When 10 x N job and process events are produced as fast as possible
    Then the journal should contain all 10 x N events

  # The native SQLite store writing no log file is specified with that store:
  # crates/native/tests/features/native_journal_store_F-061.feature

  Scenario: Entries are redacted
    Given a file titled "Grandma's birthday" was imported with GPS metadata
    When the administrator searches the journal
    Then no entry should contain the title, coordinates, file contents, keys or credentials

  Scenario: Members cannot read the journal or health
    Given a filesquid-server with users Ana (administrator) and Ben (member)
    When Ben requests GET /api/v1/admin/journal and GET /api/v1/admin/health
    Then both responses should be 403

  Scenario: The journal can be searched and exported
    Given journal entries from the last 30 days
    When the administrator filters by operation "ocfl.commit" and outcome "failed" and exports
    Then the export should be JSON Lines containing exactly the matching entries

  Scenario: A stalled process degrades health live
    Given an import job that sends heartbeats every 5 seconds with a 30 second timeout
    When the job stops sending heartbeats for 31 seconds
    Then the process registry should mark the job "stalled"
    And health should become "degraded" with reason "process stalled"
    And the Monitor dashboard should show the change without a refresh

  Scenario: The Monitor dashboard shows processing in the FileSquid domain
    Given an import, a C2PA scan and a sync session are running
    When the owner opens the Monitor dashboard
    Then each process should show its type, state, progress, rate and ETA
    And the dashboard should show queues, throughput, error rate, storage roots and free space

  Scenario: Metrics agree across exposition formats
    Given filesquid-server is running
    When I read /metrics and GET /api/v1/admin/metrics
    Then every counter should report the same value in both

  Scenario: Optional OTLP export loses nothing while the sink is down
    Given OTLP export is enabled to a VictoriaLogs endpoint
    And the endpoint is unreachable for 10 minutes while activity continues
    When the endpoint comes back
    Then every journal entry from the outage should arrive in VictoriaLogs at least once
    And each arrived entry should carry its journal id so duplicates can be removed

  # Source
  # - resources/prd.md F-061
  # - resources/srs.md SRS-F-061
  # - resources/architecture/033-activity-journal-and-live-health-monitoring.md
