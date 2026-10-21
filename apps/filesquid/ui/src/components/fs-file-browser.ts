/**
 * `<fs-file-browser>`: the Drive-style file browser of the FileSquid default UI (UI-F2.2,
 * UI-S-002). It lists one folder of one OCFL object in a list (default) or grid layout with
 * name, size, type hint, modified and version, opens folders from the listing and the
 * breadcrumb, shows a file in a viewer with a preview, and uploads, creates folders, renames or
 * moves, removes and restores, each a single API call that commits one OCFL version.
 *
 * Every name shown is the API's display name (UI-S-019); the stored name never reaches the DOM.
 * The browser reads and writes only through the {@link FileSource} it is given (UI-S-012), so
 * it never touches a module, a path or a database (ADR 030).
 *
 * **Slots (ADR 028 Decision 6).** `registerPanel(id, create)` mounts a panel element in the
 * sidebar; each panel receives the opened file as its `fileRef` property (a
 * {@link FileRef}, or null when none is open) and no reference to any other panel, so panels
 * stay independent. `registerPreviewRenderer(renderer)` adds a preview renderer that is tried
 * before the built-in ones.
 *
 * **Events** (bubbling and composed): `fs-location-change` (`detail.location`) when a folder is
 * entered, `fs-file-open` (`detail.file`, or null when the viewer closes) and `fs-files-change`
 * (`detail.operation`, `detail.commit`) after a write commits.
 *
 * Accessibility (UI-S-016): see {@link FileList} for the listing's keys and roving Tab stop;
 * the shared stylesheet comes first in the shadow root; focus moves only after a user action
 * (opening a file moves it to the viewer title, closing returns it to the row, a folder opened
 * from the keyboard keeps focus in the new list) and never on load.
 */
import { formatLabel, type LabelTable } from "../theme/labels";
import { A11Y_STYLE } from "./a11y-style";
import { focusElement } from "../a11y/focus";
import { RETRY_EVENT } from "./fs-states";
import { failureState, type ViewState } from "./view-state";
import { ApiError } from "../api/client";
import {
  MAX_LISTING_PAGES,
  PAGE_SIZE,
  affectsPath,
  fileRows,
  formatSize,
  lastSegment,
  locationHash,
  parentPath,
  prefixTrail,
  rowSummary,
  type FileCommit,
  type FileLocation,
  type FileRef,
  type FileRow,
  type FolderEntry,
  type FolderPage,
} from "../files/model";
import type { PreviewRenderer } from "../files/preview";
import type { FileSource } from "../files/source";
import { buildFrame, STYLE as FRAME_STYLE, type Frame } from "./file-browser-frame";
import { FileList, type FileLayout } from "./file-list";
import { STYLE as ROW_STYLE } from "./file-row-view";
import { FileWrites, type WriteOperation, type WritesHost } from "./file-browser-writes";

/** What the browser needs. */
export interface FileBrowserOptions {
  /** The active label table. */
  labels: LabelTable;
  /** Reads and writes the files. */
  source: FileSource;
  /** The link of a folder: its route. Defaults to {@link locationHash}. */
  href?: (location: FileLocation) => string;
  /** Whether the browser is online; defaults to `navigator.onLine`. */
  online?: () => boolean;
  /** Entries per listing page (default 200). */
  pageSize?: number;
  /** Most listing pages read for one folder (default 500). */
  maxPages?: number;
}

/** What reading the pages of a folder produced; applied only if it is still the current read. */
interface PagesRead {
  entries: FolderEntry[];
  cursor: string | null;
  pages: number;
  head: string;
}

/** An element a distribution mounts in a panel slot. */
export type FilePanel = HTMLElement & { fileRef?: FileRef | null };

/** Builds a panel element. */
export type PanelFactory = () => FilePanel;

/** One registered panel. */
interface RegisteredPanel {
  readonly id: string;
  readonly create: PanelFactory;
  element?: FilePanel;
}

const STYLE = `
  :host { display: block; min-width: 0; font-family: var(--fs-font-sans); font-size: var(--fs-font-size-body); color: var(--fs-on-surface); }
  :host([hidden]) { display: none; }
`;

