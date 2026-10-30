/**
 * The Node side of the browsing performance probe (Task UI-F3.1, NFR-002 clock boundaries). The
 * page side is `probe-page.js`, injected before any script of the app runs; it adds no behaviour
 * to the app, it only watches. For one armed sample it takes the start from the first trusted
 * input event (its `timeStamp`, or a fixed start for a cold load), polls the sample's
 * predicates once per animation frame and ends at the start of the first frame after the one in
 * which a predicate first held, an upper bound of the moment the change was presented.
 * Predicates are plain data, so the page needs no `eval`.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import type { Page } from "@playwright/test";

/** A condition on the page, as data. `at` is a path of selectors joined by ` >> `. */
export type Pred =
  | { k: "attr"; at: string; name: string; value?: string | null }
  | { k: "text"; at: string; equals?: string; includes?: string }
  | { k: "count"; at: string; min?: number; max?: number }
  | { k: "absent"; at: string }
  | { k: "img"; at: string; alt: string }
  | { k: "value"; at: string; equals: string }
  | { k: "focus"; at: string }
  | { k: "all"; of: Pred[] };

/** What one sample waits for. */
export interface ArmSpec {
  /** Input event types whose first trusted occurrence is the start (empty with `startAt`). */
  events: string[];
  /** The feedback predicate, when the sample measures the acknowledgment separately. */
  ack?: Pred;
  /** The result predicate. */
  ready: Pred;
  /** Give up after this many milliseconds from arming. */
  timeoutMs: number;
  /** A fixed start on the page's clock (a cold load starts at 0). */
  startAt?: number;
}

/** What a sample measured; times are milliseconds from the start. */
export interface SampleResult {
  /** When the feedback was visible; equals `readyMs` when no separate feedback was asked. */
  ackMs: number | null;
  /** When the result was visible. */
  readyMs: number | null;
  /** True when the predicate never held within the timeout. */
  timedOut: boolean;
  /** The milestones of a cold load, from the navigation and resource timing entries. */
  milestones?: Record<string, number>;
}

/** What a scripted scroll reports. */
export interface ScrollRun {
  /** Milliseconds between consecutive animation frames during the run. */
  intervals: number[];
  /** Rows or tiles in the DOM at the end of the run. */
  domRows: number;
  /** How far the area had scrolled at the highest point, in pixels. */
  maxScrollTop: number;
  /** The height of the scrollable content at the end of the run. */
  scrollHeight: number;
}

/** The heights of a scrolling area. */
export interface AreaHeights {
  /** The visible height in pixels. */
  clientHeight: number;
  /** The content height in pixels. */
  scrollHeight: number;
}

/** The interface the page's `window.__fsProbe` offers (`probe-page.js`). */
interface PageProbe {
  arm(spec: ArmSpec): void;
  result(): Promise<SampleResult>;
  framesStart(): void;
  framesStop(): number[];
  scrollRun(
    path: string,
    pxPerSecond: number,
    durationMs: number,
    reverse: boolean,
  ): Promise<ScrollRun>;
  longTasks(): { start: number; duration: number }[];
  domCount(): number;
  rowCount(at: string): number;
  scrollToEnd(path: string): AreaHeights;
  scrollTo(path: string, top: number): void;
  centre(path: string): { x: number; y: number };
  describe(path: string): { href: string | null; text: string } | null;
}

/** The link target and text of an element. */
export interface Described {
  /** The `href` attribute, or null. */
  href: string | null;
  /** The trimmed text. */
  text: string;
}

declare global {
  interface Window {
    __fsProbe?: PageProbe;
    __fsProbeArm?: ArmSpec;
  }
}

/** The page side of the probe, injected into every page before the app runs. */
const PAGE_SCRIPT = resolve(process.cwd(), "tests/performance/probe-page.js");

/** The Node side of the probe for one page. */
export class Probe {
  /**
   * @param page - The page the probe was installed in (before it navigated).
   */
  constructor(private readonly page: Page) {}

  /**
   * Install the probe before the app's scripts run.
   *
   * @param page - The page, not yet navigated.
   * @param armAtStart - A sample that starts at navigation start (a cold load), or null.
   * @returns The Node side.
   */
  static async install(page: Page, armAtStart: ArmSpec | null = null): Promise<Probe> {
    // One init script, not two: Playwright does not define the order of several, and the probe
    // must find the spec that arms a cold load when it starts.
    const arm = armAtStart ? `window.__fsProbeArm = ${JSON.stringify(armAtStart)};\n` : "";
    await page.addInitScript({ content: arm + readFileSync(PAGE_SCRIPT, "utf8") });
    return new Probe(page);
  }

