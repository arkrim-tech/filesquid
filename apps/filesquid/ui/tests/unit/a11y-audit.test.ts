/**
 * UI-S-016: `accessibilityViolations` finds positive tabindex, unnamed controls, images
 * without a text alternative and unnamed landmarks, through open shadow roots.
 */
import { describe, expect, it } from "vitest";
import { accessibilityViolations, accessibleName, effectiveRole } from "../../src/a11y/audit";
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

describe("accessibleName and the audit follow the accname rules (review round 1)", () => {
  /** Name of the first element matching `selector` in `markup`. */
  const nameOf = (markup: string, selector: string) =>
    accessibleName(html(markup).querySelector(selector) as Element);

  it("checks name-required widget roles: switch, combobox, slider, menuitemcheckbox and more", () => {
    const roles = ["switch", "combobox", "slider", "menuitemcheckbox", "spinbutton", "textbox"];
    const root = html(roles.map((role) => `<div role="${role}" tabindex="0"></div>`).join(""));
    expect(accessibilityViolations(root)).toEqual(
      roles.map(() => ({ rule: "unnamed-control", element: "div" })),
    );
  });
  it("counts alt as a name only on img and image inputs", () => {
    expect(nameOf(`<button type="button" alt="Close"></button>`, "button")).toBe("");
    expect(nameOf(`<input type="image" alt="Search" src="/s.svg">`, "input")).toBe("Search");
    expect(nameOf(`<img alt="Logo">`, "img")).toBe("Logo");
  });
  it("counts a descendant image's alt toward a link or button name", () => {
    expect(nameOf(`<a href="/"><img alt="FileSquid home"></a>`, "a")).toBe("FileSquid home");
    expect(nameOf(`<button type="button">Open <img alt="menu"></button>`, "button")).toBe(
      "Open menu",
    );
    expect(accessibilityViolations(html(`<a href="/"><img alt="Home"></a>`))).toEqual([]);
  });
  it("names button, submit and reset inputs by their value or the default label", () => {
    expect(nameOf(`<input type="submit" value="Sign in">`, "input")).toBe("Sign in");
    expect(nameOf(`<input type="submit">`, "input")).toBe("Submit");
    expect(nameOf(`<input type="reset">`, "input")).toBe("Reset");
    expect(nameOf(`<input type="button" value="Go">`, "input")).toBe("Go");
    expect(nameOf(`<input type="text" value="typed">`, "input")).toBe("");
  });
  it("never names select or textarea from their content", () => {
    const root = html(`<select><option>Light</option></select><textarea>draft</textarea>`);
    expect(accessibilityViolations(root)).toEqual([
      { rule: "unnamed-control", element: "select" },
      { rule: "unnamed-control", element: "textarea" },
    ]);
  });
  it("does not name a slider or textbox role from its content", () => {
    expect(nameOf(`<div role="slider">50</div>`, "div")).toBe("");
    expect(nameOf(`<div role="switch">Wi-Fi</div>`, "div")).toBe("Wi-Fi");
  });
  it("skips controls disabled through a fieldset or inside an inert subtree", () => {
    const root = html(`
      <fieldset disabled><button type="button"></button><input></fieldset>
      <div inert><a href="/x"></a></div>`);
    expect(accessibilityViolations(root)).toEqual([]);
  });
  it("groups by rule across shadow roots, the root's own tree first", () => {
    const outer = html(`<img src="/a.svg"><span></span>`);
    const shadow = (outer.querySelector("span") as HTMLElement).attachShadow({ mode: "open" });
    shadow.innerHTML = `<button type="button"></button><img src="/b.svg">`;
    outer.prepend(document.createElement("i"));
    outer.insertAdjacentHTML("beforeend", `<input>`);
    expect(accessibilityViolations(outer)).toEqual([
      { rule: "unnamed-control", element: "input" },
      { rule: "unnamed-control", element: "button" },
      { rule: "missing-alt", element: "img" },
      { rule: "missing-alt", element: "img" },
    ]);
  });
});

describe("role tokens and content text (review round 2)", () => {
  it("uses the first token of a fallback role list as the effective role", () => {
    const root = html(`
      <div role="switch checkbox" tabindex="0"></div>
      <div role=" Slider  textbox " tabindex="0">50</div>
      <div role="presentation button"></div>`);
    const [first, second, third] = [...root.querySelectorAll("div")];
    expect([first, second, third].map((el) => effectiveRole(el as Element))).toEqual([
      "switch",
      "slider",
      "presentation",
    ]);
    expect(accessibilityViolations(root)).toEqual([
      { rule: "unnamed-control", element: "div" },
      { rule: "unnamed-control", element: "div" },
    ]);
  });
  it("applies the content-name rule to the effective role only", () => {
    const root = html(`<div role="switch checkbox">Wi-Fi</div><div role="slider switch">50</div>`);
    const [named, slider] = [...root.querySelectorAll("div")];
    expect(accessibleName(named as Element)).toBe("Wi-Fi");
    expect(accessibleName(slider as Element)).toBe("");
  });
  it("finds a landmark by its effective role", () => {
    expect(accessibilityViolations(html(`<div role="region group"></div>`))).toEqual([
      { rule: "unnamed-landmark", element: "div" },
    ]);
  });
  it("takes content names from text nodes only, never from comments", () => {
    const root = html(
      `<button type="button"><!-- icon only --></button><a href="/x">Go<!-- x --></a>`,
    );
    expect(accessibleName(root.querySelector("button") as Element)).toBe("");
    expect(accessibleName(root.querySelector("a") as Element)).toBe("Go");
    expect(accessibilityViolations(root)).toEqual([{ rule: "unnamed-control", element: "button" }]);
  });
});
