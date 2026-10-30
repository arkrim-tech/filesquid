# API-S-036: Import route — Review (owner answer 5, 2026-10-22, "Keep the review"): after the
# owner picks the source and before any byte is staged, the import route MUST answer a plan
# (files found, new and already in this library, bytes to copy, the destination, the names to
# be numbered, free bytes after the upload, the staging limit); the plan items route MUST list
# the planned files by base name, at most 1,000 per page; the job MUST start only on the start
# route (a repeated start answers the same job); a plan MUST be discarded by DELETE or after
# 1 hour, changing nothing; a changed source MUST be refused as source_changed; a repeated
# operation ID answers its open plan, its started job or cancelled without a dialog; the start
# and discard routes take no CommandEnvelope, the plan ID being their idempotency key (API-S-002,
# owner decision 2026-10-30), so a repeated start answers the same job and a repeated discard is
# plan_not_found; every plan
# request, start and discard MUST be journaled once with IDs only, never a path or a file name.
# API-S-037: Job routes — Per-file progress (owner answer 6, 2026-10-22, "Yes, report progress
# per file"): an upload job's view MUST carry its destination; the job items route MUST list its
# files with display name, destination folder, state, bytes and error code, never a path; a
# retry of one failed file or of all failed files MUST put them back to queued and hand the job
# to its runner, refuse a paused job as job_paused with no change, change nothing when repeated,
# and journal each file put back once as job.item_retried.
# Task API-F2.16 (contract 0.32.0).
Feature: Upload review step and per-file job progress
  As a library owner
  I want to review what an upload will do before it starts and follow every file while it runs
  So that nothing is copied by surprise and a failed file can be retried on its own

  Background:
    Given a desktop host whose import dialog plans what the owner picks

  # Happy path
  Scenario: A picked folder is planned before any byte is staged
    When the owner asks to upload a folder into a collection
    Then the review should show 3 files found, 2 new and 1 already in this library
    And the review should name the destination, the numbered names, the free bytes after the upload and the staging limit
    And no job should have started
    And the plan request should be journaled once with the plan and no file name

  Scenario: The planned files are listed by base name only
    Given the owner has asked to upload a folder into a collection
    When the owner reads the planned files 2 at a time
    Then the files should be listed by base name with their size and whether they are new
    And no path should appear in any answer

  Scenario: Confirming the plan starts one job and a second confirmation answers the same job
    Given the owner has asked to upload a folder into a collection
    When the owner confirms the upload twice
    Then one import job should have started and both answers should name it
    And each start should be journaled once with the job

  Scenario: A repeated request answers the open plan without a second dialog
    Given the owner has asked to upload a folder into a collection
    When the same request is sent again with the same operation ID
    Then the open plan should be answered again
    And the dialog should have been shown once

  Scenario: A repeated request after the upload started answers the started job
    Given the owner has asked to upload a folder into a collection
    And the owner has confirmed the upload
    When the same request is sent again with the same operation ID
    Then the started job should be answered again
    And the dialog should have been shown once

  Scenario: A repeated request after the owner closed the dialog answers cancelled
    Given a desktop host whose import dialog the owner closes
    And the owner has asked to upload a folder into a collection
    When the same request is sent again with the same operation ID
    Then the upload should be answered as cancelled again
    And the dialog should have been shown once

  # Edge cases
  Scenario: The owner discards the plan and nothing changes
    Given the owner has asked to upload a folder into a collection
    When the owner discards the plan
    Then the plan should be gone and no job should have started
    And the discard should be journaled once with the plan

  Scenario: A discarded plan discarded again is plan_not_found and changes nothing
    Given the owner has asked to upload a folder into a collection
    And the owner has discarded the plan
    When the owner discards the plan again
    Then the discard should be refused as "plan_not_found"
    And the plan should be gone and no job should have started
    And the discard should be journaled once with the plan

  Scenario: A page of planned files holds at most 1,000 files
    Given the owner has asked to upload a folder into a collection
    When the owner asks for 1001 planned files at once
    Then the read should be refused as an invalid request

  Scenario: A plan that nobody confirms expires after an hour
    Given the owner has asked to upload a folder into a collection
    When an hour passes
    Then the plan should be gone and no job should have started

  # Error case
  Scenario: A source that changed since the plan is refused and the plan stays open
    Given the owner has asked to upload a folder into a collection
    And the picked files changed since the plan
    When the owner confirms the upload
    Then the upload should be refused as "source_changed"
    And the plan should still be open

  Scenario: A host without an import dialog or per-file progress answers unavailable
    Given a host without an import dialog or per-file progress
    When the owner asks to upload, reads a plan's files and reads a job's files
    Then every answer should be unavailable

  # Per-file progress
  Scenario: An upload job shows its destination and every file's state
    Given an upload job with a copied, a copying and a failed file
    When the owner reads the job and its files
    Then the job should name its destination collection and folder
    And the files should show their name, folder, state and bytes and never a path

  Scenario: Retrying the failed file queues it again and hands the job to its runner
    Given an upload job with a copied, a copying and a failed file
    When the owner retries the failed file
    Then the file should be queued again and the job handed to its runner
    And the retry should be journaled once for that file without its name
    When the owner retries the failed file
    Then nothing should change and nothing should be handed to the runner

  Scenario: Retrying all failed files queues every failed file and hands the job to its runner
    Given an upload job with a copied, a copying and a failed file
    And the copying file failed too
    When the owner retries all failed files
    Then both failed files should be queued again and the job handed to its runner
    And the retry should be journaled once for each file without its name

  Scenario: A paused job refuses a retry and nothing changes
    Given an upload job with a copied, a copying and a failed file
    And the job is paused
    When the owner retries all failed files
    Then the retry should be refused as "job_paused" and the file should stay failed
