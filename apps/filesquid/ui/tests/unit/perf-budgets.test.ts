/**
 * The budget table of the performance suite (Task UI-F3.1, UI-S-035): the values the PRD
 * defines (NFR-002, NFR-003) are checked against the text of the PRD itself so they cannot
 * drift from the requirement they cite; the limits this task defined (RC-002 asks for them) are
 * checked against their own rules (the missed-frame threshold is 1.5 frame budgets, the tail
 * limit is twice the feedback budget, a warm class has at least the 100 samples NFR-002 names)
 * and against the values UI-S-035 declares (120 warm samples, a warm-up of 20, 20 cold loads, a
 * 1% idle control, 0.1% stalls). The owner confirmed them on 2026-10-30 except the two
 * 12-megapixel original limits, which stay provisional; the last block checks that UI-S-035 and
 * `PROVISIONAL_LIMITS` say the same.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { BUDGETS, PROVISIONAL_LIMITS } from "../performance/budgets";
import { PROFILES } from "../performance/fixtures";

const PRD = readFileSync(resolve(process.cwd(), "../../../resources/prd.md"), "utf8");
const UI_SRS = readFileSync(resolve(process.cwd(), "resources/srs.md"), "utf8");

/** The UI-S-035 row of the UI SRS: one table line. */
const UI_S_035 = UI_SRS.split("\n").find((line) => line.startsWith("| UI-S-035 ")) ?? "";

/** The text of one PRD requirement section, from its heading to the next one. */
function section(id: string): string {
  const start = PRD.indexOf(`## ${id}:`);
  const end = PRD.indexOf("\n## ", start + 1);
  return PRD.slice(start, end);
}

const NFR_002 = section("NFR-002");
const NFR_003 = section("NFR-003");

describe("the budgets NFR-002 states", () => {
  it("warm interaction-to-visible-feedback p95 is 100 ms", () => {
    expect(NFR_002).toContain("warm viewport interaction-to-visible-feedback p95 MUST be ≤100 ms");
    expect(BUDGETS.warmFeedbackP95Ms).toBe(100);
  });

  it("indexed metadata search p95 is 2 s", () => {
    expect(NFR_002).toContain("indexed metadata search p95 ≤2 s");
    expect(BUDGETS.searchCompletedP95Ms).toBe(2000);
  });

  it("the first useful local viewport is 3 s", () => {
    expect(NFR_002).toContain("first useful local viewport ≤3 s");
    expect(BUDGETS.firstUsefulMs).toBe(3000);
  });

  it("a prepared viewer image paints at p95 100 ms", () => {
    expect(NFR_002).toContain("paint the correct image at p95 ≤100 ms");
    expect(BUDGETS.preparedImageP95Ms).toBe(100);
  });

  it("the frame budget at 60 Hz is 16.7 ms", () => {
    expect(NFR_002).toContain("16.7 ms frame budget");
    expect(BUDGETS.frameBudgetMs).toBe(16.7);
  });

  it("at least 100 interactions are measured after a declared warm-up", () => {
    expect(NFR_002).toContain("Measure at least 100 queries/interactions after declared warm-up");
    expect(BUDGETS.samples.minimum).toBe(100);
    expect(BUDGETS.samples.warm).toBeGreaterThanOrEqual(BUDGETS.samples.minimum);
    expect(BUDGETS.samples.pressure).toBeGreaterThanOrEqual(BUDGETS.samples.minimum);
    expect(BUDGETS.samples.warmup).toBeGreaterThan(0);
  });
});

describe("the sample sizes UI-S-035 declares", () => {
  it("120 warm samples after a warm-up of 20, 20 cold loads, 100 under pressure, 20 failures", () => {
    expect(BUDGETS.samples).toMatchObject({
      warm: 120,
      warmup: 20,
      cold: 20,
      pressure: 100,
      failure: 20,
    });
  });

  it("600 mixed interactions run for the heap (NFR-003)", () => {
    expect(BUDGETS.samples.mixed).toBe(600);
  });
});

