/**
 * UI tests for `<fs-file-preview>` (UI-F2.2): previews through content handles (image, video,
 * audio), a text preview through one ranged read of 64 KiB, the unsupported fallback, custom
 * renderer slots, the five view states, and the content-handle rules of UI-S-014: the URL is
 * only a media `src`, a media error mints one new handle, a second failure shows the error.
 */
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import {
  accessibilityViolations,
  defineElements,
  type FsFilePreview,
  type PreviewRenderer,
} from "../../src/index";
import { MemoryFileSource } from "../../src/files/memory-source";
import { TEXT_PREVIEW_BYTES, type FileRef } from "../../src/files/model";
import labels from "../../src/theme/labels.en.json";

beforeEach(() => defineElements());

/** Let queued promise callbacks run. */
const settle = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

const BIG_TEXT = "x".repeat(TEXT_PREVIEW_BYTES + 100);

/** A source holding one file of each kind. */
function source(): MemoryFileSource {
  return new MemoryFileSource({
    library_id: "lib",
    object_id: "obj",
    files: [
      { path: "0a1b2c3d.png", displayName: "facade-study.png", content: "png" },
      { path: "0a1b2c3e.mp4", displayName: "site-walkthrough.mp4", content: "mp4" },
      { path: "0a1b2c3f.wav", displayName: "interview.wav", content: "wav" },
      { path: "0a1b2c40.txt", displayName: "readme.txt", content: "hello world" },
      { path: "0a1b2c41.txt", displayName: "big.txt", content: BIG_TEXT },
      { path: "0a1b2c42.blend", displayName: "model-export.blend", content: "blend" },
    ],
  });
}

/** A file reference by stored path and display name. */
function ref(path: string, display_name: string): FileRef {
  return { library_id: "lib", object_id: "obj", version: "v1", path, display_name };
}

/** Mount a preview over `src` showing `file`. */
async function mount(
  src: MemoryFileSource,
  file: FileRef | null,
  renderers: PreviewRenderer[] = [],
): Promise<FsFilePreview> {
  const preview = document.createElement("fs-file-preview") as FsFilePreview;
  preview.configure({ labels, source: src, renderers });
  document.body.replaceChildren(preview);
  preview.file = file;
  await settle();
  return preview;
}