/** Custom element that browses and edits the files of one object. */
export class FsFileBrowser extends HTMLElement {
  private options?: FileBrowserOptions;
  private frame?: Frame;
  private list?: FileList;
  private writes?: FileWrites;
  private where: FileLocation = { library_id: "", object_id: "", prefix: "" };
  private entries: FolderEntry[] = [];
  private rows: FileRow[] = [];
  private readonly pending = new Set<string>();
  private cursor: string | null = null;
  private pages = 0;
  private head = "";
  /** The location whose listing is loaded and shown now; undefined while loading or failed. */
  private listed?: FileLocation;
  private loadingMore = false;
  private generation = 0;
  private abort?: AbortController;
  private layout: FileLayout = "list";
  private panels: RegisteredPanel[] = [];
  private renderers: PreviewRenderer[] = [];
  private current: FileRef | null = null;
  private returnKey = "";
  private hrefs = new Map<string, string>();

  constructor() {
    super();
    this.attachShadow({ mode: "open" });
  }

  /**
   * Set the labels and source and render the frame. Call before or after connecting, then set
   * `location`.
   *
   * @param options Labels, source and optional link builder and paging bounds.
   */
  configure(options: FileBrowserOptions): void {
    this.options = options;
    this.build();
    if (this.where.object_id !== "") void this.loadFolder();
  }

  /** Where the browser is: the object and the folder. */
  get location(): FileLocation {
    return this.where;
  }

  /** Go to a folder of an object: loads its listing and closes any open file. */
  set location(location: FileLocation) {
    const same = this.sameLocation(location);
    if (
      location.library_id !== this.where.library_id ||
      location.object_id !== this.where.object_id
    ) {
      this.pending.clear();
    }
    if (!same) this.writes?.leave();
    this.where = location;
    if (same && this.dataset.state === "ready") return;
    this.closeViewer(false);
    void this.loadFolder();
  }

  /** The file open in the viewer, or null. */
  get file(): FileRef | null {
    return this.current;
  }

  /**
   * Mount a panel in a named sidebar slot. A panel registered again under the same id replaces
   * the old one. The panel is created when a file opens and receives `fileRef`.
   *
   * @param id The slot name, for example `details`.
   * @param create Builds the panel element.
   */
  registerPanel(id: string, create: PanelFactory): void {
    const existing = this.panels.findIndex((panel) => panel.id === id);
    if (existing >= 0) this.panels[existing]?.element?.remove();
    const entry: RegisteredPanel = { id, create };
    if (existing >= 0) this.panels[existing] = entry;
    else this.panels.push(entry);
    this.mountPanels();
  }

  /**
   * Add a preview renderer, tried before the built-in ones. One registered again under the same
   * id replaces the old one.
   *
   * @param renderer The renderer.
   */
  registerPreviewRenderer(renderer: PreviewRenderer): void {
    this.renderers = [...this.renderers.filter((r) => r.id !== renderer.id), renderer];
    this.configurePreview();
  }

  /** Release the listing's observer when the element leaves the document. */
  disconnectedCallback(): void {
    this.abort?.abort();
    this.list?.disconnect();
  }

  /** Follow size changes of the listing when attached. */
  connectedCallback(): void {
    this.list?.connect();
  }

  /** True when `location` is the one already shown. */
  private sameLocation(location: FileLocation, now: FileLocation = this.where): boolean {
    return (
      now.library_id === location.library_id &&
      now.object_id === location.object_id &&
      now.prefix === location.prefix
    );
  }

  /** The name the loaded listing of `location` shows for `path`, if it is the one shown. */
  private nameOf(location: FileLocation, path: string): string | undefined {
    const loaded = this.listed;
    if (!loaded || !this.sameLocation(location, loaded) || !this.sameLocation(location)) {
      return undefined;
    }
    return this.rowOfPath(path)?.name;
  }

  /** Build the frame, the listing and the write controller. */
  private build(): void {
    const root = this.shadowRoot as ShadowRoot;
    const options = this.options as FileBrowserOptions;
    const frame = buildFrame(options.labels);
    this.frame = frame;
    this.list = new FileList({
      labels: options.labels,
      locale: document.documentElement.lang || "en",
      href: (row) => this.routeOf(row.path),
      rtl: () => getComputedStyle(this).direction === "rtl",
      hooks: {
        open: (row) => this.openRow(row),
        choose: () => this.writes?.syncToolbar(),
        rename: (row) => this.writes?.rename(row),
        remove: (row) => void this.writes?.remove(row),
        more: () => void this.loadMore(),
      },
    });
    frame.listing.host.replaceChildren(this.list.element);
    this.writes = new FileWrites(frame, this.writesHost(options));
    this.wireEvents(root, frame);
    const style = document.createElement("style");
    style.textContent = A11Y_STYLE + STYLE + FRAME_STYLE + ROW_STYLE;
    root.replaceChildren(style, frame.root);
    this.configurePreview();
    this.mountPanels();
    if (this.isConnected) this.list.connect();
  }

