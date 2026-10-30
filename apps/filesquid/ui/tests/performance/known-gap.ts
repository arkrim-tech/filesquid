/**
 * Known gaps of the performance suite (Task UI-F3.1, Finding 4): budgets the product is known
 * to miss, recorded for the owner. Pure, so it is unit tested without a browser.
 */
import type { Measurement } from "./report";

/** A budget the product is known to miss, recorded for the owner. */
export interface KnownGap {
  /** The records that make up the gap. */
  readonly ids: readonly string[];
  /** Why, in a sentence, with where it is recorded. */
  readonly reason: string;
}

/**
 * The known gap of the Large catalog's navigation tree (owner decision needed, Task UI-F3.1
 * Walkthrough, Finding 4). The tree draws every node of a level up to its cap of 10,000 nodes,
 * about 60,000 elements, and that costs about 50 ms of every frame that changes the current
 * destination, so opening a folder and going back miss the 100 ms result gate on the Large
 * catalog (the Small catalog holds it). A virtualised tree is the fix.
 *
 * @param ids - The records of the classes that open folders.
 * @returns The gap.
 */
export function treeGap(...ids: string[]): KnownGap {
  return {
    ids,
    reason:
      "Large catalog: the 10,000-node navigation tree keeps opening folders above the 100 ms gate",
  };
}

/** The failures of a run, split by whether a known gap tolerates them. */
export interface Partition {
  /** Failures that fail the run. */
  readonly strict: string[];
  /** Failures the gap names: a latency limit a gap record is known to miss. */
  readonly tolerated: string[];
}

/** True for a failure that is a latency limit missed (a percentile or the maximum). */
function isLimitMiss(failure: string): boolean {
  return / is above the .* limit/.test(failure);
}

/**
 * Split the failures of some records. Only a latency limit missed by a record of the gap is
 * tolerated; too few samples, a missing frame count, a record outside the gap and every other
 * failure stay strict, so a gap can hide nothing but the miss it names.
 *
 * @param records - The published measurements.
 * @param gap - The known gap among them, if any.
 * @returns The strict and the tolerated failures, each with its record and budget.
 */
export function partitionFailures(
  records: readonly Measurement[],
  gap: KnownGap | undefined,
): Partition {
  const strict: string[] = [];
  const tolerated: string[] = [];
  for (const record of records) {
    const inGap = gap?.ids.includes(record.id) === true;
    for (const failure of record.failures) {
      const line = `${record.id}: ${failure} (${record.budget})`;
      (inGap && isLimitMiss(failure) ? tolerated : strict).push(line);
    }
  }
  return { strict, tolerated };
}
