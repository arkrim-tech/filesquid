/**
 * Packaged-window proof of the database-deleted journal rebuild, second launch (Task DE-0.11;
 * DE-S-013): the app, closed after the first spec and started again over the data directory
 * whose `filesquid.sqlite` was then deleted, recreates the store and refills it from the sealed journal. The first launch's
 * `host.started` entry is back under the same journal ID, the refill is journaled once as
 * `journal.refill` succeeded, and no entry names a path of the data directory.
 */
import { expect } from "@wdio/globals";
import { dataDir } from "../support/app-restart.mjs";
import { journalEntries, keptEntry, waitForEntry } from "../support/journal.mjs";

describe("the journal survives a deleted operational store (refill)", () => {
  it("refills the new store from the sealed journal under the same IDs", async () => {
    const kept = keptEntry();
    const refill = await waitForEntry("journal.refill");
    expect(refill.outcome).toBe("succeeded");
    const started = await journalEntries("host.started");
    const back = started.find((entry) => entry.id === kept.id);
    expect(back).toBeDefined();
    expect(back.time).toBe(kept.time);
    expect(started.length).toBeGreaterThan(1);
    const seals = await journalEntries("journal.seal");
    expect(seals.some((entry) => entry.outcome === "succeeded")).toBe(true);
    const text = JSON.stringify([...started, ...seals, refill]);
    expect(text.includes(dataDir())).toBe(false);
  });
});