  /** What the write controller needs from this element. */
  private writesHost(options: FileBrowserOptions): WritesHost {
    return {
      labels: options.labels,
      source: options.source,
      location: () => this.where,
      target: () => (this.current ? this.rowOfPath(this.current.path) : this.list?.chosenRow()),
      reload: () => this.reload(),
      committed: (operation: WriteOperation, commit: FileCommit, changed: readonly string[]) =>
        this.onCommitted(operation, commit, changed),
      nameOf: (location: FileLocation, path: string) => this.nameOf(location, path),
      pendFolder: (path: string) => void this.pending.add(path),
      focusPath: (path: string) => this.list?.focusRow(path),
    };
  }

  /** Attach the listeners of the frame. */
  private wireEvents(root: ShadowRoot, frame: Frame): void {
    const { bar, viewer, listing } = frame;
    bar.list.addEventListener("click", () => this.setLayout("list"));
    bar.grid.addEventListener("click", () => this.setLayout("grid"));
    frame.breadcrumb.addEventListener("click", (event) => this.onCrumbClick(event));
    viewer.back.addEventListener("click", () => this.closeViewer(true));
    viewer.previous.addEventListener("click", () => this.step(-1));
    viewer.next.addEventListener("click", () => this.step(1));
    viewer.element.addEventListener("keydown", (event) => {
      if (event.key === "Escape") this.closeViewer(true);
    });
    root.addEventListener(RETRY_EVENT, () => void this.loadFolder());
    for (const type of ["dragenter", "dragover"]) {
      listing.element.addEventListener(type, (event) => this.onDrag(event as DragEvent));
    }
    listing.element.addEventListener("dragleave", (event) => this.onDragLeave(event as DragEvent));
    listing.element.addEventListener("drop", (event) => this.onDrop(event as DragEvent));
  }

  /** The label table. */
  private get labels(): LabelTable {
    return (this.options as FileBrowserOptions).labels;
  }

  /** The route of the folder at `path` in the current object. */
  private routeOf(path: string): string {
    const location = { ...this.where, prefix: path };
    return this.options?.href ? this.options.href(location) : locationHash(location);
  }

  /** Switch between list and grid. */
  private setLayout(layout: FileLayout): void {
    this.layout = layout;
    const bar = this.frame?.bar;
    bar?.list.setAttribute("aria-pressed", String(layout === "list"));
    bar?.grid.setAttribute("aria-pressed", String(layout === "grid"));
    this.list?.setLayout(layout);
  }

  /** Read the first page of the folder and show it (the whole shown extent again on reload). */
  private async loadFolder(extent = 0): Promise<void> {
    const options = this.options;
    const frame = this.frame;
    if (!options || !frame || this.where.object_id === "") return;
    this.abort?.abort();
    const abort = new AbortController();
    this.abort = abort;
    const generation = ++this.generation;
    const hadFocus = this.list?.hasFocus() ?? false;
    this.listed = undefined;
    if (this.rows.length === 0 || extent === 0) this.showState({ kind: "loading" });
    this.updateCrumbs();
    try {
      const where = this.where;
      const read = await this.readPages(where, extent, abort.signal);
      if (generation !== this.generation) return;
      this.entries = read.entries;
      this.cursor = read.cursor;
      this.pages = read.pages;
      this.head = read.head;
      this.listed = where;
      this.afterLoad(hadFocus);
    } catch (error) {
      if (generation !== this.generation || abort.signal.aborted) return;
      this.failLoad(error);
    }
  }

  /** Read the first page, then more pages until `extent` entries are shown (bounded). */
  private async readPages(
    where: FileLocation,
    extent: number,
    signal: AbortSignal,
  ): Promise<PagesRead> {
    const source = (this.options as FileBrowserOptions).source;
    const size = this.options?.pageSize ?? PAGE_SIZE;
    const limit = this.options?.maxPages ?? MAX_LISTING_PAGES;
    const read: PagesRead = { entries: [], cursor: null, pages: 0, head: this.head };
    do {
      const page = await this.readOne(source, where, read.cursor, size, signal);
      read.entries.push(...page.entries);
      read.cursor = page.next_cursor;
      read.head = page.version;
      read.pages += 1;
    } while (read.cursor !== null && read.entries.length < extent && read.pages < limit);
    return read;
  }