  /**
   * Measure one sample: arm, run the real input, wait for the predicates.
   *
   * @param spec - What to wait for.
   * @param trigger - Sends the real input through Playwright.
   * @returns The measured times; `timedOut` when the predicate never held.
   */
  async measure(spec: ArmSpec, trigger: () => Promise<void>): Promise<SampleResult> {
    await this.page.evaluate((armed) => window.__fsProbe?.arm(armed), spec);
    await trigger();
    return this.result();
  }

  /**
   * The result of the armed sample (also of a probe armed at the start of a cold load).
   *
   * @returns The measured times.
   */
  result(): Promise<SampleResult> {
    return this.page.evaluate(() => window.__fsProbe?.result() as Promise<SampleResult>);
  }

  /** Start recording animation frame timestamps. */
  async framesStart(): Promise<void> {
    await this.page.evaluate(() => window.__fsProbe?.framesStart());
  }

  /**
   * Stop recording and return the intervals between frames.
   *
   * @returns Milliseconds between consecutive frames.
   */
  framesStop(): Promise<number[]> {
    return this.page.evaluate(() => window.__fsProbe?.framesStop() as number[]);
  }

  /**
   * Scroll an area at a constant speed from inside the page, one step per animation frame.
   *
   * @param path - The scrolling area, as a selector path.
   * @param pxPerSecond - Speed.
   * @param durationMs - Length of the run.
   * @param reverse - Scroll back up for the second half (a direction reversal).
   * @returns The frame intervals and the area's state at the end.
   */
  scrollRun(
    path: string,
    pxPerSecond: number,
    durationMs: number,
    reverse: boolean,
  ): Promise<ScrollRun> {
    return this.page.evaluate(
      (run) =>
        window.__fsProbe?.scrollRun(
          run.path,
          run.speed,
          run.length,
          run.back,
        ) as Promise<ScrollRun>,
      { path, speed: pxPerSecond, length: durationMs, back: reverse },
    );
  }

  /**
   * Long tasks since the last call.
   *
   * @returns Start and duration of each, in milliseconds.
   */
  longTasks(): Promise<{ start: number; duration: number }[]> {
    return this.page.evaluate(() => window.__fsProbe?.longTasks() ?? []);
  }

  /**
   * Elements in the page, through open shadow roots.
   *
   * @returns The count.
   */
  domCount(): Promise<number> {
    return this.page.evaluate(() => window.__fsProbe?.domCount() ?? 0);
  }

  /**
   * Elements matching a selector path.
   *
   * @param at - The path.
   * @returns The count.
   */
  rowCount(at: string): Promise<number> {
    return this.page.evaluate((path) => window.__fsProbe?.rowCount(path) ?? 0, at);
  }

  /**
   * Click the centre of the element at a path with the browser's real mouse input. The element
   * is found by the probe's own path walk, not by Playwright's selector engine, which walks
   * every element of a large page (about 70 ms on the Large catalog) and installs a hit-target
   * check around the click: neither belongs in an interaction measurement.
   *
   * @param at - The element, as a selector path.
   * @throws Error when nothing is at the path.
   */
  async click(at: string): Promise<void> {
    const point = await this.page.evaluate((path) => window.__fsProbe?.centre(path), at);
    if (!point) throw new Error(`the probe is not installed, so ${at} cannot be clicked`);
    await this.page.mouse.click(point.x, point.y);
  }

  /**
   * Wait until an element is in the page, then read its link target and text. Bounded: it looks
   * every 20 ms for at most `timeoutMs`.
   *
   * @param at - The element, as a selector path.
   * @param timeoutMs - How long to look.
   * @returns The element's `href` and text.
   * @throws Error when nothing appeared in time.
   */
  async described(at: string, timeoutMs: number): Promise<Described> {
    const deadline = Date.now() + timeoutMs;
    let left = timeoutMs;
    while (left > 0) {
      const found = await this.page.evaluate(
        (path) => window.__fsProbe?.describe(path) ?? null,
        at,
      );
      if (found) return found;
      left = deadline - Date.now();
      // The last sleep is cut to the time left, so the wait never runs past the timeout.
      if (left > 0) await this.page.waitForTimeout(Math.min(20, left));
    }
    throw new Error(`nothing appeared at ${at} within ${timeoutMs} ms`);
  }

  /**
   * Scroll an area to a position.
   *
   * @param at - The scrolling area, as a selector path.
   * @param top - Pixels from the top.
   */
  async scrollTo(at: string, top: number): Promise<void> {
    await this.page.evaluate((target) => window.__fsProbe?.scrollTo(target.at, target.top), {
      at,
      top,
    });
  }

  /**
   * Scroll an area to its loaded end.
   *
   * @param at - The scrolling area, as a selector path.
   * @returns Its visible and its content height in pixels, after the scroll.
   */
  scrollToEnd(at: string): Promise<AreaHeights> {
    return this.page.evaluate((path) => window.__fsProbe?.scrollToEnd(path) as AreaHeights, at);
  }
}
