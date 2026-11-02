/**
 * Unit tests for the live entries of the Logs view (UI-F2.16; UI-S-021): a `journal.entry` event
 * carries a journal entry without its PROV-O link, so the view builds the entry it lists from it:
 * a failed or refused entry or a single-file download is journal-only by definition, any other
 * entry names the operation and the object it carries, and the list merges live entries by ID.
 */
import { describe, expect, it } from "vitest";
import type { JournalEntry, JournalEntryView } from "../../src/api/client";
import { DENIED_OPERATION, isLiveEntry, liveEntryView, mergeNewest } from "../../src/logs/model";

const OP = "3f9c2a70-5be1-4c0d-9e44-81d7a2c90e21";

/** A live entry with the fields a test sets. */
function live(overrides: Partial<JournalEntry> = {}): JournalEntry {
  return {
    id: 7,
    time: "2026-10-19T11:03:02.418Z",
    host: "desktop",
    actor: "urn:uuid:5a0c41e2-0000-4000-8000-00000000e19b",
    operation: "metadata.set",
    outcome: "succeeded",
    level: "info",
    target_ids: ["lib-1"],
    operation_id: OP,
    object_ref: "urn:uuid:1d0e88b4-0000-4000-8000-00000000a14c@v8",
    ...overrides,
  };
}

describe("the entry a live event becomes", () => {
  it("links a committed operation to its activities by its operation and object", () => {
    const view = liveEntryView(live());
    expect(view.provenance).toEqual({
      journal_only: false,
      operation_id: OP,
      object_ref: "urn:uuid:1d0e88b4-0000-4000-8000-00000000a14c@v8",
    });
  });

  it("marks failed and refused entries and downloads journal-only", () => {
    expect(liveEntryView(live({ outcome: "failed" })).provenance.journal_only).toBe(true);
    expect(liveEntryView(live({ operation: DENIED_OPERATION })).provenance.journal_only).toBe(true);
    expect(liveEntryView(live({ operation: "file.download" })).provenance.journal_only).toBe(true);
  });

  it("keeps every field of the entry and adds no name", () => {
    const view = liveEntryView(live({ duration_ms: 38 }));
    expect(view.duration_ms).toBe(38);
    expect(view.display_name).toBeUndefined();
  });

  it("recognises an event whose data is a journal entry", () => {
    expect(isLiveEntry(live())).toBe(true);
    expect(isLiveEntry({ id: 1 })).toBe(false);
    expect(isLiveEntry(null)).toBe(false);
    expect(isLiveEntry("entry")).toBe(false);
  });
});

describe("merging live entries into the list", () => {
  const row = (id: number): JournalEntryView => liveEntryView(live({ id }));

  it("puts the newest first and keeps each ID once", () => {
    const merged = mergeNewest([row(5), row(4)], [row(6), row(5)]);
    expect(merged.map((e) => e.id)).toEqual([6, 5, 4]);
  });

  it("keeps the list as it is when there is nothing new", () => {
    const list = [row(5), row(4)];
    expect(mergeNewest(list, [])).toEqual(list);
  });
});
