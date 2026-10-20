/**
 * `<fs-file-preview>`: the preview area of the file browser (UI-F2.2). Give it a file version
 * (`file`) and it reads the file's storage facts, picks the first renderer that accepts the
 * media type (a registered one before the built-ins) and shows its element: an image, video or
 * audio file through a content handle (UI-S-014), or text through one ranged read of the first
 * 64 KiB. A file no renderer accepts gets a plain "no preview" message and never a broken
 * viewer. Loading, error, offline and permission denied use `<fs-states>` with Try again.
 *
 * The element never fetches by itself: the {@link FileSource} it is given does. Its `data-state`
 * is `empty`, `loading`, `ready`, `unsupported`, `error`, `offline` or `denied`.
 *
 * Accessibility (UI-S-016): the shared stylesheet comes first; every media element and the text
 * area are named by the file's display name (UI-S-019); a failed media element is replaced by an
 * alert; changing the file removes the old preview and its media listeners.
 */
import type { LabelTable } from "../theme/labels";
import { A11Y_STYLE } from "./a11y-style";
import { RETRY_EVENT, type FsStates } from "./fs-states";
import { failureState, type ViewState } from "./view-state";
import type { FileDetails, FileRef } from "../files/model";
import {
  MEDIA_STATE_EVENT,
  builtInRenderers,
  pickRenderer,
  type PreviewContext,
  type PreviewRenderer,
} from "../files/preview";
import type { FileSource } from "../files/source";

/** What the preview needs. */
export interface FilePreviewOptions {
  /** The active label table. */
  labels: LabelTable;
  /** Byte access and storage facts. */
  source: FileSource;
  /** Renderers tried before the built-ins, first registered first. */
  renderers?: readonly PreviewRenderer[];
  /** Whether the browser is online; defaults to `navigator.onLine`. */
  online?: () => boolean;
}

const STYLE = `
  :host { display: block; min-width: 0; font-family: var(--fs-font-sans); font-size: var(--fs-font-size-body); color: var(--fs-on-surface); }
  :host([hidden]) { display: none; }
  figure { display: flex; flex-direction: column; align-items: center; gap: var(--fs-space-2); margin: 0; min-width: 0; }
  img, video { max-inline-size: 100%; max-block-size: 70vh; border-radius: var(--fs-radius); background: var(--fs-surface-container); }
  audio { inline-size: 100%; }
  pre { box-sizing: border-box; inline-size: 100%; max-block-size: 70vh; margin: 0; padding: var(--fs-space-3); overflow: auto; border: 1px solid var(--fs-outline); border-radius: var(--fs-radius); background: var(--fs-surface-container); font-family: var(--fs-font-mono); font-size: var(--fs-font-size-small); white-space: pre-wrap; overflow-wrap: anywhere; }
  .note, .help { margin: 0; color: var(--fs-on-surface-variant); font-size: var(--fs-font-size-small); }
  .media-error { margin: 0; color: var(--fs-error); font-weight: 600; overflow-wrap: anywhere; }
  .unsupported { display: flex; flex-direction: column; align-items: center; gap: var(--fs-space-2); padding: var(--fs-space-5); border: 1px solid var(--fs-outline); border-radius: var(--fs-radius); background: var(--fs-surface-container); text-align: center; }
  .unsupported .title { margin: 0; font-weight: 600; }
  [hidden] { display: none !important; }
`;

/** Custom element that previews one file version. */
export class FsFilePreview extends HTMLElement {
  private options?: FilePreviewOptions;
  private current: FileRef | null = null;
  private generation = 0;
  private abort?: AbortController;
  private cleanups: (() => void)[] = [];

  constructor() {
    super();
    const root = this.attachShadow({ mode: "open" });
    root.addEventListener(RETRY_EVENT, (event) => {
      event.stopPropagation();
      void this.show();
    });
    root.addEventListener(MEDIA_STATE_EVENT, (event) => {
      const state = (event as CustomEvent<string>).detail;
      this.dataset.state = state === "error" ? "error" : "ready";
    });
  }

