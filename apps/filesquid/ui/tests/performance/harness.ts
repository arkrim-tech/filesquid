/**
 * Shared steps of the browsing performance specs (Task UI-F3.1): open the real built UI on a
 * seeded catalogue with the probe installed, the selector paths of the measured surfaces, the
 * predicates that mean "the correct rows are painted" (never a placeholder), the sample loop
 * that applies the declared warm-up and fails a sample that never completes, and the browser
 * level reads (forced garbage collection and heap, CPU throttling).
 */
import { expect, type CDPSession, type Page } from "@playwright/test";
import { BUDGETS } from "./budgets";
import { BIG_NODE, Catalogue, type ViewParams } from "./fixtures";
import { serveHost, type Host } from "./host";
import { Probe, type ArmSpec, type Pred } from "./probe";
import { trackErrors } from "../e2e/support/strict-dom";
import type { UserPreferences } from "../../src/preferences/model";

/** The drive view of the navigation route, as a probe path. */
export const VIEW = "fs-app-shell >> fs-navigation-view >> fs-drive-view";
/** The scrolling area of the list or grid. */
export const SCROLLER = `${VIEW} >> .scroller`;
/** The drive view of the search route, a direct child of the shell. */
export const RESULTS = "fs-app-shell >> fs-drive-view";
/** The drive view as a Playwright locator (which pierces open shadow roots). */
export const VIEW_LOCATOR = "fs-navigation-view fs-drive-view";

/** A page with its probe, host and error watch. */
export interface Session {
  /** The page. */
  readonly page: Page;
  /** The probe. */
  readonly probe: Probe;
  /** The fake host. */
  readonly host: Host;
  /** The catalogue the host answers from. */
  readonly catalogue: Catalogue;
  /** Console and page errors; every spec asserts it is empty at the end. */
  readonly errors: string[];
}

/** The default query of the big view: name ascending, folders on top. */
export const BIG_QUERY: ViewParams = {
  node: BIG_NODE,
  view: "all",
  sort: "name",
  direction: "ascending",
  folders: "on_top",
  filter: "",
};

/**
 * Open the real UI on a catalogue and wait until the view under test shows its first rows.
 *
 * @param page - A new page.
 * @param catalogue - The seeded catalogue.
 * @param options - `hash` to open (default the big collection), `preferences` the host holds.
 * @returns The session.
 */
export async function openView(
  page: Page,
  catalogue: Catalogue,
  options: { hash?: string; preferences?: Partial<UserPreferences>; query?: ViewParams } = {},
): Promise<Session> {
  const errors = trackErrors(page);
  const probe = await Probe.install(page);
  const host = await serveHost(page, catalogue, options.preferences);
  await page.goto(options.hash ?? `/#/navigation/${BIG_NODE}`);
  const query = options.query ?? BIG_QUERY;
  const first = catalogue.firstNames(query, 1)[0] as string;
  await expect(page.locator(VIEW_LOCATOR)).toHaveAttribute("data-state", "ready");
  if (options.preferences?.layout === "grid") {
    await expect(page.locator(`${VIEW_LOCATOR} .tile`).first()).toContainText(first);
    return { page, probe, host, catalogue, errors };
  }
  await expect(page.locator(`${VIEW_LOCATOR} .row`).first().locator(".name")).toHaveText(first);
  return { page, probe, host, catalogue, errors };
}

/**
 * The predicate "the view shows the correct first rows": ready, enough rows, none a
 * placeholder, and the first row is the one the host's order puts first.
 *
 * @param at - The view's path.
 * @param first - The display name the first row must show.
 * @param min - Fewest rows that must be in the DOM (the viewport's worth).
 * @returns The predicate.
 */
export function rowsShown(at: string, first: string, min = 10): Pred {
  return {
    k: "all",
    of: [
      { k: "attr", at, name: "data-state", value: "ready" },
      { k: "count", at: `${at} >> .row`, min },
      { k: "absent", at: `${at} >> .placeholder-row` },
      { k: "text", at: `${at} >> .row >> .name`, equals: first },
    ],
  };
}

/** What one sample loop needs. */
export interface Loop {
  /** The session. */
  readonly session: Session;
  /** Samples thrown away first. */
  readonly warmup: number;
  /** Samples kept. */
  readonly count: number;
  /** What sample `i` waits for. */
  spec(i: number): ArmSpec;
  /** Sends the real input of sample `i`. */
  trigger(i: number): Promise<void>;
  /** Puts the page into the state sample `i` starts from, unmeasured. */
  prepare?(i: number): Promise<void>;
}

/** The feedback and result times of the kept samples, in milliseconds. */
export interface LoopResult {
  /** Time to the feedback. */
  readonly ack: number[];
  /** Time to the result. */
  readonly ready: number[];
}

/**
 * Wait for two animation frames and a short pause, so the previous sample's trailing work
 * (saving a preference, a hover card) does not overlap the next sample.
 *
 * @param page - The page.
 */
export async function idle(page: Page): Promise<void> {
  await page.evaluate(
    () =>
      new Promise<void>((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(resolve, 40))),
      ),
  );
}

/**
 * Run the warm-up and then the kept samples of one class.
 *
 * @param loop - The loop.
 * @returns The feedback and result times of the kept samples.
 * @throws Error when a sample never reached its predicate within the timeout: a sample that
 *   cannot complete fails the run, it is never dropped.
 */
export async function runSamples(loop: Loop): Promise<LoopResult> {
  const ack: number[] = [];
  const ready: number[] = [];
  const { page, probe } = loop.session;
  for (let i = 0; i < loop.warmup + loop.count; i += 1) {
    await loop.prepare?.(i);
    const result = await probe.measure(loop.spec(i), () => loop.trigger(i));
    if (result.timedOut || result.readyMs === null) {
      throw new Error(
        `sample ${i} never reached its predicate within ${BUDGETS.sampleTimeoutMs} ms`,
      );
    }
    if (i >= loop.warmup) {
      ack.push(result.ackMs ?? result.readyMs);
      ready.push(result.readyMs);
    }
    await idle(page);
  }
  return { ack, ready };
}

/**
 * Open a DevTools session for browser-level reads and switches.
 *
 * @param page - The page.
 * @returns The session.
 */
export function devtools(page: Page): Promise<CDPSession> {
  return page.context().newCDPSession(page);
}

/**
 * The JS heap in use after a forced garbage collection, so growth is not noise from garbage.
 *
 * @param cdp - The DevTools session.
 * @returns Bytes.
 */
export async function heapAfterGc(cdp: CDPSession): Promise<number> {
  await cdp.send("HeapProfiler.collectGarbage");
  const usage = await cdp.send("Runtime.getHeapUsage");
  return usage.usedSize;
}

/**
 * A label template with its `{placeholders}` filled.
 *
 * @param template - The label text.
 * @param values - The values.
 * @returns The text.
 */
export function fill(template: string, values: Record<string, string | number>): string {
  return template.replace(/\{(\w+)\}/g, (whole, name: string) =>
    name in values ? String(values[name]) : whole,
  );
}
