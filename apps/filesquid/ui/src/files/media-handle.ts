/**
 * Content-handle playback for `<img>`, `<video>` and `<audio>` (UI-S-014, UI-F2.2). A media
 * element cannot send `Authorization`, so it reads through a content handle: a short-lived URL
 * minted with an authenticated request. The controller mints the handle, sets it as the
 * element's `src` and nothing else, and when the element reports an error it mints a new handle
 * at most once for that error and resumes at the same position and paused state; if the new
 * handle fails too, it reports the error. A successful load re-arms the single re-mint.
 *
 * The URL is a credential: it is never logged, rendered, stored or put in an error here, and
 * the messages this module logs name only what happened.
 */

/** What the viewer of the media should show. */
export type MediaState = "loading" | "ready" | "error";

/** Re-mints one controller may make over its whole life, however often the element recovers. */
export const MAX_REMINTS = 20;

/** An element a handle can feed. */
export type MediaElement = HTMLImageElement | HTMLMediaElement;

/** Mints a new handle URL. Network I/O; rejects when the mint is refused. */
export type MintUrl = () => Promise<string>;

/** True for an audio or video element, which has a position and a paused state. */
function isTimed(element: MediaElement): element is HTMLMediaElement {
  return "currentTime" in element && "paused" in element;
}

/** Position and paused state to restore after a re-mint. */
interface Resume {
  readonly position: number;
  readonly playing: boolean;
}

/** Feeds one media element from content handles. */
export class MediaHandleController {
  private armed = true;
  private reminted = 0;
  private disposed = false;
  private onMetadata?: () => void;
  private readonly onError = () => void this.recover();
  private readonly onLoad = () => this.loaded();

  /**
   * @param element The `<img>`, `<video>` or `<audio>` to feed.
   * @param mint Mints a handle URL; called once to start and once per recovery.
   * @param onState Called with `loading` when starting, `ready` when the element has data and
   *   `error` when playback cannot be recovered.
   */
  constructor(
    private readonly element: MediaElement,
    private readonly mint: MintUrl,
    private readonly onState: (state: MediaState) => void,
  ) {}

  /**
   * Mint the first handle and attach it. Side effects: network call through `mint`, sets the
   * element's `src` and adds its listeners. A refused mint reports `error`. A controller that
   * was disposed before it started does nothing.
   *
   * @returns Resolves once the source is set or the error is reported.
   */
  async start(): Promise<void> {
    if (this.disposed) return;
    this.onState("loading");
    this.element.addEventListener("error", this.onError);
    this.element.addEventListener(isTimed(this.element) ? "loadeddata" : "load", this.onLoad);
    const url = await this.mintOrNull();
    if (this.disposed) return;
    if (url === null) return this.onState("error");
    this.element.setAttribute("src", url);
  }

  /**
   * Detach: remove the listeners, stop playback, drop the source and ignore any mint still in
   * flight. Idempotent.
   */
  dispose(): void {
    this.disposed = true;
    if (this.onMetadata) this.element.removeEventListener("loadedmetadata", this.onMetadata);
    this.onMetadata = undefined;
    if (isTimed(this.element) && !this.element.paused) this.element.pause();
    this.element.removeAttribute("src");
    this.element.removeEventListener("error", this.onError);
    this.element.removeEventListener("load", this.onLoad);
    this.element.removeEventListener("loadeddata", this.onLoad);
  }

  /** The element has data: the next error may be recovered once more. */
  private loaded(): void {
    this.armed = true;
    this.onState("ready");
  }

  /** Mint a handle, or null (logged without the URL) when the mint fails. */
  private async mintOrNull(): Promise<string | null> {
    try {
      return await this.mint();
    } catch (error) {
      console.warn("FileSquid could not mint a content handle", errorKind(error));
      return null;
    }
  }

  /** Answer a media error: one new handle for this error, else the error state. */
  private async recover(): Promise<void> {
    if (this.disposed) return;
    if (!this.armed || this.reminted >= MAX_REMINTS) return this.onState("error");
    this.armed = false;
    this.reminted += 1;
    const resume = this.snapshot();
    const url = await this.mintOrNull();
    if (this.disposed) return;
    if (url === null) return this.onState("error");
    this.attach(url, resume);
  }

  /** The position and paused state to come back to. */
  private snapshot(): Resume {
    const el = this.element;
    return isTimed(el)
      ? { position: el.currentTime, playing: !el.paused }
      : { position: 0, playing: false };
  }

  /** Set the new source and, for timed media, resume once its metadata is known. */
  private attach(url: string, resume: Resume): void {
    const el = this.element;
    if (isTimed(el)) {
      if (this.onMetadata) el.removeEventListener("loadedmetadata", this.onMetadata);
      this.onMetadata = () => void this.resume(el, resume);
      el.addEventListener("loadedmetadata", this.onMetadata, { once: true });
    }
    el.setAttribute("src", url);
  }

  /** Seek back to the saved position and play again only if it was playing. */
  private async resume(el: HTMLMediaElement, resume: Resume): Promise<void> {
    if (this.disposed) return;
    el.currentTime = resume.position;
    if (!resume.playing) return;
    try {
      await el.play();
    } catch (error) {
      console.warn("FileSquid could not resume playback", errorKind(error));
    }
  }
}

/** The kind of an error, without its message (which could quote a URL). */
function errorKind(error: unknown): string {
  return error instanceof Error ? error.name : typeof error;
}
