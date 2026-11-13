/**
 * The viewport of the virtualised navigation tree (Task UI-F3.4, UI-S-035): the part of each
 * section's tree that the window and every clipping ancestor leave in view, and the scrolls and
 * size changes the tree follows. jsdom has no layout, so the tests state the boxes themselves.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { TREE_ROW_REM, domViewport } from "../../src/components/nav-tree-viewport";

/** Give an element a box: its top edge, its inner height and, optionally, its border. */
function box(element: Element, top: number, height: number, border = 0): void {
  vi.spyOn(element, "getBoundingClientRect").mockReturnValue({
    top,
    bottom: top + height + 2 * border,
    left: 0,
    right: 0,
    width: 0,
    height: height + 2 * border,
    x: 0,
    y: top,
    toJSON: () => ({}),
  });
  Object.defineProperty(element, "clientHeight", { configurable: true, value: height });
  Object.defineProperty(element, "clientTop", { configurable: true, value: border });
}

/** A scrolling area in the body with the tree host and two lists inside it. */
function mount(overflow = "auto") {
  const scroller = document.createElement("div");
  scroller.style.overflowY = overflow;
  const host = document.createElement("div");
  const first = document.createElement("ul");
  const second = document.createElement("ul");
  host.append(first, second);
  scroller.append(host);
  document.body.replaceChildren(scroller);
  return { scroller, host, first, second };
}

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});

describe("domViewport", () => {
  it("has rows of 1.875 rem: 30 px at the default text size", () => {
    expect(TREE_ROW_REM).toBe(1.875);
    expect(domViewport(document.body).rowHeight()).toBe(30);
  });

  it("scales the row with the root font size, as text scaling does", () => {
    vi.spyOn(window, "getComputedStyle").mockReturnValue({
      fontSize: "32px",
    } as CSSStyleDeclaration);
    expect(domViewport(document.body).rowHeight()).toBe(60);
  });

  it("answers the part of each list inside the scrolling area, from the list's own top", () => {
    const { scroller, host, first, second } = mount();
    vi.stubGlobal("innerHeight", 900);
    box(scroller, 200, 500);
    // The first list starts 1,000 px above the area's top; the second 300 px below its top.
    box(first, -800, 3000);
    box(second, 500, 3000);
    expect(domViewport(host).bands([first, second])).toEqual([
      { top: 1000, bottom: 1500 },
      { top: -300, bottom: 200 },
    ]);
  });

  it("starts the view inside the border of a clipping ancestor", () => {
    const { scroller, host, first } = mount();
    vi.stubGlobal("innerHeight", 900);
    box(scroller, 200, 500, 4);
    box(first, 204, 3000);
    expect(domViewport(host).bands([first])).toEqual([{ top: 0, bottom: 500 }]);
  });

  it("is cut by the window when the scrolling area reaches past it", () => {
    const { scroller, host, first } = mount();
    vi.stubGlobal("innerHeight", 600);
    box(scroller, 100, 2000);
    box(first, 100, 300_000);
    expect(domViewport(host).bands([first])).toEqual([{ top: 0, bottom: 500 }]);
  });

  it("uses the window alone when no ancestor clips (the page scrolls the tree)", () => {
    const { scroller, host, first } = mount("visible");
    vi.stubGlobal("innerHeight", 600);
    box(scroller, -5000, 300_000);
    box(first, -4000, 300_000);
    expect(domViewport(host).bands([first])).toEqual([{ top: 4000, bottom: 4600 }]);
  });

  it("assumes 20 rows when nothing is measured, so the window stays small", () => {
    const { scroller, host, first } = mount();
    vi.stubGlobal("innerHeight", 600);
    box(scroller, 0, 0);
    box(first, 0, 0);
    expect(domViewport(host).bands([first])).toEqual([{ top: 0, bottom: 600 }]);
  });

  it("sees a clipping ancestor through a slot and a shadow root", () => {
    const shell = document.createElement("div");
    const root = shell.attachShadow({ mode: "open" });
    const area = document.createElement("div");
    area.style.overflowY = "auto";
    const slot = document.createElement("slot");
    slot.name = "navigation";
    area.append(slot);
    root.append(area);
    const host = document.createElement("div");
    host.slot = "navigation";
    const list = document.createElement("ul");
    host.append(list);
    shell.append(host);
    document.body.replaceChildren(shell);
    vi.stubGlobal("innerHeight", 900);
    box(area, 260, 400);
    box(list, 60, 300_000);
    expect(domViewport(host).bands([list])).toEqual([{ top: 200, bottom: 600 }]);
    const changed = vi.fn();
    const stop = domViewport(host).follow(changed);
    area.dispatchEvent(new Event("scroll"));
    expect(changed).toHaveBeenCalledTimes(1);
    stop();
    area.dispatchEvent(new Event("scroll"));
    expect(changed).toHaveBeenCalledTimes(1);
  });

  it("follows the scrolls of its ancestors and the window, and its size, until stopped", () => {
    const { scroller, host } = mount();
    const observed: Element[] = [];
    const disconnect = vi.fn();
    let resize: () => void = () => undefined;
    vi.stubGlobal(
      "ResizeObserver",
      class {
        constructor(callback: () => void) {
          resize = callback;
        }
        observe(element: Element): void {
          observed.push(element);
        }
        disconnect = disconnect;
      },
    );
    const changed = vi.fn();
    const stop = domViewport(host).follow(changed);
    scroller.dispatchEvent(new Event("scroll"));
    window.dispatchEvent(new Event("scroll"));
    window.dispatchEvent(new Event("resize"));
    resize();
    expect(changed).toHaveBeenCalledTimes(4);
    expect(observed).toContain(host);
    expect(observed).toContain(scroller);
    stop();
    scroller.dispatchEvent(new Event("scroll"));
    window.dispatchEvent(new Event("scroll"));
    window.dispatchEvent(new Event("resize"));
    expect(changed).toHaveBeenCalledTimes(4);
    expect(disconnect).toHaveBeenCalledTimes(1);
  });

  it("follows without a ResizeObserver", () => {
    const { scroller, host } = mount();
    vi.stubGlobal("ResizeObserver", undefined);
    const changed = vi.fn();
    const stop = domViewport(host).follow(changed);
    scroller.dispatchEvent(new Event("scroll"));
    expect(changed).toHaveBeenCalledTimes(1);
    stop();
  });
});
