/**
 * The warm interaction classes measured on the Drive view (Task UI-F3.1, NFR-002): selecting a
 * row, moving with the keyboard, sorting, switching the layout, opening a folder, going back
 * and narrowing with the sidebar filter. Each class is one function that runs the declared
 * warm-up and the kept samples through real input and returns the time to the feedback (the
 * control's own state change is visible) and to the result (the correct rows are painted,
 * never a placeholder), so the specs can run the same classes idle, under background work and
 * under CPU pressure.
 *
 * Every predicate names the state the sample produces and is false in the state it starts
 * from, so a sample cannot finish before the interaction has had its effect.
 */
import { BUDGETS } from "./budgets";
import { BIG_NAME, type ViewParams } from "./fixtures";
import {
  BIG_QUERY,
  SCROLLER,
  VIEW,
  VIEW_LOCATOR,
  runSamples,
  type LoopResult,
  type Session,
} from "./harness";
import type { Pred } from "./probe";
import { defaultDirection } from "../../src/drive/model";

/** Preferences that put files first in a stable, distinct order (folders mixed, newest first). */
export const FILES_FIRST = {
  sort_key: "modified",
  sort_direction: "descending",
  folder_placement: "mixed",
} as const;

/** The row at an index in the DOM, as a probe path. */
const ROW_PX = 44;

const rowAt = (index: number) => `${VIEW} >> .row[data-index='${index}']`;

/** The result of a class: its id and the feedback and result times. */
export interface ClassResult extends LoopResult {
  /** Stable id of the class. */
  readonly id: string;
}

/** How many samples a class runs. */
export interface Counts {
  /** Samples thrown away first. */
  readonly warmup: number;
  /** Samples kept. */
  readonly count: number;
}

/** The warm-up and sample size of the declared profile. */
export const WARM: Counts = { warmup: BUDGETS.samples.warmup, count: BUDGETS.samples.warm };

/** The first rows of a query, as the names the page must show. */
function names(session: Session, query: ViewParams, count = 3): string[] {
  return session.catalogue.firstNames(query, count);
}

/** The predicate: the first rows of the view are exactly these names, in order. */
export function firstRows(names_: readonly string[]): Pred {
  return {
    k: "all",
    of: [
      { k: "attr", at: VIEW, name: "data-state", value: "ready" },
      { k: "count", at: `${VIEW} >> .row`, min: 10 },
      { k: "absent", at: `${VIEW} >> .placeholder-row` },
      ...names_.map((equals, index) => ({
        k: "text" as const,
        at: `${rowAt(index)} >> .name`,
        equals,
      })),
    ],
  };
}

/**
 * Tick and untick the first rows: each sample toggles one row's checkbox.
 *
 * @param session - A session opened with {@link FILES_FIRST}.
 * @param counts - Warm-up and sample sizes.
 * @returns The feedback and result times (one measurement: the row shows its state).
 */
export async function selectClass(session: Session, counts: Counts = WARM): Promise<ClassResult> {
  const result = await runSamples({
    session,
    ...counts,
    spec: (i) => ({
      events: ["click"],
      ready: {
        k: "attr",
        at: rowAt(Math.floor(i / 2) % 8),
        name: "data-selected",
        value: i % 2 === 0 ? undefined : null,
      },
      timeoutMs: BUDGETS.sampleTimeoutMs,
    }),
    trigger: (i) => session.probe.click(`${rowAt(Math.floor(i / 2) % 8)} >> input.check`),
  });
  return { id: "select", ...result };
}

/**
 * Move the active row down and up the first nine rows with the arrow keys.
 *
 * @param session - A session opened with {@link FILES_FIRST}.
 * @param counts - Warm-up and sample sizes.
 * @returns The feedback and result times (one measurement: focus is on the next row).
 */
