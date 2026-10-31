/**
 * Packaged-window proof of the database-deleted journal rebuild, first launch (Task DE-0.11;
 * DE-S-013, API-S-032): the running app has journaled its start, which this spec keeps for the
 * second spec. When this spec's worker ends, `wdio.conf.mjs` closes the app's last window (so
 * the app seals its journal at close into `<data dir>/journal.ocfl/`), waits for it to quit and
 * deletes the operational store `filesquid.sqlite` (`support/app-restart.mjs`). The second spec
 * (`journal-rebuild-2-refill`) runs against the app the Tauri service starts again.
 */
import { expect } from "@wdio/globals";
import { keepEntry, waitForEntry } from "../support/journal.mjs";

describe("the journal survives a deleted operational store (first launch)", () => {
  it("has journaled the app's start under a journal ID", async () => {
    const started = await waitForEntry("host.started");
    expect(typeof started.id).toBe("number");
    keepEntry(started);
  });
});
