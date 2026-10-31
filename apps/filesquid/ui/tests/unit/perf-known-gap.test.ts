/**
 * The known-gap rule of the performance suite (Task UI-F3.1, Finding 4): a gap exempts only
 * the latency limits its records are known to miss (a percentile or the maximum above its
 * limit). A record that has too few samples, or a record outside the gap, still fails the run.
 */
import { describe, expect, it } from "vitest";
import { partitionFailures, treeGap } from "../performance/known-gap";
import type { Measurement } from "../performance/report";

/** A record with the given failures. */
function record(id: string, failures: string[]): Measurement {
  return { id, title: id, budget: "limit", gated: true, warmup: 20, failures, notes: {}, raw: [] };
}

const gap = treeGap("open-result", "open-feedback");

describe("partitionFailures", () => {
  it("tolerates a limit a gap record misses and keeps every other failure strict", () => {
    const { strict, tolerated } = partitionFailures(
      [
        record("open-result", [
          "p95 115.8 ms is above the 100 ms limit",
          "max 400 ms is above the 200 ms limit",
        ]),
        record("open-feedback", ["n 20 is below the 100 samples the budget needs"]),
        record("sort-result", ["p95 120 ms is above the 100 ms limit"]),
      ],
      gap,
    );
    expect(tolerated.map((line) => line.split(":")[0])).toEqual(["open-result", "open-result"]);
    expect(strict).toHaveLength(2);
    expect(strict.join("\n")).toContain("open-feedback: n 20 is below");
    expect(strict.join("\n")).toContain("sort-result: p95 120 ms");
  });

  it("has nothing tolerated when no gap record misses a limit", () => {
    const { strict, tolerated } = partitionFailures([record("open-result", [])], gap);
    expect(strict).toEqual([]);
    expect(tolerated).toEqual([]);
  });

  it("is all strict without a gap", () => {
    const { strict, tolerated } = partitionFailures(
      [record("open-result", ["p95 115.8 ms is above the 100 ms limit"])],
      undefined,
    );
    expect(strict).toHaveLength(1);
    expect(tolerated).toEqual([]);
  });
});
