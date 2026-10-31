/**
 * The statistics of the performance suite (Task UI-F3.1, NFR-002, RC-002): nearest-rank
 * percentiles, the summary every measurement reports (sample size, minimum, p50, p90, p95, p99,
 * maximum), the frame statistics of a 60 Hz run (missed frames, stalls, tail), the heap growth
 * ratio and the judge that turns a summary and a limit into failure messages. Pure functions,
 * so the arithmetic behind every budget assertion is itself tested.
 */
import { describe, expect, it } from "vitest";
import { frameStats, growth, judge, percentile, summarise } from "../performance/stats";

/** The integers 1 to n, in a fixed shuffled order so the order of the input never matters. */
function ranks(n: number): number[] {
  return Array.from({ length: n }, (_, i) => ((i * 37) % n) + 1);
}

describe("percentile (nearest rank)", () => {
  it("is the smallest sample with at least p percent of the samples at or below it", () => {
    const hundred = ranks(100);
    expect(percentile(hundred, 50)).toBe(50);
    expect(percentile(hundred, 95)).toBe(95);
    expect(percentile(hundred, 99)).toBe(99);
    expect(percentile(hundred, 100)).toBe(100);
  });

  it("uses the 190th sample for p95 of 200 and the 198th for p99", () => {
    const two = ranks(200);
    expect(percentile(two, 95)).toBe(190);
    expect(percentile(two, 99)).toBe(198);
  });

  it("does not change its input", () => {
    const input = [3, 1, 2];
    percentile(input, 50);
    expect(input).toEqual([3, 1, 2]);
  });

  it("answers the only sample for one sample", () => {
    expect(percentile([42], 95)).toBe(42);
  });

  it("refuses no samples, a percentile outside 0 to 100 and a sample that is not finite", () => {
    expect(() => percentile([], 95)).toThrow(RangeError);
    expect(() => percentile([1], 101)).toThrow(RangeError);
    expect(() => percentile([1], -1)).toThrow(RangeError);
    expect(() => percentile([1, Number.NaN], 50)).toThrow(RangeError);
    expect(() => percentile([1, Infinity], 50)).toThrow(RangeError);
  });
});

describe("summarise", () => {
  it("reports the sample size, the order statistics and the mean", () => {
    const summary = summarise(ranks(200));
    expect(summary).toEqual({
      n: 200,
      min: 1,
      p50: 100,
      p90: 180,
      p95: 190,
      p99: 198,
      max: 200,
      mean: 100.5,
    });
  });

  it("rounds nothing: a value keeps its fraction", () => {
    expect(summarise([0.25, 0.75]).mean).toBe(0.5);
  });
});

describe("frameStats", () => {
  const rules = { missedAboveMs: 25.05, stallAboveMs: 100, discard: 5 };

  it("discards the first frames, then counts missed frames and stalls", () => {
    const intervals = [500, 400, 300, 200, 100, ...Array(95).fill(16.7), 30, 120];
    const stats = frameStats(intervals, rules);
    expect(stats.frames).toBe(97);
    expect(stats.missed).toBe(2);
    expect(stats.stalls).toBe(1);
    expect(stats.missedRate).toBeCloseTo(2 / 97, 10);
    expect(stats.max).toBe(120);
  });

  it("treats exactly the threshold as on time", () => {
    const stats = frameStats([...Array(5).fill(1), 25.05, 25.05], rules);
    expect(stats.missed).toBe(0);
  });

  it("reports p50, p95 and p99 of the kept intervals", () => {
    const kept = Array.from({ length: 100 }, (_, i) => i + 1);
    const stats = frameStats([...Array(5).fill(1), ...kept], rules);
    expect([stats.p50, stats.p95, stats.p99]).toEqual([50, 95, 99]);
  });

  it("refuses a run with no frames left after the discard", () => {
    expect(() => frameStats([1, 2, 3], rules)).toThrow(RangeError);
  });
});

describe("growth", () => {
  it("is the fraction by which the value rose", () => {
    expect(growth(100, 110)).toBeCloseTo(0.1, 10);
    expect(growth(100, 90)).toBeCloseTo(-0.1, 10);
  });

  it("refuses a baseline that is not positive", () => {
    expect(() => growth(0, 5)).toThrow(RangeError);
  });
});

describe("judge", () => {
  const summary = summarise(ranks(200));

  it("passes when every limit holds", () => {
    expect(judge(summary, { minSamples: 100, p95Max: 190, p99Max: 198 })).toEqual([]);
  });

  it("names each limit that is exceeded, with the measured and the allowed value", () => {
    const failures = judge(summary, { minSamples: 100, p95Max: 189, p99Max: 197 });
    expect(failures).toHaveLength(2);
    expect(failures[0]).toContain("p95 190");
    expect(failures[0]).toContain("189");
    expect(failures[1]).toContain("p99 198");
  });

  it("fails a measurement with too few samples, whatever its percentiles are", () => {
    const failures = judge(summarise([1, 2, 3]), { minSamples: 100, p95Max: 1000 });
    expect(failures).toEqual(["n 3 is below the 100 samples the budget needs"]);
  });

  it("applies the maximum limit when one is given", () => {
    expect(judge(summary, { minSamples: 1, maxMax: 199 })[0]).toContain("max 200");
  });
});