  /** Read one page; a folder that exists only in the client (pending) is empty, not missing. */
  private async readOne(
    source: FileSource,
    where: FileLocation,
    cursor: string | null,
    limit: number,
    signal: AbortSignal,
  ): Promise<FolderPage> {
    try {
      return await source.list(where, cursor, limit, { signal });
    } catch (error) {
      const missing = error instanceof ApiError && error.code === "not_found";
      if (!missing || !this.isPendingFolder(where)) throw error;
      return {
        library_id: "",
        object_id: "",
        version: this.head,
        prefix: "",
        entries: [],
        next_cursor: null,
      };
    }
  }

  /** True when `prefix` is a folder created in the client, or lies below one. */
  private isPendingFolder(where: FileLocation): boolean {
    const prefix = where.prefix;
    return prefix !== "" && [...this.pending].some((path) => affectsPath(path, prefix));
  }

  /** Load the next page when the window nears the loaded end. */
  private async loadMore(): Promise<void> {
    const options = this.options;
    const limit = options?.maxPages ?? MAX_LISTING_PAGES;
    if (!options || this.cursor === null || this.loadingMore || this.pages >= limit) return;
    this.loadingMore = true;
    const generation = this.generation;
    try {
      const page = await options.source.list(
        this.where,
        this.cursor,
        options.pageSize ?? PAGE_SIZE,
        {
          signal: this.abort?.signal,
        },
      );
      if (generation !== this.generation) return;
      this.cursor = page.next_cursor;
      this.pages += 1;
      this.entries = [...this.entries, ...page.entries];
      this.applyRows();
    } catch (error) {
      if (generation === this.generation) this.failLoad(error);
    } finally {
      this.loadingMore = false;
    }
  }

  /** Show the loaded entries; return focus to the first row if it was in the old list. */
  private afterLoad(hadFocus: boolean): void {
    this.applyRows();
    const shadow = this.shadowRoot as ShadowRoot;
    if (hadFocus && shadow.activeElement === null) this.list?.focusFirst();
  }

  /** Build the rows, the footer and the state from the loaded entries. */
  private applyRows(): void {
    const frame = this.frame as Frame;
    const listed = new Set(this.entries.map((entry) => entry.path));
    for (const path of this.pending) if (listed.has(path)) this.pending.delete(path);
    const here = [...this.pending].filter((path) => parentPath(path) === this.where.prefix);
    this.rows = fileRows(this.entries, here);
    const more = this.cursor !== null;
    this.list?.setRows(this.rows, more);
    this.writes?.syncToolbar();
    this.showFooter(more);
    this.showState(this.rows.length === 0 ? { kind: "empty" } : undefined);
    frame.listing.host.hidden = this.rows.length === 0;
  }

  /** Show the summary and the "more" and "limit" notes. */
  private showFooter(more: boolean): void {
    const { summary, more: moreNote, limit } = (this.frame as Frame).listing;
    const sum = rowSummary(this.rows);
    summary.textContent = formatLabel(this.labels.fileSummary, {
      folders: String(sum.folders),
      files: String(sum.files),
      size: formatSize(sum.bytes, this.labels, document.documentElement.lang || "en"),
    });
    const capped = more && this.pages >= (this.options?.maxPages ?? MAX_LISTING_PAGES);
    limit.hidden = !capped;
    limit.textContent = capped
      ? formatLabel(this.labels.fileLimitReached, { count: String(this.entries.length) })
      : "";
    moreNote.hidden = !more || capped;
  }

  /** Show a non-content state, or hide the states element for content (undefined). */
  private showState(state: ViewState | undefined): void {
    const { states } = (this.frame as Frame).listing;
    states.hidden = state === undefined;
    if (state) states.state = state;
    const kind = state?.kind ?? "ready";
    this.dataset.state = kind;
    if (kind !== "ready" && kind !== "empty") {
      (this.frame as Frame).listing.host.hidden = true;
    }
  }