  /**
   * Set the labels, source and renderers, then render. Call before or after connecting.
   *
   * @param options Labels, source and optional renderers.
   */
  configure(options: FilePreviewOptions): void {
    this.options = options;
    void this.show();
  }

  /** The file version previewed now, or null. */
  get file(): FileRef | null {
    return this.current;
  }

  /** Preview another file version, or none with null. */
  set file(file: FileRef | null) {
    this.current = file;
    void this.show();
  }

  /** Stop the preview's work when the element leaves the document. */
  disconnectedCallback(): void {
    this.dispose();
  }

  /** Abort the work for the file shown and run its cleanups. Idempotent. */
  private dispose(): void {
    this.abort?.abort();
    this.abort = undefined;
    const cleanups = this.cleanups;
    this.cleanups = [];
    for (const cleanup of cleanups) cleanup();
  }

  /** Show the current file: its details, then the renderer's element. */
  private async show(): Promise<void> {
    const root = this.shadowRoot;
    const options = this.options;
    if (!root || !options) return;
    this.dispose();
    const generation = ++this.generation;
    const file = this.current;
    if (!file) return this.render([], "empty");
    this.render([this.states(options.labels, { kind: "loading" })], "loading");
    const abort = new AbortController();
    this.abort = abort;
    try {
      const details = await options.source.details(file, { signal: abort.signal });
      if (generation !== this.generation) return;
      await this.present(file, details, abort.signal, generation);
    } catch (error) {
      if (generation !== this.generation) return;
      this.fail(error);
    }
  }

  /** Render the file with the renderer that accepts it, or the unsupported message. */
  private async present(
    file: FileRef,
    details: FileDetails,
    signal: AbortSignal,
    generation: number,
  ): Promise<void> {
    const options = this.options as FilePreviewOptions;
    const renderers = [...(options.renderers ?? []), ...builtInRenderers()];
    const renderer = pickRenderer(renderers, details);
    if (!renderer) return this.render([unsupported(options.labels)], "unsupported");
    const context = this.context(file, details, signal);
    const element = await renderer.render(context);
    if (generation !== this.generation) return;
    this.render([element], "ready");
  }

  /** The context a renderer gets; its cleanups run when the preview is replaced. */
  private context(file: FileRef, details: FileDetails, signal: AbortSignal): PreviewContext {
    const options = this.options as FilePreviewOptions;
    return {
      file,
      details,
      labels: options.labels,
      source: options.source,
      signal,
      onDispose: (cleanup) => this.cleanups.push(cleanup),
    };
  }

  /** Show a failed read as the matching state. */
  private fail(error: unknown): void {
    const options = this.options as FilePreviewOptions;
    const online = options.online ? options.online() : navigator.onLine;
    const state = failureState(error, online);
    console.warn("FileSquid could not preview a file", error instanceof Error ? error.name : error);
    this.render([this.states(options.labels, state)], state.kind);
  }

  /** A `<fs-states>` showing `state`. */
  private states(labels: LabelTable, state: ViewState): FsStates {
    const states = document.createElement("fs-states") as FsStates;
    states.configure({ labels });
    states.state = state;
    return states;
  }

  /** Replace the shown content and set `data-state`. */
  private render(content: readonly Node[], state: string): void {
    const root = this.shadowRoot as ShadowRoot;
    const style = document.createElement("style");
    style.textContent = A11Y_STYLE + STYLE;
    root.replaceChildren(style, ...content);
    this.dataset.state = state;
  }
}

/** The message for a file no renderer accepts. */
function unsupported(labels: LabelTable): HTMLElement {
  const box = document.createElement("div");
  box.className = "unsupported";
  box.setAttribute("role", "status");
  const title = document.createElement("p");
  title.className = "title";
  title.textContent = labels.fileNoPreview;
  const help = document.createElement("p");
  help.className = "help";
  help.textContent = labels.fileNoPreviewHelp;
  box.append(title, help);
  return box;
}
