/**
 * Reporting of the browsing performance suite (Task UI-F3.1, ADR 017, test-cleanup rule). Each
 * measurement is turned into a record that carries what NFR-002 and RC-002 ask a report to
 * show: sample size, warm-up, the order statistics (never an average alone), the tail, the
 * budget it was judged against and every failure. The record is written as JSON under
 * `tests/allure-results/performance/` (cleared before each run by `pretest:perf`), attached to
 * the test (so Allure shows it) and printed as a table by the suite's global teardown.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { expect, type TestInfo } from "@playwright/test";
import { BUDGETS } from "./budgets";
import { frameStats, judge, summarise, type FrameStats, type Limit, type Summary } from "./stats";

/** Where the suite's raw results go. */
export const RESULTS_DIR = resolve(process.cwd(), "tests/allure-results/performance");

/** One reported measurement. */
export interface Measurement {
  /** Stable slug; the file name of its JSON. */
  readonly id: string;
  /** What was measured, in words. */
  readonly title: string;
  /** The budget it was judged against, in words with its source. */
  readonly budget: string;
  /** False for a measurement that is reported as evidence and not asserted. */
  readonly gated: boolean;
  /** Samples thrown away before measuring. */
  readonly warmup: number;
  /** The order statistics of the samples (milliseconds), absent for a frame run. */
  readonly summary?: Summary;
  /** The statistics of a frame run, absent for a latency. */
  readonly frames?: FrameStats;
  /** Every failure of the budget; empty when it holds. */
  readonly failures: string[];
  /** Anything else worth reporting: coverage, environment, counts, decomposition. */
  readonly notes: Record<string, string | number | boolean>;
  /** The raw samples, so a reader can recompute every number. */
  readonly raw: readonly number[];
}

/** What a latency record is made from. */
export interface LatencyInput {
  /** Stable slug. */
  id: string;
  /** What was measured. */
  title: string;
  /** The budget in words with its source. */
  budget: string;
  /** The samples in milliseconds, after the warm-up. */
  samples: readonly number[];
  /** Samples thrown away before measuring. */
  warmup: number;
  /** The limits; without them the measurement is reported and not asserted. */
  limit?: Limit;
  /** Extra facts to report. */
  notes?: Record<string, string | number | boolean>;
}

/**
 * Build the record of a latency measurement.
 *
 * @param input - The samples and the limits they are judged against.
 * @returns The record, with its failures.
 */
export function latency(input: LatencyInput): Measurement {
  const summary = summarise(input.samples);
  return {
    id: input.id,
    title: input.title,
    budget: input.budget,
    gated: input.limit !== undefined,
    warmup: input.warmup,
    summary,
    failures: input.limit ? judge(summary, input.limit) : [],
    notes: input.notes ?? {},
    raw: input.samples,
  };
}

/** What a frame record is made from. */
export interface FramesInput {
  /** Stable slug. */
  id: string;
  /** What was measured. */
  title: string;
  /** The frame intervals of each run, in order; the first frames of every run are discarded. */
  runs: readonly (readonly number[])[];
  /** Extra facts to report. */
  notes?: Record<string, string | number | boolean>;
  /** Judge against the app's limits (true) or the stricter control limit (false). */
  app: boolean;
  /** Fewest frames the run needs (default {@link BUDGETS.minFramesPerLayout}). */
  minFrames?: number;
}

/**
 * Build the record of a 60 Hz frame run and judge it against the frame limits.
 *
 * @param input - The intervals and what they are.
 * @returns The record, with its failures.
 */
export function frames(input: FramesInput): Measurement {
  const kept = input.runs.flatMap((run) => run.slice(BUDGETS.discardFrames));
  const stats = frameStats(kept, {
    missedAboveMs: BUDGETS.missedFrameAboveMs,
    stallAboveMs: BUDGETS.stallAboveMs,
    discard: 0,
  });
  const failures: string[] = [];
  const maxRate = input.app ? BUDGETS.missedFrameRateMax : BUDGETS.controlMissedRateMax;
  const percent = (value: number) => `${Math.round(value * 1000) / 10}%`;
  if (stats.missedRate > maxRate) {
    failures.push(
      `missed frames ${percent(stats.missedRate)} are above the ${percent(maxRate)} limit`,
    );
  }
  if (input.app && stats.p99 > BUDGETS.frameP99Ms) {
    failures.push(`p99 frame ${stats.p99} ms is above the ${BUDGETS.frameP99Ms} ms limit`);
  }
  if (input.app && stats.stalls > stats.frames * BUDGETS.stallRateMax) {
    failures.push(
      `${stats.stalls} stalls above ${BUDGETS.stallAboveMs} ms in ${stats.frames} frames exceed ${percent(BUDGETS.stallRateMax)}`,
    );
  }
  const minFrames = input.minFrames ?? BUDGETS.minFramesPerLayout;
  if (stats.frames < minFrames) {
    failures.push(`${stats.frames} frames are below the ${minFrames} the budget needs`);
  }
  return {
    id: input.id,
    title: input.title,
    budget: `60 Hz, frame budget ${BUDGETS.frameBudgetMs} ms (NFR-002); missed > ${BUDGETS.missedFrameAboveMs.toFixed(2)} ms`,
    gated: true,
    warmup: BUDGETS.discardFrames,
    frames: stats,
    failures,
    notes: input.notes ?? {},
    raw: kept,
  };
}

/** What a single-value record is made from (heap growth, DOM rows, counts). */
export interface ValueInput {
  /** Stable slug. */
  id: string;
  /** What was measured. */
  title: string;
  /** The limit in words with its source. */
  budget: string;
  /** The measured value. */
  value: number;
  /** The most it may be, when it is asserted. */
  max?: number;
  /** Extra facts to report. */
  notes?: Record<string, string | number | boolean>;
}

/**
 * Build the record of one measured value.
 *
 * @param input - The value and its limit.
 * @returns The record.
 */
export function value(input: ValueInput): Measurement {
  const failures =
    input.max !== undefined && input.value > input.max
      ? [`${input.value} is above the limit ${input.max}`]
      : [];
  return {
    id: input.id,
    title: input.title,
    budget: input.budget,
    gated: input.max !== undefined,
    warmup: 0,
    failures,
    notes: { value: input.value, ...(input.notes ?? {}) },
    raw: [input.value],
  };
}

/**
 * Write the records to the results directory and attach them to the test.
 *
 * @param info - The running test.
 * @param records - The measurements to publish.
 * @returns Resolves when written and attached.
 */
export async function publish(info: TestInfo, records: readonly Measurement[]): Promise<void> {
  mkdirSync(RESULTS_DIR, { recursive: true });
  for (const record of records) {
    const body = JSON.stringify(record, null, 2);
    writeFileSync(join(RESULTS_DIR, `${record.id}.json`), body);
    await info.attach(`${record.id}.json`, { body, contentType: "application/json" });
  }
}

/**
 * Fail the test for every record whose budget did not hold, naming each failure.
 *
 * @param records - The published measurements.
 */
export function assertBudgets(records: readonly Measurement[]): void {
  const failed = records.flatMap((record) =>
    record.failures.map((failure) => `${record.id}: ${failure} (${record.budget})`),
  );
  expect(failed, "performance budgets").toEqual([]);
}
