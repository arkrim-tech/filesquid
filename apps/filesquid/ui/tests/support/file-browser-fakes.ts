/**
 * Test helpers for `<fs-file-browser>` (UI-F2.2): a fixture object with stored names that differ
 * from display names, a mount function, and readers over the browser's shadow DOM, including a
 * deep scan of every shadow root so a stored name that leaks anywhere is caught (UI-S-019).
 */
import { defineElements, type FsFileBrowser } from "../../src/index";
import { MemoryFileSource, type MemoryFile } from "../../src/files/memory-source";
import type { FileLocation } from "../../src/files/model";
import labels from "../../src/theme/labels.en.json";

/** The library and object of the fixture. */
export const LOCATION: FileLocation = { library_id: "lib", object_id: "obj", prefix: "" };

/** Stored names that must never appear in the page. */
export const STORED_NAMES = [
  "3f9a0001",
  "3f9a0002",
  "3f9a0003",
  "3f9a0004",
  "3f9a0005",
  "3f9a0006",
] as const;

/** The fixture files: folders `Contracts` and `Drawings`, and three top-level files. */
export const FILES: readonly MemoryFile[] = [
  { path: "Contracts/3f9a0001.pdf", displayName: "harbour-masterplan-v7.pdf", content: "%PDF" },
  { path: "Contracts/3f9a0002.pdf", displayName: "tender-response.pdf", content: "%PDF" },
  { path: "Drawings/3f9a0003.txt", displayName: "notes.txt", content: "drawing notes" },
  { path: "3f9a0004.png", displayName: "facade-study.png", content: "png" },
  { path: "3f9a0005.txt", displayName: "readme.txt", content: "hello world" },
  { path: "3f9a0006.blend", displayName: "model-export.blend", content: "blend" },
];

/** A source over the fixture, or over `files`. */
export function fixture(files: readonly MemoryFile[] = FILES): MemoryFileSource {
  return new MemoryFileSource({ library_id: "lib", object_id: "obj", files });
}

/** Let queued promise callbacks and timers run. */
export const settle = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

/**
 * Wait until the browser has no write in progress (its frame stops being `aria-busy`), so a
 * test can assert on the result of an upload, which reads files through the event loop.
 */
export async function idle(browser: FsFileBrowser): Promise<void> {
  const frame = browser.shadowRoot?.querySelector(".frame");
  for (let i = 0; i < 200; i += 1) {
    await settle();
    if (!frame?.hasAttribute("aria-busy")) break;
  }
  await settle();
}

/** Options a test may pass when mounting. */
export interface MountOptions {
  /** Where to start; the root by default. */
  location?: FileLocation;
  /** Listing page size. */
  pageSize?: number;
  /** Most pages read. */
  maxPages?: number;
}

/**
 * Mount a browser over `source` at `location` and wait for the first load.
 *
 * @param source The file source.
 * @param options Start location and paging bounds.
 * @returns The mounted element.
 */
export async function mount(
  source: MemoryFileSource,
  options: MountOptions = {},
): Promise<FsFileBrowser> {
  defineElements();
  const browser = document.createElement("fs-file-browser") as FsFileBrowser;
  browser.configure({
    labels,
    source,
    pageSize: options.pageSize,
    maxPages: options.maxPages,
  });
  document.body.replaceChildren(browser);
  browser.location = options.location ?? LOCATION;
  await settle();
  return browser;
}

/** The browser's shadow root. */
export function rootOf(browser: FsFileBrowser): ShadowRoot {
  return browser.shadowRoot as ShadowRoot;
}

/** Elements matching `selector` in the browser's shadow root. */
export function all<T extends Element = HTMLElement>(
  browser: FsFileBrowser,
  selector: string,
): T[] {
  return [...rootOf(browser).querySelectorAll<T>(selector)];
}

/** The first element matching `selector`; throws when there is none, so a typo fails loudly. */
export function one<T extends Element = HTMLElement>(browser: FsFileBrowser, selector: string): T {
  const found = rootOf(browser).querySelector<T>(selector);
  if (!found) throw new Error(`no element matches ${selector}`);
  return found;
}

/** The visible names of the rows or tiles, in order. */
export function names(browser: FsFileBrowser): string[] {
  return all(browser, ".row .name, .tile .name").map((el) => el.textContent?.trim() ?? "");
}

/** The name control of the row or tile whose name is `name`. */
export function control(browser: FsFileBrowser, name: string): HTMLElement {
  const found = all(browser, ".row .name, .tile .name").find(
    (el) => el.textContent?.trim() === name,
  );
  if (!found) throw new Error(`no row named ${name}`);
  return found;
}

/** Click an element and let the work it starts finish. */
export async function click(element: Element): Promise<void> {
  (element as HTMLElement).click();
  await settle();
}

/**
 * Every text and attribute value in the browser and in every shadow root below it, as one
 * string, for "this never appears anywhere" checks.
 */
export function deepText(root: ParentNode): string {
  const parts: string[] = [];
  const stack: ParentNode[] = [root];
  for (let guard = 0; guard < 100_000 && stack.length > 0; guard += 1) {
    const node = stack.pop() as ParentNode;
    for (const el of node.querySelectorAll("*")) {
      parts.push(el.textContent ?? "", ...[...el.attributes].map((a) => `${a.name}=${a.value}`));
      if (el.shadowRoot) stack.push(el.shadowRoot);
    }
  }
  return parts.join("\n");
}
