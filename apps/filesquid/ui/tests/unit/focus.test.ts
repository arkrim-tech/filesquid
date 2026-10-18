/**
 * UI-S-016 focus recovery: the deep active element is found through shadow roots, a target
 * that is not focusable is made programmatically focusable, and a re-render that removes the
 * focused element hands focus to a defined target instead of leaving it on <body>.
 */
import { describe, expect, it } from "vitest";
import {
  deepActiveElement,
  focusElement,
  recoverFocus,
  replaceKeepingFocus,
} from "../../src/a11y/focus";

/** A host element in the document with an open shadow root holding `markup`. */
function shadowHost(markup: string): ShadowRoot {
  const host = document.createElement("div");
  document.body.replaceChildren(host);
  const root = host.attachShadow({ mode: "open" });
  root.innerHTML = markup;
  return root;
}

describe("deepActiveElement", () => {
  it("follows focus into nested shadow roots", () => {
    const root = shadowHost(`<span></span>`);
    const inner = (root.querySelector("span") as HTMLElement).attachShadow({ mode: "open" });
    inner.innerHTML = `<button type="button">Go</button>`;
    (inner.querySelector("button") as HTMLButtonElement).focus();
    expect(deepActiveElement(document)?.textContent).toBe("Go");
  });
  it("returns body when nothing has focus", () => {
    document.body.replaceChildren();
    expect(deepActiveElement(document)).toBe(document.body);
  });
});

describe("focusElement", () => {
  it("makes a heading programmatically focusable without adding it to the Tab order", () => {
    const root = shadowHost(`<h2>Health</h2>`);
    const heading = root.querySelector("h2") as HTMLElement;
    focusElement(heading);
    expect(heading.getAttribute("tabindex")).toBe("-1");
    expect(root.activeElement).toBe(heading);
  });
  it("leaves a native control's tabindex alone", () => {
    const root = shadowHost(`<input id="t">`);
    const input = root.querySelector("input") as HTMLInputElement;
    focusElement(input);
    expect(input.hasAttribute("tabindex")).toBe(false);
    expect(root.activeElement).toBe(input);
  });
});

describe("replaceKeepingFocus", () => {
  it("moves focus to the target when the focused element is replaced", () => {
    const root = shadowHost(`<button type="button">Search</button>`);
    (root.querySelector("button") as HTMLButtonElement).focus();
    const heading = document.createElement("h2");
    heading.textContent = "Health";
    replaceKeepingFocus(root, [heading], () => heading);
    expect(root.activeElement).toBe(heading);
    expect(deepActiveElement(document)).toBe(heading);
  });
  it("does not take focus when focus was elsewhere (first load)", () => {
    const root = shadowHost(`<p>Loading</p>`);
    const outside = document.createElement("button");
    document.body.append(outside);
    outside.focus();
    const heading = document.createElement("h2");
    replaceKeepingFocus(root, [heading], () => heading);
    expect(document.activeElement).toBe(outside);
    expect(heading.hasAttribute("tabindex")).toBe(false);
  });
  it("gives focus back to a focused element that is re-inserted, the way a browser drops it", () => {
    const root = shadowHost(`<input>`);
    const input = root.querySelector("input") as HTMLInputElement;
    input.focus();
    const note = document.createElement("p");
    replaceKeepingFocus(root, [note, input], () => input);
    expect(root.activeElement).toBe(input);
    expect(input.hasAttribute("tabindex")).toBe(false);
  });
  it("leaves focus unset when the target is missing", () => {
    const root = shadowHost(`<button type="button">Go</button>`);
    (root.querySelector("button") as HTMLButtonElement).focus();
    replaceKeepingFocus(root, [], () => null);
    expect(root.activeElement).toBeNull();
  });
});

describe("focusElement leaves natively focusable elements in the Tab order (review round 1)", () => {
  it.each([
    ["details summary", `<details><summary>More</summary><p>x</p></details>`, "summary"],
    ["editing host", `<div contenteditable="true">Note</div>`, "div"],
    ["link", `<a href="#/x">Go</a>`, "a"],
    ["select", `<select><option>A</option></select>`, "select"],
  ])("does not add tabindex to a %s", (_name, markup, selector) => {
    const root = shadowHost(markup);
    const element = root.querySelector(selector) as HTMLElement;
    focusElement(element);
    expect(element.hasAttribute("tabindex")).toBe(false);
  });
  it("still makes a plain div programmatically focusable", () => {
    const root = shadowHost(`<div contenteditable="false">Static</div>`);
    const element = root.querySelector("div") as HTMLElement;
    focusElement(element);
    expect(element.getAttribute("tabindex")).toBe("-1");
  });
});

describe("recoverFocus", () => {
  it("focuses the target when focus was inside and is now lost", () => {
    const root = shadowHost(`<button type="button">Old</button>`);
    (root.querySelector("button") as HTMLButtonElement).focus();
    root.innerHTML = `<h2>New</h2>`;
    recoverFocus(root, true, () => root.querySelector("h2"));
    expect(root.activeElement?.textContent).toBe("New");
  });
  it("does nothing when focus was not inside before the re-render", () => {
    const root = shadowHost(`<h2>New</h2>`);
    recoverFocus(root, false, () => root.querySelector("h2"));
    expect(root.activeElement).toBeNull();
    expect(root.querySelector("h2")?.hasAttribute("tabindex")).toBe(false);
  });
  it("does nothing when focus is still inside", () => {
    const root = shadowHost(`<input><h2>New</h2>`);
    const input = root.querySelector("input") as HTMLInputElement;
    input.focus();
    recoverFocus(root, true, () => root.querySelector("h2"));
    expect(root.activeElement).toBe(input);
  });
});