describe("<fs-file-preview>", () => {
  it("is a defined custom element and shows nothing without a file", async () => {
    expect(customElements.get("fs-file-preview")).toBeDefined();
    const preview = await mount(source(), null);
    expect(preview.dataset.state).toBe("empty");
    expect(preview.shadowRoot?.querySelector("img, video, audio, pre")).toBeNull();
  });

  it("previews an image through one content handle used only as its src", async () => {
    const src = source();
    const preview = await mount(src, ref("0a1b2c3d.png", "facade-study.png"));
    const img = preview.shadowRoot?.querySelector("img");
    expect(img?.getAttribute("src")).toBe(src.mints[0]?.url);
    expect(img?.alt).toBe("facade-study.png");
    expect(src.mints).toHaveLength(1);
    expect(preview.dataset.state).toBe("ready");
    expect(accessibilityViolations(preview)).toEqual([]);
  });

  it("mints nothing when the details arrive after the preview left the page", async () => {
    const src = source();
    const real = src.details.bind(src);
    let release: () => void = () => undefined;
    const gate = new Promise<void>((resolve) => (release = resolve));
    vi.spyOn(src, "details").mockImplementation(async (...args) => {
      await gate;
      return real(...args);
    });
    const mint = vi.spyOn(src, "mintContent");
    const preview = await mount(src, ref("0a1b2c3d.png", "facade-study.png"));
    preview.remove();
    release();
    await settle();
    await settle();
    expect(mint).not.toHaveBeenCalled();
  });

  it("never shows the stored name, the handle URL or a link to it", async () => {
    const src = source();
    const preview = await mount(src, ref("0a1b2c3d.png", "facade-study.png"));
    const root = preview.shadowRoot as ShadowRoot;
    const url = src.mints[0]?.url as string;
    expect(root.textContent).not.toContain("0a1b2c3d");
    expect(root.textContent).not.toContain(url);
    expect(root.querySelector("a")).toBeNull();
    expect(root.innerHTML.split(url)).toHaveLength(2);
    expect(root.querySelector(`[src="${url}"]`)?.tagName).toBe("IMG");
    expect(window.location.href).not.toContain("memory");
  });

  it("plays a video and an audio file natively, named by the display name", async () => {
    const src = source();
    const video = await mount(src, ref("0a1b2c3e.mp4", "site-walkthrough.mp4"));
    const player = video.shadowRoot?.querySelector("video");
    expect(player?.controls).toBe(true);
    expect(player?.getAttribute("aria-label")).toBe("site-walkthrough.mp4");
    expect(player?.getAttribute("src")).toBe(src.mints[0]?.url);
    const audio = await mount(src, ref("0a1b2c3f.wav", "interview.wav"));
    const sound = audio.shadowRoot?.querySelector("audio");
    expect(sound?.controls).toBe(true);
    expect(sound?.getAttribute("aria-label")).toBe("interview.wav");
    expect(accessibilityViolations(audio)).toEqual([]);
  });

  it("reads text through one ranged read of the first 64 KiB", async () => {
    const src = source();
    const read = vi.spyOn(src, "readRange");
    const preview = await mount(src, ref("0a1b2c40.txt", "readme.txt"));
    expect(read).toHaveBeenCalledTimes(1);
    expect(read.mock.calls[0]?.[1]).toEqual({ start: 0, end: TEXT_PREVIEW_BYTES - 1 });
    const pre = preview.shadowRoot?.querySelector("pre");
    expect(pre?.textContent).toBe("hello world");
    expect(pre?.getAttribute("aria-label")).toBe("readme.txt");
    expect(preview.shadowRoot?.querySelector(".note")).toBeNull();
    expect(src.mints).toHaveLength(0);
  });

  it("says when a text file is longer than the part shown", async () => {
    const preview = await mount(source(), ref("0a1b2c41.txt", "big.txt"));
    expect(preview.shadowRoot?.querySelector("pre")?.textContent).toHaveLength(TEXT_PREVIEW_BYTES);
    expect(preview.shadowRoot?.querySelector(".note")?.textContent).toBe(
      "Showing the first 64 KB only",
    );
  });

  it("says there is no preview for a format it cannot show, without a viewer", async () => {
    const preview = await mount(source(), ref("0a1b2c42.blend", "model-export.blend"));
    expect(preview.dataset.state).toBe("unsupported");
    const root = preview.shadowRoot as ShadowRoot;
    expect(root.querySelector(".unsupported .title")?.textContent).toBe(labels.fileNoPreview);
    expect(root.querySelector(".unsupported .help")?.textContent).toBe(labels.fileNoPreviewHelp);
    expect(root.querySelector("img, video, audio, pre, iframe, object, embed")).toBeNull();
    expect(root.textContent).not.toContain("0a1b2c42");
  });

  it("lets a registered renderer take a file before the built-ins", async () => {
    const custom: PreviewRenderer = {
      id: "custom-png",
      accepts: (details) => details.media_type === "image/png",
      render: (context) => {
        const el = document.createElement("div");
        el.className = "custom";
        el.textContent = context.file.display_name;
        return el;
      },
    };
    const src = source();
    const preview = await mount(src, ref("0a1b2c3d.png", "facade-study.png"), [custom]);
    expect(preview.shadowRoot?.querySelector(".custom")?.textContent).toBe("facade-study.png");
    expect(preview.shadowRoot?.querySelector("img")).toBeNull();
    expect(src.mints).toHaveLength(0);
  });

  it("shows the error state with Try again when the details cannot be read, and retries", async () => {
    const src = source();
    src.failure = new Error("offline-ish");
    const preview = await mount(src, ref("0a1b2c3d.png", "facade-study.png"));
    expect(preview.dataset.state).toBe("error");
    const states = preview.shadowRoot?.querySelector("fs-states") as HTMLElement;
    expect(states.dataset.state).toBe("error");
    src.failure = undefined;
    states.dispatchEvent(new CustomEvent("fs-retry", { bubbles: true, composed: true }));
    await settle();
    expect(preview.dataset.state).toBe("ready");
  });

  it("shows permission denied with the reason's label", async () => {
    const src = source();
    src.failure = new ApiError(403, "/api/v1/files/details", {
      code: "forbidden",
      reason: "not_permitted",
    });
    const preview = await mount(src, ref("0a1b2c3d.png", "facade-study.png"));
    expect(preview.dataset.state).toBe("denied");
    const states = preview.shadowRoot?.querySelector("fs-states") as HTMLElement;
    expect(states.shadowRoot?.querySelector(".detail")?.textContent).toBe(labels.authNotPermitted);
  });

  it("shows an empty text file as empty without reading a range", async () => {
    const src = new MemoryFileSource({
      library_id: "lib",
      object_id: "obj",
      files: [{ path: "empty.txt", content: "" }],
    });
    const read = vi.spyOn(src, "readRange");
    const preview = await mount(src, ref("empty.txt", "empty.txt"));
    expect(read).not.toHaveBeenCalled();
    expect(preview.dataset.state).toBe("ready");
    expect(preview.shadowRoot?.querySelector("pre")?.textContent).toBe("");
  });

  it("shows the file again when the element is put back into the document", async () => {
    const preview = await mount(source(), ref("0a1b2c40.txt", "readme.txt"));
    preview.remove();
    expect(preview.shadowRoot?.querySelector("pre")).not.toBeNull();
    document.body.append(preview);
    await settle();
    expect(preview.shadowRoot?.querySelector("pre")?.textContent).toBe("hello world");
  });

  it("mints one new handle on a media error and shows the error if that fails too", async () => {
    const src = source();
    const preview = await mount(src, ref("0a1b2c3d.png", "facade-study.png"));
    const img = preview.shadowRoot?.querySelector("img") as HTMLImageElement;
    img.dispatchEvent(new Event("error"));
    await settle();
    expect(src.mints).toHaveLength(2);
    expect(img.getAttribute("src")).toBe(src.mints[1]?.url);
    img.dispatchEvent(new Event("error"));
    await settle();
    expect(src.mints).toHaveLength(2);
    expect(preview.dataset.state).toBe("error");
    const alert = preview.shadowRoot?.querySelector("figure [role='alert']") as HTMLElement;
    expect(alert.hidden).toBe(false);
    expect(alert.textContent).toBe(labels.filePreviewError);
    expect(img.hidden).toBe(true);
  });

  it("drops the previous preview and stops its work when another file is shown", async () => {
    const src = source();
    const preview = await mount(src, ref("0a1b2c3d.png", "facade-study.png"));
    const img = preview.shadowRoot?.querySelector("img") as HTMLImageElement;
    preview.file = ref("0a1b2c40.txt", "readme.txt");
    await settle();
    expect(preview.shadowRoot?.querySelector("img")).toBeNull();
    img.dispatchEvent(new Event("error"));
    await settle();
    expect(src.mints).toHaveLength(1);
    preview.file = null;
    await settle();
    expect(preview.shadowRoot?.querySelector("pre")).toBeNull();
  });

  it("ignores a slow answer for a file that is no longer shown", async () => {
    const src = source();
    const real = src.details.bind(src);
    let release: () => void = () => undefined;
    vi.spyOn(src, "details").mockImplementationOnce(async (file) => {
      await new Promise<void>((resolve) => (release = resolve));
      return real(file);
    });
    const preview = await mount(src, ref("0a1b2c3d.png", "facade-study.png"));
    preview.file = ref("0a1b2c40.txt", "readme.txt");
    await settle();
    release();
    await settle();
    expect(preview.shadowRoot?.querySelector("img")).toBeNull();
    expect(preview.shadowRoot?.querySelector("pre")?.textContent).toBe("hello world");
  });
});
