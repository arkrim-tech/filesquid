/**
 * Test helpers for `<fs-drive-view>` (UI-F2.8): a fixture view of one library node whose rows
 * carry display names apart from their file ids, a fake file source for the writes, a mount
 * function with a fixed clock, and readers over the view's shadow DOM, including a deep scan so
 * a name or path that leaks into an attribute is caught (UI-S-019).
 */
import { vi } from "vitest";
import { defineElements } from "../../src/index";
import type { FsDriveView } from "../../src/components/fs-drive-view";
import type { DriveRow } from "../../src/drive/model";
import { MemoryViewSource } from "../../src/drive/memory-source";
import type { NodeViews } from "../../src/drive/memory-source";
import { DEFAULT_PREFERENCES, type UserPreferences } from "../../src/preferences/model";
import { MemoryPreferencesSource } from "../../src/preferences/source";
import { PreferencesService } from "../../src/preferences/service";
import { NavigationStore } from "../../src/navigation/store";
import type { FileSource } from "../../src/files/source";
import labels from "../../src/theme/labels.en.json";

/** The library node and the library id of the fixture. */
export const NODE = "node-library";
export const LIBRARY = "lib-1";
/** The clock of the fixture: every relative date is measured from here. */
export const NOW = new Date("2026-10-16T12:00:00Z");

/** A file row. */
function file(
  id: string,
  name: string,
  size: number,
  modified: string,
  extra: Partial<DriveRow> = {},
): DriveRow {
  return {
    key: id,
    kind: "file",
    name,
    fileId: id,
    objectId: `urn:uuid:${id}`,
    mediaType: "application/octet-stream",
    size,
    modified,
    version: "v2",
    ...extra,
  };
}

/** A folder row. */
function folder(node: string, name: string, count: number): DriveRow {
  return { key: node, kind: "folder", name, node, nodeKind: "collection", fileCount: count };
}

/** The rows of the fixture library: two collections and four files. */
export const ROWS: readonly DriveRow[] = [
  folder("c-contracts", "Contracts", 14),
  folder("c-drawings", "Drawings", 62),
  file("f-pdf", "harbour-masterplan-v7.pdf", 19_293_798, "2026-10-16T10:00:00Z", {
    openedByMe: "2026-10-16T09:00:00Z",
  }),
  file("f-xlsx", "budget-2026.xlsx", 421_888, "2026-10-15T12:00:00Z", {
    modifiedByMe: "2026-10-15T12:00:00Z",
  }),
  file("f-jpg", "facade-study.jpg", 6_500_000, "2026-10-15T08:00:00Z"),
  file("f-csv", "survey-points.csv", 90_112, "2026-10-13T08:00:00Z"),
];

/** The stored name that must never appear in the page. */
export const STORED_NAME = "3f9a0001.pdf";

/** The fixture's source: the library's `all` view. */
export function viewSource(rows: readonly DriveRow[] = ROWS): MemoryViewSource {
  const views: NodeViews = { all: rows, deleted: [] };
  return new MemoryViewSource(new Map([[NODE, views]]), LIBRARY);
}

/** A source of preferences and the service over it, in "api" mode. */
export async function preferences(
  overrides: Partial<UserPreferences> = {},
): Promise<{ service: PreferencesService; source: MemoryPreferencesSource }> {
  const source = new MemoryPreferencesSource({ ...DEFAULT_PREFERENCES, ...overrides });
  const navigation = new NavigationStore({});
  const service = new PreferencesService({ source, navigation });
  await service.start();
  return { service, source };
}

/** A fake of the file writes the view uses; every method is a spy. */
export function fakeFiles(): Pick<FileSource, "list" | "rename" | "remove" | "restore"> {
  return {
    list: vi.fn(async () => ({
      library_id: LIBRARY,
      object_id: "urn:uuid:f-pdf",
      version: "v2",
      prefix: "original",
      entries: [
        {
          kind: "file" as const,
          name: STORED_NAME,
          path: `original/${STORED_NAME}`,
          display_name: "harbour-masterplan-v7.pdf",
          size: 1,
          type_hint: "pdf",
          modified: "2026-10-16T10:00:00Z",
          modified_version: "v2",
        },
      ],
      next_cursor: null,
    })),
    rename: vi.fn(async () => ({ version: "v3" }) as never),
    remove: vi.fn(async () => ({
      library_id: LIBRARY,
      object_id: "urn:uuid:f-pdf",
      version: "v3",
      created: "2026-10-16T12:00:00Z",
    })),
    restore: vi.fn(async () => ({
      library_id: LIBRARY,
      object_id: "urn:uuid:f-pdf",
      version: "v4",
      created: "2026-10-16T12:00:00Z",
    })),
  } as never;
}

/** Let queued promise callbacks and timers run. */
export const settle = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

/** What a test may change when mounting. */
export interface MountOptions {
  /** The rows of the library node. */
  rows?: readonly DriveRow[];
  /** The view asked for. */
  view?: "all" | "recent";
  /** The preferences saved already. */
  prefs?: Partial<UserPreferences>;
  /** A source to use instead of the fixture's. */
  source?: MemoryViewSource;
}

/** The mounted element with what a test needs to steer and inspect it. */
export interface Mounted {
  readonly element: FsDriveView;
  readonly source: MemoryViewSource;
  readonly files: ReturnType<typeof fakeFiles>;
  readonly prefs: PreferencesService;
  readonly saved: MemoryPreferencesSource;
}

/**
 * Mount a view over the fixture and wait for the first load.
 *
 * @param options Rows, view and saved preferences.
 * @returns The element and its fakes.
 */
export async function mount(options: MountOptions = {}): Promise<Mounted> {
  defineElements();
  const source = options.source ?? viewSource(options.rows);
  const files = fakeFiles();
  const { service, source: saved } = await preferences(options.prefs);
  const element = document.createElement("fs-drive-view") as unknown as FsDriveView;
  element.configure({
    labels,
    source,
    files,
    preferences: service,
    now: () => NOW,
    online: () => true,
  });
  document.body.replaceChildren(element);
  element.heading = "Harbour project";
  element.view = options.view ?? "all";
  element.node = NODE;
  await settle();
  return { element, source, files, prefs: service, saved };
}

/** The view's shadow root. */
export function rootOf(element: Element): ShadowRoot {
  return element.shadowRoot as ShadowRoot;
}

/** Elements matching `selector` in the view's shadow root. */
export function all<T extends Element = HTMLElement>(element: Element, selector: string): T[] {
  return [...rootOf(element).querySelectorAll<T>(selector)];
}

/** The first element matching `selector`; throws when there is none, so a typo fails loudly. */
export function one<T extends Element = HTMLElement>(element: Element, selector: string): T {
  const found = rootOf(element).querySelector<T>(selector);
  if (!found) throw new Error(`no element matches ${selector}`);
  return found;
}

/** The visible names of the rows or tiles, in order. */
export function names(element: Element): string[] {
  return all(element, ".row .name, .tile .name").map((el) => el.textContent?.trim() ?? "");
}

/** The row or tile named `name`. */
export function rowNamed(element: Element, name: string): HTMLElement {
  const found = all(element, ".row, .tile").find(
    (el) => el.querySelector(".name")?.textContent?.trim() === name,
  );
  if (!found) throw new Error(`no row named ${name}`);
  return found;
}

/** Click an element and let the work it starts finish. */
export async function click(element: Element): Promise<void> {
  (element as HTMLElement).click();
  await settle();
}

/** Every text and attribute value below `root`, shadow roots included, as one string. */
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
