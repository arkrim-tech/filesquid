/**
 * The budgets of the browsing performance suite (Task UI-F3.1, UI-S-035). Each value is either
 * a number NFR-002, NFR-003 or ADR 007 state (checked against the PRD text by
 * `tests/unit/perf-budgets.test.ts`) or a limit this task defines because NFR-002 and RC-002
 * require the profile to define it before acceptance (marked "defined"). The defined limits are
 * proposals for the owner to confirm; they are not claims about any packaged build.
 */

/** Sizes of the samples (NFR-002: at least 100 interactions after a declared warm-up). */
export interface SampleSizes {
  /** The fewest warm interactions NFR-002 accepts. */
  readonly minimum: number;
  /** Warm interactions measured per class (defined). */
  readonly warm: number;
  /** Interactions thrown away before measuring, to warm the JIT, styles and decoders (defined). */
  readonly warmup: number;
  /** Cold loads per catalog profile, each in a new browser context (defined). */
  readonly cold: number;
  /** Warm interactions per class under CPU pressure (defined; reported, not gated). */
  readonly pressure: number;
  /** Failing previews measured (defined). */
  readonly failure: number;
  /** Mixed interactions of the heap-growth run (defined). */
  readonly mixed: number;
}

/** The budget table. */
export const BUDGETS = {
  /** NFR-002: warm viewport interaction-to-visible-feedback p95, milliseconds. */
  warmFeedbackP95Ms: 100,
  /** Defined: tail limit for every 100 ms class, twice the budget (the usual "good" bound). */
  warmTailP99Ms: 200,
  /** NFR-002: indexed metadata search p95, milliseconds (completed first page of results). */
  searchCompletedP95Ms: 2000,
  /** NFR-002: first useful local viewport, milliseconds. */
  firstUsefulMs: 3000,
  /** NFR-002: a prepared viewer image paints the correct image at p95, milliseconds. */
  preparedImageP95Ms: 100,
  /** Defined: a 12-megapixel original with no prepared derivative, warm p95, milliseconds. */
  largeImageWarmP95Ms: 2000,
  /** Defined: the same original on the first view after load, milliseconds. */
  largeImageColdMs: 3000,
  /** Defined: a preview whose content cannot be read shows the error state within this. */
  failureVisibleMs: 2000,
  /** NFR-002: 60 Hz frame budget, milliseconds. */
  frameBudgetMs: 16.7,
  /** Defined: a frame is missed when its interval exceeds 1.5 frame budgets. */
  missedFrameAboveMs: 16.7 * 1.5,
  /** Defined: a stall is a frame longer than the feedback budget. */
  stallAboveMs: 100,
  /** Defined: most stalled frames allowed while scrolling, as a fraction of the frames. */
  stallRateMax: 0.001,
  /** Defined: most missed frames allowed while scrolling, as a fraction of the frames. */
  missedFrameRateMax: 0.05,
  /** Defined: the 99th percentile frame interval while scrolling, milliseconds. */
  frameP99Ms: 50,
  /** Defined: the idle control page may miss at most this fraction, or the environment is unusable. */
  controlMissedRateMax: 0.01,
  /** Defined: at least this many frames per layout so tail percentiles mean something. */
  minFramesPerLayout: 1500,
  /** Defined: frames at the start of a run that are discarded. */
  discardFrames: 5,
  /** Defined: a sample waits this long for its predicate, then fails the test. */
  sampleTimeoutMs: 5000,
  /** NFR-003: post-warm-up non-cache memory growth, as a fraction. */
  heapGrowthMax: 0.1,
  /** Sample sizes. */
  samples: {
    minimum: 100,
    warm: 120,
    warmup: 20,
    cold: 20,
    pressure: 100,
    failure: 20,
    mixed: 600,
  } satisfies SampleSizes,
} as const;

/** The viewport of the declared profile: the mockups' window (UI-S-024, UI-S-025). */
export const VIEWPORT = { width: 1440, height: 900 } as const;

/** Device pixel ratio of the declared profile: a 2x (Retina) display, as the macOS target has. */
export const DEVICE_SCALE_FACTOR = 2;

/** CPU slowdown of the resource-pressure case (Chromium's `Emulation.setCPUThrottlingRate`). */
export const PRESSURE_CPU_RATE = 4;

/** Navigation events per second of the background-work case. */
export const BACKGROUND_EVENTS_PER_SECOND = 5;
