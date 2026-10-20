/**
 * Unit tests for the content-handle media controller (UI-S-014, UI-F2.2): the handle URL is
 * set only as the `src` of the media element; on a media error one new handle is minted and
 * playback resumes at the same position and paused state; a second failure shows the error;
 * a successful load arms the single re-mint again; nothing is logged.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MAX_REMINTS, MediaHandleController, type MediaState } from "../../src/files/media-handle";

/** Let queued promise callbacks run. */
const settle = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

/** A video whose position and paused state the test controls. */
function video(state: { currentTime: number; paused: boolean }): HTMLVideoElement {
  const el = document.createElement("video");
  Object.defineProperty(el, "currentTime", {
    get: () => state.currentTime,
    set: (value: number) => {
      state.currentTime = value;
    },
    configurable: true,
  });
  Object.defineProperty(el, "paused", { get: () => state.paused, configurable: true });
  el.play = vi.fn(async () => {
    state.paused = false;
  });
  return el;
}

/** A minter that answers `handle-1`, `handle-2`, … as URLs, or rejects when told to. */
function minter(fail: number[] = []) {
  let calls = 0;
  const mint = vi.fn(async () => {
    calls += 1;
    if (fail.includes(calls)) throw new Error("mint refused");
    return `http://h/api/v1/content/handle-${calls}`;
  });
  return mint;
}

let logged: unknown[][];
beforeEach(() => {
  logged = [];
  for (const level of ["log", "info", "warn", "error", "debug"] as const) {
    vi.spyOn(console, level).mockImplementation((...args: unknown[]) => void logged.push(args));
  }
});
afterEach(() => vi.restoreAllMocks());

describe("MediaHandleController", () => {
  it("mints once and sets the URL as the media source", async () => {
    const states: MediaState[] = [];
    const el = video({ currentTime: 0, paused: true });
    const mint = minter();
    const controller = new MediaHandleController(el, mint, (s) => states.push(s));
    await controller.start();
    expect(mint).toHaveBeenCalledTimes(1);
    expect(el.getAttribute("src")).toBe("http://h/api/v1/content/handle-1");
    expect(states).toEqual(["loading"]);
    el.dispatchEvent(new Event("loadeddata"));
    expect(states).toEqual(["loading", "ready"]);
  });

  it("on an error mints one new handle and resumes at the same position while playing", async () => {
    const state = { currentTime: 42.5, paused: false };
    const el = video(state);
    const mint = minter();
    const controller = new MediaHandleController(el, mint, () => undefined);
    await controller.start();
    el.dispatchEvent(new Event("error"));
    await settle();
    expect(mint).toHaveBeenCalledTimes(2);
    expect(el.getAttribute("src")).toBe("http://h/api/v1/content/handle-2");
    state.currentTime = 0;
    state.paused = true;
    el.dispatchEvent(new Event("loadedmetadata"));
    await settle();
    expect(state.currentTime).toBe(42.5);
    expect(el.play).toHaveBeenCalledTimes(1);
  });

  it("keeps a paused video paused after the new handle", async () => {
    const state = { currentTime: 10, paused: true };
    const el = video(state);
    const controller = new MediaHandleController(el, minter(), () => undefined);
    await controller.start();
    el.dispatchEvent(new Event("error"));
    await settle();
    state.currentTime = 0;
    el.dispatchEvent(new Event("loadedmetadata"));
    await settle();
    expect(state.currentTime).toBe(10);
    expect(el.play).not.toHaveBeenCalled();
  });

  it("shows the error when the new handle fails too, and mints no third", async () => {
    const states: MediaState[] = [];
    const el = video({ currentTime: 0, paused: true });
    const mint = minter();
    const controller = new MediaHandleController(el, mint, (s) => states.push(s));
    await controller.start();
    el.dispatchEvent(new Event("error"));
    await settle();
    el.dispatchEvent(new Event("error"));
    await settle();
    expect(mint).toHaveBeenCalledTimes(2);
    expect(states.at(-1)).toBe("error");
  });

  it("shows the error when minting the new handle fails", async () => {
    const states: MediaState[] = [];
    const el = video({ currentTime: 0, paused: true });
    const controller = new MediaHandleController(el, minter([2]), (s) => states.push(s));
    await controller.start();
    el.dispatchEvent(new Event("error"));
    await settle();
    expect(states.at(-1)).toBe("error");
  });

  it("shows the error when the first mint fails", async () => {
    const states: MediaState[] = [];
    const el = video({ currentTime: 0, paused: true });
    const controller = new MediaHandleController(el, minter([1]), (s) => states.push(s));
    await controller.start();
    expect(states).toEqual(["loading", "error"]);
    expect(el.hasAttribute("src")).toBe(false);
  });

  it("re-arms the single re-mint after the new handle loads", async () => {
    const el = video({ currentTime: 3, paused: true });
    const mint = minter();
    const controller = new MediaHandleController(el, mint, () => undefined);
    await controller.start();
    el.dispatchEvent(new Event("error"));
    await settle();
    el.dispatchEvent(new Event("loadeddata"));
    el.dispatchEvent(new Event("error"));
    await settle();
    expect(mint).toHaveBeenCalledTimes(3);
  });

  it("works for an image, which has no position", async () => {
    const img = document.createElement("img");
    const mint = minter();
    const controller = new MediaHandleController(img, mint, () => undefined);
    await controller.start();
    img.dispatchEvent(new Event("error"));
    await settle();
    expect(img.getAttribute("src")).toBe("http://h/api/v1/content/handle-2");
    img.dispatchEvent(new Event("error"));
    await settle();
    expect(mint).toHaveBeenCalledTimes(2);
  });

  it("stops after a bounded number of re-mints over its lifetime", async () => {
    const states: MediaState[] = [];
    const el = video({ currentTime: 0, paused: true });
    const mint = minter();
    const controller = new MediaHandleController(el, mint, (s) => states.push(s));
    await controller.start();
    for (let i = 0; i < MAX_REMINTS + 5; i += 1) {
      el.dispatchEvent(new Event("error"));
      await settle();
      el.dispatchEvent(new Event("loadeddata"));
    }
    expect(mint.mock.calls.length).toBeLessThanOrEqual(MAX_REMINTS + 1);
  });

  it("does nothing after dispose, and ignores a mint that finishes late", async () => {
    const el = video({ currentTime: 0, paused: true });
    let release: (url: string) => void = () => undefined;
    const mint = vi.fn(() => new Promise<string>((resolve) => (release = resolve)));
    const controller = new MediaHandleController(el, mint, () => undefined);
    const started = controller.start();
    controller.dispose();
    release("http://h/api/v1/content/late");
    await started;
    expect(el.hasAttribute("src")).toBe(false);
    el.dispatchEvent(new Event("error"));
    await settle();
    expect(mint).toHaveBeenCalledTimes(1);
  });

  it("never writes the handle URL or anything else to the console", async () => {
    const el = video({ currentTime: 1, paused: false });
    const controller = new MediaHandleController(el, minter([2]), () => undefined);
    await controller.start();
    el.dispatchEvent(new Event("error"));
    await settle();
    expect(JSON.stringify(logged)).not.toContain("handle-");
    expect(JSON.stringify(logged)).not.toContain("api/v1/content");
  });
});
