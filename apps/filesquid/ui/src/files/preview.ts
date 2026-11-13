/**
 * Preview renderers of the file browser (UI-F2.2, ADR 028 Decision 6): the contract a renderer
 * implements, the built-in renderers (image, video and audio through content handles; text
 * through a ranged read) and the choice among them. A distribution registers its own renderer
 * and it is tried before the built-ins; a file no renderer accepts gets the unsupported
 * fallback in `<fs-file-preview>`, never a broken viewer.
 *
 * The built-in media renderers set a content-handle URL only as the `src` of a media element
 * (UI-S-014); the byte access a renderer gets is the host's {@link FileSource}. The image
 * renderer adds the zoom chip of UI-S-031 under the media (Task UI-F2.19).
 */
import type { LabelTable } from "../theme/labels";
import { formatLabel } from "../theme/labels";
import { MediaHandleController, type MediaState } from "./media-handle";
import { fitPercent } from "./zoom";
import {
  TEXT_PREVIEW_BYTES,
  formatSize,
  previewKind,
  type FileDetails,
  type FileRef,
  type PreviewKind,
} from "./model";
import type { FileSource } from "./source";

/** Event a media figure dispatches (bubbling inside the preview) when its state changes. */
export const MEDIA_STATE_EVENT = "fs-media-state";

/** What a renderer gets for one file. */
export interface PreviewContext {
  /** The file version being previewed; `display_name` is the name to show and announce. */
  readonly file: FileRef;
  /** Its storage facts, including the media type. */
  readonly details: FileDetails;
  /** The active label table. */
  readonly labels: LabelTable;
  /** The host's byte access: ranged reads and content handles. */
  readonly source: FileSource;
  /** Aborted when the preview is replaced or removed. */
  readonly signal: AbortSignal;
  /** Register work to run when the preview is replaced or removed (stop playback, detach). */
  readonly onDispose: (cleanup: () => void) => void;
}

/** A named extension point that turns one kind of file into a preview element. */
export interface PreviewRenderer {
  /** Stable identifier; a renderer registered with an existing id replaces that one. */
  readonly id: string;
  /** True when this renderer can show the file. Pure. */
  accepts(details: FileDetails): boolean;
  /**
   * Build the preview. May read bytes through `context.source`. A rejection is shown as the
   * preview's error state.
   */
  render(context: PreviewContext): HTMLElement | Promise<HTMLElement>;
}

/** The first renderer that accepts the file, custom ones first. Pure. */
export function pickRenderer(
  renderers: readonly PreviewRenderer[],
  details: FileDetails,
): PreviewRenderer | undefined {
  return renderers.find((renderer) => renderer.accepts(details));
}

/** A `<p>` with a class and text. */
function paragraph(className: string, text: string): HTMLParagraphElement {
  const p = document.createElement("p");
  p.className = className;
  p.textContent = text;
  return p;
}

/** A renderer for one kind of media type. */
function kindRenderer(kind: PreviewKind, render: PreviewRenderer["render"]): PreviewRenderer {
  return {
    id: `fs-${kind}`,
    accepts: (details) => previewKind(details.media_type) === kind,
    render,
  };
}

/**
 * Feed a media element from content handles and wrap it with the error message the controller
 * can reveal. The wrapper is a `figure`; its `data-state` follows the controller, and `follow`
 * (the image's zoom chip) hears every state too.
 */
function mediaFigure(
  element: HTMLElement,
  context: PreviewContext,
  follow?: (state: MediaState) => void,
): HTMLElement {
  const figure = document.createElement("figure");
  const failure = paragraph("media-error", context.labels.filePreviewError);
  failure.setAttribute("role", "alert");
  failure.hidden = true;
  figure.append(element, failure);
  const controller = new MediaHandleController(
    element as HTMLImageElement | HTMLMediaElement,
    () => context.source.mintContent(context.file, { signal: context.signal }),
    (state) => {
      figure.dataset.state = state;
      failure.hidden = state !== "error";
      element.hidden = state === "error";
      follow?.(state);
      figure.dispatchEvent(new CustomEvent(MEDIA_STATE_EVENT, { detail: state, bubbles: true }));
    },
  );
  context.onDispose(() => controller.dispose());
  void controller.start();
  return figure;
}

/** The parts of the zoom chip that change with the image's state. */
interface ZoomChip {
  readonly element: HTMLElement;
  readonly fit: HTMLElement;
  readonly loading: HTMLElement;
  readonly progress: HTMLElement;
}

/**
 * Build the zoom chip of UI-S-031: "Fit · {percent}%" and, while the full-size image loads,
 * "Loading full size" with a progress bar named by that line. The `<img>` element reports no
 * byte progress, so the bar is indeterminate (no `aria-valuenow`). Pure DOM construction.
 */