  /** Show a failed read as the error, offline, denied or unavailable state. */
  private failLoad(error: unknown): void {
    const online = this.options?.online ? this.options.online() : navigator.onLine;
    console.warn("FileSquid could not list a folder", error instanceof Error ? error.name : error);
    this.showState(failureState(error, online));
  }

  /** Reload the listing, keeping the number of entries already shown. */
  private async reload(): Promise<void> {
    await this.loadFolder(this.entries.length);
    this.refreshViewer();
  }

  /** Set the breadcrumb to the root and the folders down to the current one. */
  private updateCrumbs(): void {
    const frame = this.frame as Frame;
    this.hrefs.clear();
    const steps = [{ name: this.labels.fileRoot, prefix: "" }, ...prefixTrail(this.where.prefix)];
    frame.breadcrumb.items = steps.map((step) => {
      const href = this.routeOf(step.prefix);
      this.hrefs.set(href, step.prefix);
      return { node: step.prefix, name: step.name, href };
    });
  }

  /** A click in the breadcrumb: enter that folder unless the click is a modified one. */
  private onCrumbClick(event: Event): void {
    const mouse = event as MouseEvent;
    const link = event
      .composedPath()
      .find((el): el is HTMLAnchorElement => el instanceof HTMLAnchorElement);
    const native =
      mouse.ctrlKey || mouse.metaKey || mouse.shiftKey || mouse.altKey || mouse.button !== 0;
    const path = link ? this.hrefs.get(link.getAttribute("href") ?? "") : undefined;
    if (path === undefined || native) return;
    event.preventDefault();
    this.enter(path);
  }

  /** Enter a folder and announce the new location. */
  private enter(prefix: string): void {
    this.closeViewer(false);
    this.where = { ...this.where, prefix };
    this.emit("fs-location-change", { location: this.where });
    void this.loadFolder();
  }

  /** A row was activated: enter a folder or open a file. */
  private openRow(row: FileRow): void {
    if (row.kind === "folder") this.enter(row.path);
    else this.openFile(row);
  }

  /** Open a file in the viewer. */
  private openFile(row: FileRow): void {
    const viewer = (this.frame as Frame).viewer;
    this.returnKey = row.key;
    this.showViewer(this.refOf(row));
    this.updateViewerHeader(row);
    focusElement(viewer.title);
  }

  /** The reference of a listed file at the version listed. */
  private refOf(row: FileRow): FileRef {
    return {
      library_id: this.where.library_id,
      object_id: this.where.object_id,
      version: this.head,
      path: row.path,
      display_name: row.name,
    };
  }

  /** Show the viewer for `ref`, hide the listing, and hand `ref` to the preview and panels. */
  private showViewer(ref: FileRef): void {
    const frame = this.frame as Frame;
    this.current = ref;
    frame.viewer.element.hidden = false;
    frame.listing.element.hidden = true;
    frame.panels.hidden = this.panels.length === 0;
    frame.viewer.preview.file = ref;
    const mounted = this.panels.filter((panel) => panel.element);
    this.mountPanels();
    for (const panel of mounted) if (panel.element) panel.element.fileRef = ref;
    this.writes?.syncToolbar();
    this.emit("fs-file-open", { file: ref });
  }

  /** Close the viewer; when `focus`, return focus to the row that opened it. */
  private closeViewer(focus: boolean): void {
    const frame = this.frame;
    if (!frame || this.current === null) return;
    this.current = null;
    frame.viewer.element.hidden = true;
    frame.listing.element.hidden = false;
    frame.panels.hidden = true;
    frame.viewer.preview.file = null;
    for (const panel of this.panels) if (panel.element) panel.element.fileRef = null;
    this.writes?.syncToolbar();
    this.emit("fs-file-open", { file: null });
    if (focus) this.list?.focusRow(this.returnKey);
  }

  /** The title, position and the previous and next buttons for the open file. */
  private updateViewerHeader(row: FileRow): void {
    const { viewer } = this.frame as Frame;
    const files = this.rows.filter((r) => r.kind === "file");
    const index = files.findIndex((r) => r.key === row.key);
    viewer.title.textContent = row.name;
    viewer.position.textContent = formatLabel(this.labels.filePosition, {
      position: String(index + 1),
      total: String(files.length),
    });
    viewer.previous.setAttribute("aria-disabled", String(index <= 0));
    viewer.next.setAttribute("aria-disabled", String(index >= files.length - 1));
  }

