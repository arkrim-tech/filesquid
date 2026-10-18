/**
 * UI-S-016: `accessibilityViolations` finds positive tabindex, unnamed controls, images
 * without a text alternative and unnamed landmarks, through open shadow roots.
 */
import { describe, expect, it } from "vitest";
import { accessibilityViolations, accessibleName } from "../../src/a11y/audit";
import * as entry from "../../src/index";

/** Build a detached container from markup. */
function html(markup: string): HTMLElement {
  const root = document.createElement("div");
  root.innerHTML = markup;
  return root;
}

describe("accessibleName", () => {
  it("prefers aria-labelledby, then aria-label, then a label, then alt, then text", () => {
    const root = html(`
      <h2 id="h">Journal</h2>
      <div id="a" aria-labelledby="h" aria-label="ignored"></div>
      <button id="b" aria-label="Close">x</button>
      <label for="c">Token</label><input id="c">
      <label>Theme <input id="d" type="radio"></label>
      <img id="e" alt="FileSquid logo">
      <a id="f" href="#/monitor"> Monitor </a>`);
    document.body.replaceChildren(root);
    const name = (id: string) => accessibleName(root.querySelector(`#${id}`) as HTMLElement);
    expect(["a", "b", "c", "d", "e", "f"].map(name)).toEqual([
      "Journal",
      "Close",
      "Token",
      "Theme",
      "FileSquid logo",
      "Monitor",
    ]);
  });
  it("resolves aria-labelledby inside the element's own shadow root", () => {
    const host = document.createElement("div");
    const shadow = host.attachShadow({ mode: "open" });
    shadow.innerHTML = `<h2 id="t">Processes</h2><div role="region" aria-labelledby="t"></div>`;
    expect(accessibleName(shadow.querySelector("[role=region]") as HTMLElement)).toBe("Processes");
  });
});

describe("accessibilityViolations", () => {
  it("passes named native controls, images with alt and a named nav", () => {
    const root = html(`
      <button type="button">Skip to main content</button>
      <img alt="">
      <nav aria-label="Main navigation"><a href="#/monitor">Monitor</a></nav>
      <main tabindex="-1"></main>`);
    expect(accessibilityViolations(root)).toEqual([]);
  });
  it("reports each kind of violation with its rule and element", () => {
    const root = html(`
      <div tabindex="2">Focus trap</div>
      <button type="button"></button>
      <a href="#/x"><span></span></a>
      <input type="text">
      <img src="/logo.svg">
      <nav></nav>
      <div role="region"></div>`);
    expect(accessibilityViolations(root)).toEqual([
      { rule: "positive-tabindex", element: "div" },
      { rule: "unnamed-control", element: "button" },
      { rule: "unnamed-control", element: "a" },
      { rule: "unnamed-control", element: "input" },
      { rule: "missing-alt", element: "img" },
      { rule: "unnamed-landmark", element: "nav" },
      { rule: "unnamed-landmark", element: "div" },
    ]);
  });
  it("ignores hidden and disabled controls and links without href", () => {
    const root = html(`
      <input type="hidden"><button type="button" disabled></button>
      <a>plain</a><div hidden><button type="button"></button></div>`);
    expect(accessibilityViolations(root)).toEqual([]);
  });
  it("looks inside open shadow roots, nested", () => {
    const outer = document.createElement("div");
    const shadow = outer.attachShadow({ mode: "open" });
    const inner = document.createElement("span");
    shadow.append(inner);
    inner.attachShadow({ mode: "open" }).innerHTML = `<button type="button"></button>`;
    expect(accessibilityViolations(outer)).toEqual([
      { rule: "unnamed-control", element: "button" },
    ]);
  });
  it("is exported from @filesquid/ui for distributions' own tests", () => {
    expect(entry.accessibilityViolations).toBe(accessibilityViolations);
  });
});