function buildZoomChip(labels: LabelTable): ZoomChip {
  const element = document.createElement("div");
  element.className = "zoom";
  const fit = document.createElement("span");
  fit.className = "fit";
  const loading = document.createElement("span");
  loading.className = "loading";
  loading.id = "zoom-loading";
  loading.textContent = labels.fileLoadingFullSize;
  const progress = document.createElement("span");
  progress.className = "progress";
  progress.setAttribute("role", "progressbar");
  progress.setAttribute("aria-labelledby", loading.id);
  const fill = document.createElement("span");
  fill.className = "progress-fill";
  progress.append(fill);
  element.append(fit, loading, progress);
  return { element, fit, loading, progress };
}

/** Write the fitted scale of `img` into the chip; no percent hides the line. */
function showFit(chip: ZoomChip, img: HTMLImageElement, labels: LabelTable): void {
  const percent = fitPercent(img.getBoundingClientRect().width, img.naturalWidth);
  chip.fit.hidden = percent === null;
  chip.fit.textContent =
    percent === null ? "" : formatLabel(labels.fileZoomFit, { percent: String(percent) });
}

/** Show the chip for a media state: loading, the fitted scale when ready, nothing on error. */
function showZoomState(
  chip: ZoomChip,
  img: HTMLImageElement,
  labels: LabelTable,
  state: MediaState,
): void {
  chip.element.dataset.state = state;
  chip.element.hidden = state === "error";
  chip.loading.hidden = state !== "loading";
  chip.progress.hidden = state !== "loading";
  if (state === "ready") showFit(chip, img, labels);
  else chip.fit.hidden = true;
}

/**
 * Keep the chip's percent in step with the shown width (a resized window or panel). Side
 * effect: observes `img` until the preview is replaced; no observer where the browser has none.
 */
function followSize(chip: ZoomChip, img: HTMLImageElement, context: PreviewContext): void {
  if (typeof ResizeObserver === "undefined") return;
  const observer = new ResizeObserver(() => {
    if (chip.element.dataset.state === "ready") showFit(chip, img, context.labels);
  });
  observer.observe(img);
  context.onDispose(() => observer.disconnect());
}

/** Name a media element for assistive technology with the display name. */
function named<T extends HTMLElement>(element: T, name: string): T {
  element.setAttribute("aria-label", name);
  return element;
}

/** A native `<video>` or `<audio>` with controls. */
function timed(tag: "video" | "audio", context: PreviewContext): HTMLElement {
  const element = named(document.createElement(tag), context.file.display_name);
  element.controls = true;
  element.preload = "metadata";
  return mediaFigure(element, context);
}

/**
 * The built-in image renderer: a native `<img>` over a content handle, with the zoom chip of
 * UI-S-031 under the media (Task UI-F2.19). Zooming beyond Fit is not offered.
 */
const IMAGE = kindRenderer("image", (context) => {
  const img = document.createElement("img");
  img.alt = context.file.display_name;
  const chip = buildZoomChip(context.labels);
  const figure = mediaFigure(img, context, (state) =>
    showZoomState(chip, img, context.labels, state),
  );
  figure.append(chip.element);
  followSize(chip, img, context);
  return figure;
});

/** The built-in video renderer. */
const VIDEO = kindRenderer("video", (context) => timed("video", context));

/** The built-in audio renderer. */
const AUDIO = kindRenderer("audio", (context) => timed("audio", context));

/** The built-in text renderer: the first {@link TEXT_PREVIEW_BYTES} bytes through one ranged read. */
const TEXT = kindRenderer("text", async (context) => {
  const range = { start: 0, end: TEXT_PREVIEW_BYTES - 1 };
  // An empty file has no byte to read (the API answers a range on it with 416).
  const bytes =
    context.details.size === 0
      ? new Uint8Array(0)
      : await context.source.readRange(context.file, range, { signal: context.signal });
  const figure = document.createElement("figure");
  const pre = named(document.createElement("pre"), context.file.display_name);
  pre.tabIndex = 0;
  pre.textContent = new TextDecoder("utf-8").decode(bytes);
  figure.append(pre);
  if (context.details.size > TEXT_PREVIEW_BYTES) {
    const size = formatSize(TEXT_PREVIEW_BYTES, context.labels);
    figure.append(paragraph("note", formatLabel(context.labels.fileTextTruncated, { size })));
  }
  return figure;
});

/**
 * The built-in renderers: image, video, audio and text.
 *
 * @returns A new array, in the order they are tried.
 */
export function builtInRenderers(): PreviewRenderer[] {
  return [IMAGE, VIDEO, AUDIO, TEXT];
}