  /** Open the previous or next file of the folder; nothing at the ends. */
  private step(delta: number): void {
    const frame = this.frame as Frame;
    const button = delta < 0 ? frame.viewer.previous : frame.viewer.next;
    if (button.getAttribute("aria-disabled") === "true" || this.current === null) return;
    const files = this.rows.filter((r) => r.kind === "file");
    const index = files.findIndex((r) => r.path === this.current?.path);
    const row = files[index + delta];
    if (!row) return;
    this.returnKey = row.key;
    this.showViewer(this.refOf(row));
    this.updateViewerHeader(row);
  }

  /** After a reload, point the viewer at the file's new head version, or close it if gone. */
  private refreshViewer(): void {
    const current = this.current;
    if (!current) return;
    const row = this.rowOfPath(current.path);
    if (!row) return this.closeViewer(false);
    if (current.version !== this.head) this.showViewer(this.refOf(row));
    this.updateViewerHeader(row);
  }

  /** The listed row with this logical path. */
  private rowOfPath(path: string): FileRow | undefined {
    return this.rows.find((row) => row.path === path);
  }

  /**
   * A write committed: announce it, and close the viewer when its file was moved or removed. A
   * commit to another object than the one shown (an Undo, or a call answered after the object
   * changed) is only announced: the shown object's head and viewer are not its.
   */
  private onCommitted(
    operation: WriteOperation,
    commit: FileCommit,
    changed: readonly string[],
  ): void {
    const shown =
      commit.library_id === this.where.library_id && commit.object_id === this.where.object_id;
    if (!shown) return this.emit("fs-files-change", { operation, commit });
    this.head = commit.version;
    const open = this.current?.path;
    const gone = (operation === "remove" || operation === "move") && open !== undefined;
    if (gone && changed.some((path) => affectsPath(path, open))) this.closeViewer(false);
    this.emit("fs-files-change", { operation, commit });
  }

  /** Mount every registered panel that is not mounted yet, in registration order. */
  private mountPanels(): void {
    const frame = this.frame;
    if (!frame) return;
    if (this.current === null) {
      frame.panels.hidden = true;
      return;
    }
    for (const panel of this.panels) {
      if (!panel.element) {
        panel.element = panel.create();
        panel.element.fileRef = this.current;
      }
    }
    const slots = this.panels.map((panel) => slot(panel));
    frame.panels.replaceChildren(...slots);
    frame.panels.hidden = slots.length === 0;
  }

  /** Give the preview its labels, source and renderers. */
  private configurePreview(): void {
    const options = this.options;
    if (!options || !this.frame) return;
    this.frame.viewer.preview.configure({
      labels: options.labels,
      source: options.source,
      renderers: this.renderers,
      online: options.online,
    });
  }

  /** Dispatch a bubbling, composed event. */
  private emit(name: string, detail: unknown): void {
    this.dispatchEvent(new CustomEvent(name, { detail, bubbles: true, composed: true }));
  }

  /** True when a drag carries files. */
  private carriesFiles(event: DragEvent): boolean {
    return Array.from(event.dataTransfer?.types ?? []).includes("Files");
  }

  /** A drag of files is over the listing: say where they would go. */
  private onDrag(event: DragEvent): void {
    if (!this.carriesFiles(event)) return;
    event.preventDefault();
    const { drop } = (this.frame as Frame).listing;
    const folder = this.where.prefix === "" ? this.labels.fileRoot : lastSegment(this.where.prefix);
    drop.textContent = formatLabel(this.labels.fileDropTarget, { folder });
    drop.hidden = false;
  }

  /** The drag left the listing. */
  private onDragLeave(event: DragEvent): void {
    const { element, drop } = (this.frame as Frame).listing;
    const entered = event.relatedTarget;
    if (!(entered instanceof Node) || !element.contains(entered)) drop.hidden = true;
  }

  /** Files were dropped on the listing: upload them into this folder. */
  private onDrop(event: DragEvent): void {
    if (!this.carriesFiles(event)) return;
    event.preventDefault();
    (this.frame as Frame).listing.drop.hidden = true;
    void this.writes?.upload(Array.from(event.dataTransfer?.files ?? []));
  }
}

/** The sidebar section of a panel. */
function slot(panel: RegisteredPanel): HTMLElement {
  const section = document.createElement("section");
  section.dataset.slot = panel.id;
  section.append(panel.element as HTMLElement);
  return section;
}
