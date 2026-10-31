/**
 * Statistics of the performance suite (Task UI-F3.1, NFR-002, RC-002). Pure arithmetic: nearest-rank
 * percentiles (the method that never invents a value no sample had), the summary every
 * measurement reports, the statistics of a 60 Hz frame run, the heap growth ratio and the judge
 * that turns a summary and a limit into failure messages. Reports show percentiles and tails,
 * never an average alone (NFR-002).
 */

/** What every measurement reports. */
export interface Summary {
  /** Number of samples. */
  readonly n: number;
  /** Smallest sample. */
  readonly min: number;
  /** Median (nearest rank). */
  readonly p50: number;
  /** 90th percentile (nearest rank). */
  readonly p90: number;
  /** 95th percentile (nearest rank). */
  readonly p95: number;
  /** 99th percentile (nearest rank). */
  readonly p99: number;
  /** Largest sample. */
  readonly max: number;
  /** Arithmetic mean; reported, never gated. */
  readonly mean: number;
}

/**
 * The nearest-rank percentile: the smallest sample with at least `p` percent of the samples at
 * or below it, which is the sample of rank `ceil(p / 100 * n)`.
 *
 * @param samples Measured values, in any order; not changed.
 * @param p Percentile from 0 to 100.
 * @returns The sample at that rank.
 * @throws RangeError when there are no samples, a sample is not finite or `p` is outside 0 to 100.
 */
export function percentile(samples: readonly number[], p: number): number {
  if (samples.length === 0) throw new RangeError("percentile of no samples");
  if (!(p >= 0 && p <= 100)) throw new RangeError(`percentile ${p} is outside 0 to 100`);
  if (!samples.every(Number.isFinite)) throw new RangeError("a sample is not a finite number");
  const sorted = [...samples].sort((a, b) => a - b);
  const rank = Math.max(1, Math.ceil((p / 100) * sorted.length));
  return sorted[rank - 1] as number;
}

/**
 * Summarise a measurement.
 *
 * @param samples Measured values in milliseconds (or any unit), in any order.
 * @returns Sample size, minimum, p50, p90, p95, p99, maximum and mean.
 * @throws RangeError as {@link percentile} does.
 */
export function summarise(samples: readonly number[]): Summary {
  const sum = samples.reduce((total, sample) => total + sample, 0);
  return {
    n: samples.length,
    min: percentile(samples, 0),
    p50: percentile(samples, 50),
    p90: percentile(samples, 90),
    p95: percentile(samples, 95),
    p99: percentile(samples, 99),
    max: percentile(samples, 100),
    mean: sum / samples.length,
  };
}

/** How a frame run is judged. */
export interface FrameRules {
  /** A frame is missed when its interval is greater than this many milliseconds. */
  readonly missedAboveMs: number;
  /** A frame is a stall when its interval is greater than this many milliseconds. */
  readonly stallAboveMs: number;
  /** Frames at the start of the run that are thrown away (start-up cost is not scrolling). */
  readonly discard: number;
}

/** What a frame run reports. */
export interface FrameStats {
  /** Frames kept. */
  readonly frames: number;
  /** Frames whose interval was above the missed threshold. */
  readonly missed: number;
  /** `missed / frames`. */
  readonly missedRate: number;
  /** Frames whose interval was above the stall threshold. */
  readonly stalls: number;
  /** Median interval. */
  readonly p50: number;
  /** 95th percentile interval. */
  readonly p95: number;
  /** 99th percentile interval. */
  readonly p99: number;
  /** Longest interval. */
  readonly max: number;
}

/**
 * The statistics of a run of frame intervals.
 *
 * @param intervals Milliseconds between consecutive animation frames, in order.
 * @param rules The missed and stall thresholds and the number of frames to discard.
 * @returns The kept frame count, missed and stalled frames, missed rate and the tail.
 * @throws RangeError when no frame is left after the discard.
 */
export function frameStats(intervals: readonly number[], rules: FrameRules): FrameStats {
  const kept = intervals.slice(rules.discard);
  if (kept.length === 0) throw new RangeError("no frame left after the discard");
  const missed = kept.filter((interval) => interval > rules.missedAboveMs).length;
  const stalls = kept.filter((interval) => interval > rules.stallAboveMs).length;
  const summary = summarise(kept);
  return {
    frames: kept.length,
    missed,
    missedRate: missed / kept.length,
    stalls,
    p50: summary.p50,
    p95: summary.p95,
    p99: summary.p99,
    max: summary.max,
  };
}

/**
 * The fraction by which a value rose.
 *
 * @param before The baseline, greater than zero.
 * @param after The later value.
 * @returns `(after - before) / before`.
 * @throws RangeError when the baseline is not positive.
 */
export function growth(before: number, after: number): number {
  if (!(before > 0)) throw new RangeError("a growth baseline must be positive");
  return (after - before) / before;
}

/** The limits one measurement is judged against. */
export interface Limit {
  /** Fewest samples the budget accepts (NFR-002 asks for at least 100). */
  readonly minSamples: number;
  /** p95 must be at most this. */
  readonly p95Max?: number;
  /** p99 must be at most this. */
  readonly p99Max?: number;
  /** The maximum must be at most this. */
  readonly maxMax?: number;
}

/**
 * Judge a measurement against its limits.
 *
 * @param summary The measurement's summary.
 * @param limit Its limits.
 * @returns One message per limit that failed, naming the measured and the allowed value; an
 *   empty list when the budget holds. Too few samples fails on that alone, because percentiles
 *   of a handful of samples prove nothing.
 */
export function judge(summary: Summary, limit: Limit): string[] {
  if (summary.n < limit.minSamples) {
    return [`n ${summary.n} is below the ${limit.minSamples} samples the budget needs`];
  }
  const failures: string[] = [];
  const check = (name: string, value: number, max: number | undefined) => {
    if (max !== undefined && value > max) {
      failures.push(`${name} ${format(value)} ms is above the ${format(max)} ms limit`);
    }
  };
  check("p95", summary.p95, limit.p95Max);
  check("p99", summary.p99, limit.p99Max);
  check("max", summary.max, limit.maxMax);
  return failures;
}

/** A millisecond value with one decimal, without a trailing `.0`. */
function format(value: number): string {
  return String(Math.round(value * 10) / 10);
}
