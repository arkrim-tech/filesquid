/**
 * Preview renderers of the file browser (UI-F2.2, ADR 028 Decision 6): the contract a renderer
 * implements, the built-in renderers (image, video and audio through content handles; text
 * through a ranged read) and the choice among them. A distribution registers its own renderer
 * and it is tried before the built-ins; a file no renderer accepts gets the unsupported
 * fallback in `<fs-file-preview>`, never a broken viewer.
 *
 * The built-in media renderers set a content-handle URL only as the `src` of a media element
 * (UI-S-014); the byte access a renderer gets is the host's {@link FileSource}.
 */
import type { LabelTable } from "../theme/labels";
import { formatLabel } from "../theme/labels";
import { MediaHandleController } from "./media-handle";
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
 * can reveal. The wrapper is a `figure`; its `data-state` follows the controller.
 */
function mediaFigure(element: HTMLElement, context: PreviewContext): HTMLElement {
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
      figure.dispatchEvent(new CustomEvent(MEDIA_STATE_EVENT, { detail: state, bubbles: true }));
    },
  );
  context.onDispose(() => controller.dispose());
  void controller.start();
  return figure;
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

/** The built-in image renderer: a native `<img>` over a content handle. */
const IMAGE = kindRenderer("image", (context) => {
  const img = document.createElement("img");
  img.alt = context.file.display_name;
  return mediaFigure(img, context);
});

/** The built-in video renderer. */
const VIDEO = kindRenderer("video", (context) => timed("video", context));

/** The built-in audio renderer. */
const AUDIO = kindRenderer("audio", (context) => timed("audio", context));

/** The built-in text renderer: the first {@link TEXT_PREVIEW_BYTES} bytes through one ranged read. */
const TEXT = kindRenderer("text", async (context) => {
  const range = { start: 0, end: TEXT_PREVIEW_BYTES - 1 };
  const bytes = await context.source.readRange(context.file, range, { signal: context.signal });
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