export async function keyboardClass(session: Session, counts: Counts = WARM): Promise<ClassResult> {
  const { page } = session;
  await page.locator(`${VIEW_LOCATOR} .row[data-index="0"] .name`).focus();
  const target = (i: number) => (i % 16 < 8 ? (i % 16) + 1 : 15 - (i % 16));
  const result = await runSamples({
    session,
    ...counts,
    spec: (i) => ({
      events: ["keydown"],
      ready: { k: "focus", at: `${rowAt(target(i))} >> .name` },
      timeoutMs: BUDGETS.sampleTimeoutMs,
    }),
    trigger: (i) => page.keyboard.press(i % 16 < 8 ? "ArrowDown" : "ArrowUp"),
  });
  return { id: "keyboard", ...result };
}

/**
 * Sort by Size, Modified and Name in turn, each a new column in its default direction.
 *
 * @param session - A session opened with {@link FILES_FIRST} (or the default preferences, with
 *   `folders` set to `on_top`).
 * @param counts - Warm-up and sample sizes.
 * @param folders - The folder placement the session's preferences give.
 * @returns The feedback (the column head carries `aria-sort`) and result (the host's new order
 *   is painted) times.
 */
export async function sortClass(
  session: Session,
  counts: Counts = WARM,
  folders: "mixed" | "on_top" = "mixed",
): Promise<ClassResult> {
  const columns = ["size", "modified", "name"] as const;
  const query = (i: number): ViewParams => {
    const sort = columns[i % 3] as (typeof columns)[number];
    return { ...BIG_QUERY, sort, direction: defaultDirection(sort), folders };
  };
  const result = await runSamples({
    session,
    ...counts,
    spec: (i) => {
      const column = columns[i % 3] as string;
      const head: Pred = { k: "attr", at: `${VIEW} >> .head >> .col-${column}`, name: "aria-sort" };
      return {
        events: ["click"],
        ack: head,
        ready: { k: "all", of: [head, firstRows(names(session, query(i)))] },
        timeoutMs: BUDGETS.sampleTimeoutMs,
      };
    },
    trigger: (i) => session.probe.click(`${VIEW} >> button.sort[data-sort='${columns[i % 3]}']`),
  });
  return { id: "sort", ...result };
}

/**
 * Switch between the grid and the list.
 *
 * @param session - A session opened with {@link FILES_FIRST}.
 * @param counts - Warm-up and sample sizes.
 * @returns The feedback (the control shows the choice) and result (the layout's items are
 *   painted) times.
 */
export async function layoutClass(session: Session, counts: Counts = WARM): Promise<ClassResult> {
  const result = await runSamples({
    session,
    ...counts,
    spec: (i) => {
      const layout = i % 2 === 0 ? "grid" : "list";
      const pressed: Pred = {
        k: "attr",
        at: `${VIEW} >> .layout >> button[data-layout='${layout}']`,
        name: "aria-pressed",
        value: "true",
      };
      const items: Pred =
        layout === "grid"
          ? {
              k: "all",
              of: [
                { k: "count", at: `${VIEW} >> .tile`, min: 8 },
                { k: "count", at: `${VIEW} >> .row`, max: 0 },
              ],
            }
          : {
              k: "all",
              of: [
                { k: "count", at: `${VIEW} >> .row`, min: 10 },
                { k: "count", at: `${VIEW} >> .tile`, max: 0 },
              ],
            };
      return {
        events: ["click"],
        ack: pressed,
        ready: {
          k: "all",
          of: [pressed, items, { k: "absent", at: `${VIEW} >> .placeholder-row` }],
        },
        timeoutMs: BUDGETS.sampleTimeoutMs,
      };
    },
    trigger: (i) =>
      session.probe.click(
        `${VIEW} >> .layout >> button[data-layout='${i % 2 === 0 ? "grid" : "list"}']`,
      ),
  });
  return { id: "layout", ...result };
}

/** The result of the folder class: opening and going back are two measurements. */
export interface FolderResult {
  /** Opening a folder: the title and the host's first rows of the folder. */
  readonly open: LoopResult;
  /** Going back to the big view through the breadcrumb. */
  readonly back: LoopResult;
  /** Distinct folders opened, so coverage can be reported. */
  readonly folders: number;
}