describe("the catalog profiles NFR-002 states", () => {
  it("Small, Medium and Large hold 10,000, 100,000 and 1,000,000 assets", () => {
    expect(NFR_002).toContain("Small: 10,000 assets/20,000 resources/100 GB/100 albums");
    expect(NFR_002).toContain("Medium: 100,000/200,000/1 TB/5,000 albums");
    expect(NFR_002).toContain(
      "Large: 1,000,000/2,000,000/10 TB/50,000 albums/10,000,000 memberships",
    );
    expect(PROFILES.small).toMatchObject({ assets: 10_000, resources: 20_000, albums: 100 });
    expect(PROFILES.medium).toMatchObject({ assets: 100_000, resources: 200_000, albums: 5_000 });
    expect(PROFILES.large).toMatchObject({
      assets: 1_000_000,
      resources: 2_000_000,
      albums: 50_000,
      memberships: 10_000_000,
    });
  });
});

describe("the NFR-003 clauses that bind browsing", () => {
  it("post-warm-up non-cache memory growth is at most 10%", () => {
    expect(NFR_003).toContain("post-warm-up non-cache memory growth MUST be ≤10%");
    expect(BUDGETS.heapGrowthMax).toBe(0.1);
  });
});

describe("the limits this task defines (UI-S-035)", () => {
  it("a missed frame is more than 1.5 frame budgets", () => {
    expect(BUDGETS.missedFrameAboveMs).toBeCloseTo(BUDGETS.frameBudgetMs * 1.5, 10);
  });

  it("the tail limit is twice the feedback budget", () => {
    expect(BUDGETS.warmTailP99Ms).toBe(BUDGETS.warmFeedbackP95Ms * 2);
  });

  it("a stall is a frame that took longer than the feedback budget", () => {
    expect(BUDGETS.stallAboveMs).toBe(BUDGETS.warmFeedbackP95Ms);
  });

  it("stalls are limited by rate: at most one frame in a thousand", () => {
    // The Large catalog's scroll shows one V8 major-GC finalisation stall of 117 to 167 ms in
    // about 1,900 frames (Task UI-F3.1, Finding 5); a rate keeps the gate while it is reported.
    expect(BUDGETS.stallRateMax).toBe(0.001);
  });

  it("the frame limits are set and the control is stricter than the app", () => {
    expect(BUDGETS.missedFrameRateMax).toBe(0.05);
    expect(BUDGETS.frameP99Ms).toBe(50);
    expect(BUDGETS.controlMissedRateMax).toBe(0.01);
    expect(BUDGETS.controlMissedRateMax).toBeLessThan(BUDGETS.missedFrameRateMax);
    expect(BUDGETS.minFramesPerLayout).toBeGreaterThanOrEqual(1500);
    expect(BUDGETS.discardFrames).toBe(5);
  });

  it("every sample waits at most 5 s for its predicate", () => {
    expect(BUDGETS.sampleTimeoutMs).toBe(5000);
  });

  it("a large original and a failure have their own limits", () => {
    expect(BUDGETS.largeImageWarmP95Ms).toBe(2000);
    expect(BUDGETS.largeImageColdMs).toBe(3000);
    expect(BUDGETS.failureVisibleMs).toBe(2000);
  });

  it("the cold loads and the mixed run are sized", () => {
    expect(BUDGETS.samples.cold).toBe(20);
    expect(BUDGETS.samples.failure).toBe(20);
    expect(BUDGETS.samples.mixed).toBe(600);
  });
});

describe("the owner's confirmation of the limits (UI-S-035, 2026-10-30)", () => {
  it("UI-S-035 records the limits as confirmed, not proposed", () => {
    expect(UI_S_035).toContain("the owner confirmed them on 2026-10-30");
    expect(UI_S_035).not.toContain("proposed, the owner confirms");
  });

  it("exactly the two 12-megapixel original limits are provisional, in code and in UI-S-035", () => {
    expect([...PROVISIONAL_LIMITS]).toEqual(["largeImageWarmP95Ms", "largeImageColdMs"]);
    expect(UI_S_035).toContain(
      "except the two 12-megapixel original limits, which are provisional: no PRD requirement states them",
    );
    expect(UI_S_035).toContain(
      `p95 ≤ ${BUDGETS.largeImageWarmP95Ms.toLocaleString("en-US")} ms warm and ≤ ${BUDGETS.largeImageColdMs.toLocaleString("en-US")} ms cold (both provisional)`,
    );
  });
});
