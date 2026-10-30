/**
 * Test helpers for the search bar and the search results (UI-F2.9): a fixture library whose
 * files carry display names and collections as mockup 20 draws them, a recording in-memory search
 * source, a small navigation tree, and mount functions with a fixed clock, plus readers over the
 * elements' shadow DOM.
 */
import { vi } from "vitest";
import { defineElements } from "../../src/index";
import type { FsSearchBar } from "../../src/components/fs-search-bar";
import type { DriveRow } from "../../src/drive/model";
import { MemoryNavigationSource, type FixtureNode } from "../../src/navigation/source";
import { MemorySearchSource } from "../../src/search/memory-source";
import type { SearchSource } from "../../src/search/source";
import labels from "../../src/theme/labels.en.json";

/** The library the search runs in and its name, as the scope chip words it. */
export const SCOPE = { node: "node-library", name: "Family archive" } as const;

/** The clock of the fixture: every relative time is measured from here. */
export const NOW = new Date("2026-10-16T12:00:00Z");

/** One file of the fixture library. */
function file(
  id: string,
  name: string,
  collection: string,
  modified: string,
  size = 1000,
): DriveRow {
  const type = name.endsWith(".pdf") ? "application/pdf" : "image/jpeg";
  return {
    key: id,
    kind: "file",
    name,
    fileId: id,
    objectId: `urn:uuid:${id}`,
    libraryId: "lib-1",
    mediaType: type,
    size,
    modified,
    collection,
  };
}

/** The files of the fixture library, as mockup 20's FILES section lists them. */
export const FILES: readonly DriveRow[] = [
  file("f1", "harbour-masterplan-v7.pdf", "Harbour project", "2026-10-16T10:00:00Z", 19_293_798),
  file("f2", "harbour-2025-report.pdf", "Harbour project", "2026-09-02T10:00:00Z", 2_000_000),
  file("f3", "harbour-walk-permit.pdf", "Documents", "2026-09-20T10:00:00Z", 100_000),
  file("f4", "facade-study.jpg", "Photos 2026", "2026-10-15T08:00:00Z", 6_500_000),
];

/** The repository, one library and its collections. */
export const TREE: FixtureNode = {
  node: "root",
  kind: "repository",
  name: null,
  children: [
    {
      node: SCOPE.node,
      kind: "library",
      name: SCOPE.name,
      children: [
        { node: "c-docs", kind: "collection", name: "Documents" },
        { node: "c-harbour", kind: "collection", name: "Harbour project" },
        { node: "c-photos", kind: "collection", name: "Photos 2026" },
        { node: "v-best", kind: "virtual_collection", name: "Best of 2026" },
      ],
    },
  ],
};

/** A search source over the fixture; every call is recorded. */
export function searchSource(rows: readonly DriveRow[] = FILES): MemorySearchSource {
  return new MemorySearchSource(new Map([[SCOPE.node, rows]]), { libraryId: "lib-1" });
}

/** Let queued promise callbacks and timers run. */
export const settle = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

/** What a test may change when mounting the bar. */
export interface BarOptions {
  /** The search source; the fixture's by default. */
  search?: SearchSource;
  /** Milliseconds the live suggestions wait after typing; 0 by default. */
  debounceMs?: number;
}

/** The mounted bar with what a test needs to steer and inspect it. */
export interface MountedBar {
  readonly element: FsSearchBar;
  readonly search: MemorySearchSource;
  readonly navigation: MemoryNavigationSource;
  readonly searched: ReturnType<typeof vi.fn>;
}

/**
 * Mount a bar over the fixture, with a spy on its `fs-search` event.
 *
 * @param options The search source and the debounce.
 * @returns The element and its fakes.
 */
export function mountBar(options: BarOptions = {}): MountedBar {
  defineElements();
  const search = searchSource();
  const navigation = new MemoryNavigationSource(TREE);
  const element = document.createElement("fs-search-bar") as FsSearchBar;
  element.configure({
    labels,
    search: options.search ?? search,
    navigation,
    scope: async () => SCOPE,
    now: () => NOW,
    debounceMs: options.debounceMs ?? 0,
  });
  document.body.replaceChildren(element);
  const searched = vi.fn();
  element.addEventListener("fs-search", (event) => searched((event as CustomEvent).detail));
  return { element, search, navigation, searched };
}

/** The element's shadow root. */
export function rootOf(element: Element): ShadowRoot {
  return element.shadowRoot as ShadowRoot;
}

/** Elements matching `selector` in the shadow root. */
export function all<T extends Element = HTMLElement>(element: Element, selector: string): T[] {
  return [...rootOf(element).querySelectorAll<T>(selector)];
}

/** The first element matching `selector`; throws when there is none, so a typo fails loudly. */
export function one<T extends Element = HTMLElement>(element: Element, selector: string): T {
  const found = rootOf(element).querySelector<T>(selector);
  if (!found) throw new Error(`no element matches ${selector}`);
  return found;
}

/** The text of every element matching `selector`, trimmed. */
export function texts(element: Element, selector: string): string[] {
  return all(element, selector).map((el) => el.textContent?.trim() ?? "");
}

/** Click an element and let the work it starts finish. */
export async function click(element: Element): Promise<void> {
  (element as HTMLElement).click();
  await settle();
}

/** Focus the field and let the drop-down open. */
export async function focusField(element: Element): Promise<HTMLInputElement> {
  const input = one<HTMLInputElement>(element, "input.text");
  input.focus();
  await settle();
  return input;
}

/** Type `text` into the field as a user does, then let the live suggestions answer. */
export async function type(element: Element, text: string): Promise<HTMLInputElement> {
  const input = await focusField(element);
  input.value = text;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  await settle();
  await settle();
  return input;
}

/** Press a key in the field. */
export async function press(element: Element, key: string): Promise<void> {
  const input = one<HTMLInputElement>(element, "input.text");
  input.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
  await settle();
}

/** The label table's word for a key, so no English is typed in a test. */
export function label(key: keyof typeof labels): string {
  return labels[key];
}