/**
 * Open one unseen folder after another from the big view and go back each time. Every open is a
 * cache miss: the UI keeps no view cache and each folder has its own content on the host.
 *
 * @param session - A session opened with the default preferences (folders on top).
 * @param counts - Warm-up (even) and sample sizes; `count` is the number of steps kept.
 * @param timeoutMs - How long a sample may wait for its predicate. Longer than the default only
 *   for the background-event case, where a reload of the navigation tree can queue a view
 *   request for seconds and that wait is the measurement, not a reason to stop the run.
 * @returns Both measurements.
 */
export async function folderClass(
  session: Session,
  counts: Counts = WARM,
  timeoutMs: number = BUDGETS.sampleTimeoutMs,
): Promise<FolderResult> {
  const { catalogue } = session;
  const parentNames = names(session, BIG_QUERY);
  const opened = new Set<string>();
  let current = { node: "", title: "", names: [] as string[] };
  const result = await runSamples({
    session,
    warmup: counts.warmup * 2,
    count: counts.count * 2,
    prepare: async (i) => {
      if (i % 2 !== 0) return;
      const row = i / 2;
      await session.probe.scrollTo(SCROLLER, row * ROW_PX);
      const link = await session.probe.described(`${rowAt(row)} >> a.name`, timeoutMs);
      const node = decodeURIComponent((link.href ?? "").split("/").at(-1) ?? "");
      opened.add(node);
      current = { node, title: link.text, names: catalogue.firstNames({ ...BIG_QUERY, node }, 3) };
    },
    spec: (i) => {
      const going = i % 2 === 0;
      const title = going ? current.title : BIG_NAME;
      const heading: Pred = { k: "text", at: `${VIEW} >> .page-title`, equals: title };
      return {
        events: ["click"],
        ack: heading,
        ready: { k: "all", of: [heading, firstRows(going ? current.names : parentNames)] },
        timeoutMs,
      };
    },
    trigger: (i) =>
      session.probe.click(
        i % 2 === 0
          ? `${rowAt(i / 2)} >> a.name`
          : "fs-app-shell >> fs-navigation-view >> fs-breadcrumb >> li:nth-last-child(2) a",
      ),
  });
  const every = (parity: number, values: number[]) => values.filter((_, k) => k % 2 === parity);
  return {
    open: { ack: every(0, result.ack), ready: every(0, result.ready) },
    back: { ack: every(1, result.ack), ready: every(1, result.ready) },
    folders: opened.size,
  };
}

/**
 * Narrow the view with the sidebar filter and clear it again.
 *
 * @param session - A session opened with {@link FILES_FIRST}.
 * @param terms - Distinct filter terms that each match at least one file.
 * @param counts - Warm-up and sample sizes; `count` is the number of steps kept.
 * @returns The feedback (the field shows the text) and result (the narrowed rows are painted).
 */
export async function filterClass(
  session: Session,
  terms: readonly string[],
  counts: Counts = WARM,
): Promise<ClassResult> {
  const { page } = session;
  const field = page.locator("fs-app-shell input.filter");
  const query = (term: string): ViewParams => ({
    ...BIG_QUERY,
    sort: "modified",
    direction: "descending",
    folders: "mixed",
    filter: term,
  });
  const unfiltered = names(session, query(""), 1);
  const result = await runSamples({
    session,
    ...counts,
    prepare: async () => {
      if (!(await field.evaluate((el) => el === document.activeElement))) await field.focus();
    },
    spec: (i) => {
      const term = terms[Math.floor(i / 2) % terms.length] as string;
      const expect_ = i % 2 === 0 ? names(session, query(term), 1) : unfiltered;
      return {
        events: ["input"],
        ready: {
          k: "all",
          of: [
            { k: "attr", at: VIEW, name: "data-state", value: "ready" },
            { k: "absent", at: `${VIEW} >> .placeholder-row` },
            { k: "text", at: `${rowAt(0)} >> .name`, equals: expect_[0] as string },
          ],
        },
        timeoutMs: BUDGETS.sampleTimeoutMs,
      };
    },
    trigger: async (i) => {
      const term = terms[Math.floor(i / 2) % terms.length] as string;
      if (i % 2 === 0) await page.keyboard.insertText(term);
      else {
        await page.keyboard.press("Control+a");
        await page.keyboard.press("Backspace");
      }
    },
  });
  return { id: "filter", ...result };
}
