// @vitest-environment node
/**
 * SRS-UI-007: custom elements style colour, type size, spacing and radius only through
 * tokens. UI-S-016: every component includes the shared accessibility stylesheet, uses
 * logical properties only (so `dir="rtl"` mirrors it), takes motion from
 * `--fs-motion-duration`, uses system colours only for forced colours, and fixes no text
 * container to a pixel height. The checks read the component stylesheets (the `STYLE` template of each component),
 * not the rest of the source, so routes such as `#/monitor` or ids are never mistaken for CSS.
 */
import { readFileSync, readdirSync } from "node:fs";
import { describe, expect, it } from "vitest";

const dir = new URL("../../src/components/", import.meta.url);
const sources = readdirSync(dir)
  .filter((name) => name.endsWith(".ts"))
  .map((name) => {
    const source = readFileSync(new URL(name, dir), "utf8");
    return { name, source, css: stylesheet(source) };
  });
/** The components that render a shadow root, and so must include the shared stylesheet. */
const ELEMENTS = [
  "fs-app-shell.ts",
  "fs-breadcrumb.ts",
  "fs-destination-picker.ts",
  "fs-details-panel.ts",
  "fs-drive-view.ts",
  "fs-file-browser.ts",
  "fs-file-panel.ts",
  "fs-file-preview.ts",
  "fs-host-indicator.ts",
  "fs-metadata-panel.ts",
  "fs-provenance-panel.ts",
  "fs-monitor.ts",
  "fs-nav-tree.ts",
  "fs-new-menu.ts",
  "fs-ocfl-findings.ts",
  "fs-settings.ts",
  "fs-states.ts",
  "fs-theme-control.ts",
  "fs-versions-panel.ts",
];
/** The shared accessibility stylesheet. */
const shared = sources.find((s) => s.name === "a11y-style.ts")?.css ?? "";

/** The CSS of a component: the contents of its `const STYLE = \`...\`` template, if any. */
function stylesheet(source: string): string {
  return [...source.matchAll(/const STYLE = `([\s\S]*?)`;/g)].map((m) => m[1]).join("\n");
}

/** Every CSS named colour (CSS Color Module Level 4), plus `transparent`. */
const NAMED_COLOURS =
  `aliceblue antiquewhite aqua aquamarine azure beige bisque black blanchedalmond
blue blueviolet brown burlywood cadetblue chartreuse chocolate coral cornflowerblue cornsilk crimson
cyan darkblue darkcyan darkgoldenrod darkgray darkgreen darkgrey darkkhaki darkmagenta
darkolivegreen darkorange darkorchid darkred darksalmon darkseagreen darkslateblue darkslategray
darkslategrey darkturquoise darkviolet deeppink deepskyblue dimgray dimgrey dodgerblue firebrick
floralwhite forestgreen fuchsia gainsboro ghostwhite gold goldenrod gray green greenyellow grey
honeydew hotpink indianred indigo ivory khaki lavender lavenderblush lawngreen lemonchiffon
lightblue lightcoral lightcyan lightgoldenrodyellow lightgray lightgreen lightgrey lightpink
lightsalmon lightseagreen lightskyblue lightslategray lightslategrey lightsteelblue lightyellow lime
limegreen linen magenta maroon mediumaquamarine mediumblue mediumorchid mediumpurple
mediumseagreen mediumslateblue mediumspringgreen mediumturquoise mediumvioletred midnightblue
mintcream mistyrose moccasin navajowhite navy oldlace olive olivedrab orange orangered orchid
palegoldenrod palegreen paleturquoise palevioletred papayawhip peachpuff peru pink plum
powderblue purple rebeccapurple red rosybrown royalblue saddlebrown salmon sandybrown seagreen
seashell sienna silver skyblue slateblue slategray slategrey snow springgreen steelblue tan teal
thistle tomato turquoise violet wheat white whitesmoke yellow yellowgreen transparent`.split(/\s+/);

/** Literal colour forms a component stylesheet must not use, matched case-insensitively. */
const LITERAL_COLOUR = new RegExp(
  `#[0-9a-f]{3,8}\\b|\\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch|color)\\(|(?<![\\w-])(?:${NAMED_COLOURS.join("|")})(?![\\w-])`,
  "i",
);

/** Properties whose values MUST come from spacing, type-size or radius tokens. */
const SIZED = /^(?:margin|padding|gap|row-gap|column-gap|font-size|border-radius)(?:-[a-z-]+)?$/;

/**
 * Return the spacing, type-size and radius declarations whose values use anything but
 * `var(--fs-*)`, `0` or `auto`.
 */
function untokenisedSizes(css: string): string[] {
  const declarations = [...css.matchAll(/([a-z-]+)\s*:\s*([^;{}]+)/g)];
  return declarations
    .filter(([, property]) => SIZED.test(property ?? ""))
    .filter(([, , value]) => {
      const rest = (value ?? "").replace(/var\(--fs-[a-z0-9-]+\)/g, "").trim();
      return rest.split(/\s+/).some((part) => part !== "" && part !== "0" && part !== "auto");
    })
    .map(([declaration]) => (declaration ?? "").trim());
}

describe("Drive layout constants", () => {
  const css = (name: string) => sources.find((s) => s.name === name)?.css ?? "";
  it("ends the Recent activity text before the actions column (track 10)", () => {
    expect(css("drive-row-view.ts")).toMatch(/\.recent \.row \.activity \{ grid-column: 7 \/ 10;/);
    expect(css("drive-row-view.ts")).toMatch(/\.row \.actions-cell \{ grid-column: 10;/);
  });
  it("keeps the Recent activity text in visible columns on a narrow screen", () => {
    const rowView = css("drive-row-view.ts");
    expect(rowView).toMatch(
      /@media \(max-width: 40rem\)[^@]*\.recent \.row \.activity \{ grid-column: 5 \/ 10;/,
    );
    expect(rowView).toMatch(
      /@media \(max-width: 40rem\)[^@]*\.recent \.row \.collection \{ display: none;/,
    );
  });
  it("keeps the Browsing status note rendered, so a live region announces what is put in it", () => {
    expect(css("settings-browsing.ts")).not.toMatch(/\.pref-note:empty\s*\{\s*display: none/);
    expect(css("settings-browsing.ts")).toMatch(
      /\.pref-note:not\(\.pref-status\):empty \{ display: none/,
    );
  });
  it("lets the Browsing rows shrink and stack so text scaling never scrolls sideways", () => {
    const browsing = css("settings-browsing.ts");
    expect(browsing).toMatch(/grid-template-columns: minmax\(0, 12rem\) minmax\(0, 1fr\)/);
    expect(browsing).toMatch(
      /@media \(max-width: 40rem\) \{ \.pref-row \{ grid-template-columns: minmax\(0, 1fr\)/,
    );
  });
});

describe("component stylesheets", () => {
  it("cover every component source", () => {
    expect(sources.map((s) => s.name)).toEqual(expect.arrayContaining(ELEMENTS));
    for (const name of ELEMENTS) {
      expect(sources.find((s) => s.name === name)?.css).toContain("var(--fs-");
    }
  });
  it.each(sources)("$name contains no literal colour", ({ css }) => {
    expect(css.match(LITERAL_COLOUR)?.[0]).toBeUndefined();
  });
  it.each(sources)("$name takes spacing, type size and radius from tokens", ({ css }) => {
    expect(untokenisedSizes(css)).toEqual([]);
  });
  it("the colour check catches literals in any case and ignores token names", () => {
    expect(LITERAL_COLOUR.test("a { color: #1f5fbf; }")).toBe(true);
    expect(LITERAL_COLOUR.test("a { background: RGB(0 0 0); }")).toBe(true);
    expect(LITERAL_COLOUR.test("a { color: White; }")).toBe(true);
    expect(LITERAL_COLOUR.test("a { color: orange; }")).toBe(true);
    expect(LITERAL_COLOUR.test("a { background: transparent; }")).toBe(true);
    expect(LITERAL_COLOUR.test("a { color: var(--fs-primary); }")).toBe(false);
    expect(LITERAL_COLOUR.test("a { color: var(--fs-on-primary-container); }")).toBe(false);
  });
  it("reads only the stylesheet, so a route or id in the source is not CSS", () => {
    const source = 'const STYLE = `a { color: var(--fs-primary); }`;\nlink.href = "#abc123";';
    expect(stylesheet(source).match(LITERAL_COLOUR)).toBeNull();
  });
  it("the size check catches a literal size and accepts tokens, 0 and auto", () => {
    expect(untokenisedSizes("a { padding: 4px var(--fs-space-2); }")).toEqual([
      "padding: 4px var(--fs-space-2)",
    ]);
    expect(untokenisedSizes("a { font-size: 1rem; margin: 0 auto; }")).toEqual(["font-size: 1rem"]);
    expect(
      untokenisedSizes("a { margin: 0 0 var(--fs-space-2); gap: var(--fs-space-1); }"),
    ).toEqual([]);
  });
});

/**
 * CSS system colours (CSS Color Module Level 4), used only for forced-colours mode. CSS
 * keywords are case-insensitive, so the match is too.
 */
const SYSTEM_COLOUR =
  /(?<![\w-])(?:Canvas|CanvasText|LinkText|VisitedText|ActiveText|ButtonFace|ButtonText|ButtonBorder|Field|FieldText|Highlight|HighlightText|SelectedItem|SelectedItemText|Mark|MarkText|GrayText|AccentColor|AccentColorText)(?![\w-])/i;

/** Physical (left/right) properties and values that would not mirror under `dir="rtl"`. */
const PHYSICAL =
  /(?<![\w-])(?:(?:margin|padding|border|inset|scroll-margin|scroll-padding)-(?:left|right)(?:-[a-z]+)?|left|right)\s*:|(?:text-align|float|clear)\s*:\s*(?:left|right)\b|border-(?:top|bottom)-(?:left|right)-radius/;

/**
 * Find the first `@media` block whose query contains `query`.
 *
 * @param css Stylesheet text.
 * @param query Text the media query must contain, for example `forced-colors`.
 * @returns The start and end (exclusive) of the block, or undefined when there is none.
 */
function mediaRange(css: string, query: string): [number, number] | undefined {
  const start = css.search(new RegExp(`@media[^{]*${query}[^{]*\\{`));
  if (start < 0) return undefined;
  let depth = 0;
  for (let end = css.indexOf("{", start); end < css.length; end += 1) {
    if (css[end] === "{") depth += 1;
    if (css[end] === "}") depth -= 1;
    if (depth === 0) return [start, end + 1];
  }
  throw new Error(`unbalanced @media ${query} block`);
}

/** Remove every `@media` block whose query contains `query` (at most 20). */
function withoutMedia(css: string, query: string): string {
  let result = css;
  for (let guard = 0; guard < 20; guard += 1) {
    const range = mediaRange(result, query);
    if (!range) return result;
    result = result.slice(0, range[0]) + result.slice(range[1]);
  }
  throw new Error(`more than 20 @media ${query} blocks`);
}

/** The text of the first `@media` block whose query contains `query`, or "". */
function mediaBlock(css: string, query: string): string {
  const range = mediaRange(css, query);
  return range ? css.slice(range[0], range[1]) : "";
}

/** Transition and animation declarations whose durations do not come from the motion token. */
function untokenisedMotion(css: string): string[] {
  const declarations = [
    ...withoutMedia(css, "prefers-reduced-motion").matchAll(
      /((?:transition|animation)(?:-[a-z-]+)?)\s*:\s*([^;{}]+)/g,
    ),
  ];
  return declarations
    .filter(([, property]) => /^(?:transition|animation)(?:-duration)?$/.test(property ?? ""))
    .filter(([, , value]) => !motionFromToken(value ?? ""))
    .map(([declaration]) => (declaration ?? "").trim());
}

/** A literal CSS time such as `150ms`, `.2s` or `0s`. */
const LITERAL_TIME = /(?<![\w-])\d*\.?\d+m?s(?![\w-])/;

/**
 * True when a transition or animation value is `none`, or takes its duration from
 * `--fs-motion-duration` and has no literal time anywhere else (in any of its comma-separated
 * parts).
 */
function motionFromToken(value: string): boolean {
  if (value.trim() === "none") return true;
  if (!value.includes("var(--fs-motion-duration)")) return false;
  return !LITERAL_TIME.test(value.replace(/var\(--fs-motion-duration\)/g, ""));
}

/**
 * Height declarations, physical (`height`) or logical (`block-size`), fixed in pixels, other
 * than the 1px of a visually hidden element.
 */
function fixedPixelHeights(css: string): string[] {
  return [...css.matchAll(/(?<![\w-])((?:min-|max-)?(?:height|block-size))\s*:\s*([^;{}]+)/g)]
    .filter(([, , value]) => /\d+px/.test(value ?? "") && (value ?? "").trim() !== "1px")
    .map(([declaration]) => (declaration ?? "").trim());
}

describe("UI-S-016 accessibility contracts in component stylesheets", () => {
  it("the shared stylesheet draws a 2px --fs-focus-ring focus ring offset by 2px", () => {
    expect(shared).toMatch(/:focus-visible\s*\{[^}]*outline:\s*2px solid var\(--fs-focus-ring\)/);
    expect(shared).toMatch(/:focus-visible\s*\{[^}]*outline-offset:\s*2px/);
  });
  it("the shared stylesheet stops motion under prefers-reduced-motion", () => {
    const block = mediaBlock(shared, "prefers-reduced-motion: reduce");
    expect(block).toMatch(/transition-duration:\s*0s !important/);
    expect(block).toMatch(/animation-duration:\s*0s !important/);
  });
  it("the shared stylesheet strengthens borders under prefers-contrast: more", () => {
    expect(mediaBlock(shared, "prefers-contrast: more")).toMatch(
      /border-color:\s*var\(--fs-on-surface-variant\)/,
    );
  });
  it("the shared stylesheet draws focus in Highlight under forced colours", () => {
    expect(mediaBlock(shared, "forced-colors: active")).toMatch(/outline-color:\s*Highlight/);
  });
  it.each(ELEMENTS)("%s includes the shared accessibility stylesheet", (name) => {
    // The rendering expression, not just the import: the shared rules go first in the style,
    // optionally followed by the shared control stylesheet (UI-S-024).
    expect(sources.find((s) => s.name === name)?.source).toMatch(
      /A11Y_STYLE\s*\+\s*(?:CONTROL_STYLE\s*\+\s*)?STYLE|\$\{A11Y_STYLE\}(?:\$\{CONTROL_STYLE\})?\$\{STYLE\}/,
    );
  });
  it.each(sources)("$name uses logical properties only", ({ css }) => {
    expect(css.match(PHYSICAL)?.[0]).toBeUndefined();
  });
  it.each(sources)("$name takes motion durations from --fs-motion-duration", ({ css }) => {
    expect(untokenisedMotion(css)).toEqual([]);
  });
  it.each(sources)("$name uses system colours only inside forced-colors blocks", ({ css }) => {
    expect(withoutMedia(css, "forced-colors").match(SYSTEM_COLOUR)?.[0]).toBeUndefined();
  });
  it.each(sources)("$name fixes no text container to a pixel height", ({ css }) => {
    expect(fixedPixelHeights(css)).toEqual([]);
  });
  it("the checks catch what they are meant to catch", () => {
    expect(PHYSICAL.test("a { margin-left: auto; }")).toBe(true);
    expect(PHYSICAL.test("th { text-align: left; }")).toBe(true);
    expect(PHYSICAL.test("a { left: 0; }")).toBe(true);
    expect(PHYSICAL.test("a { margin-inline-start: auto; text-align: start; }")).toBe(false);
    expect(untokenisedMotion("a { transition: color 150ms; }")).toEqual([
      "transition: color 150ms",
    ]);
    expect(untokenisedMotion("a { transition: color var(--fs-motion-duration); }")).toEqual([]);
    expect(
      untokenisedMotion(
        "a { transition: color var(--fs-motion-duration), background-color 300ms; }",
      ),
    ).toEqual(["transition: color var(--fs-motion-duration), background-color 300ms"]);
    expect(untokenisedMotion("a { animation-duration: .2s; }")).toEqual([
      "animation-duration: .2s",
    ]);
    expect(SYSTEM_COLOUR.test("a { color: highlight; }")).toBe(true);
    expect(SYSTEM_COLOUR.test("a { background: canvastext; }")).toBe(true);
    expect(SYSTEM_COLOUR.test("fieldset { border: 0; }")).toBe(false);
    expect(fixedPixelHeights("a { block-size: 40px; min-block-size: 2rem; }")).toEqual([
      "block-size: 40px",
    ]);
    expect(fixedPixelHeights("a { max-block-size: 300px; }")).toEqual(["max-block-size: 300px"]);
    expect(SYSTEM_COLOUR.test("a { color: Highlight; }")).toBe(true);
    expect(SYSTEM_COLOUR.test("a { color: var(--fs-highlight); }")).toBe(false);
    expect(
      withoutMedia(
        "@media (forced-colors: active) { a { color: Highlight; } } b {}",
        "forced-colors",
      ),
    ).toBe(" b {}");
    expect(fixedPixelHeights("nav { min-height: calc(100vh - 57px); } i { height: 1px; }")).toEqual(
      ["min-height: calc(100vh - 57px)"],
    );
  });
});

describe("the list scrolls inside the content area (UI-S-025, UI-S-026, Task UI-F3.1)", () => {
  // A list as tall as its rows renders every row and loads every page; the virtual window only
  // works when each element between the shell's main area and the list's scrolling area has a
  // bounded height. Layout itself is proven by the Playwright DOM-bound test in
  // tests/performance/40-resources.perf.spec.ts; this keeps the rule in the stylesheet, where a
  // refactor would break it.
  const css = sources.find((s) => s.name === "fs-navigation-view.ts")?.css ?? "";

  it("gives the navigation view the height of the main area", () => {
    expect(css).toMatch(/:host\s*\{[^}]*block-size:\s*100%/);
    expect(css).toMatch(/:host\s*\{[^}]*min-block-size:\s*0/);
  });

  it("makes the section that holds a file view a bounded column", () => {
    expect(css).toMatch(/section\.drive\s*\{[^}]*display:\s*flex/);
    expect(css).toMatch(/section\.drive\s*\{[^}]*flex-direction:\s*column/);
    expect(css).toMatch(/section\.drive\s*\{[^}]*block-size:\s*100%/);
    expect(css).toMatch(/section\.drive\s*\{[^}]*min-block-size:\s*0/);
  });

  it("keeps the column header at 38 px however long the list is", () => {
    // The scroller shrinks (min-block-size 0) and its content is 44 px per row, so the free
    // space is negative by almost its whole height; a header that may shrink gives up nearly all
    // of its own height (17 px of 38 with 11,000 rows, found by the Large catalog).
    const rows = sources.find((s) => s.name === "drive-row-view.ts")?.css ?? "";
    expect(rows).toMatch(/\.head\s*\{[^}]*flex:\s*none/);
    expect(rows).toMatch(/\.head\s*\{[^}]*block-size:\s*2\.375rem/);
  });

  it("lets the body of that section shrink to the space that is left", () => {
    expect(css).toMatch(/section\.drive > div\s*\{[^}]*flex:\s*1 1 auto/);
    expect(css).toMatch(/section\.drive > div\s*\{[^}]*min-block-size:\s*0/);
  });
});

describe("a long navigation tree does not tax every frame (NFR-002, Task UI-F3.1)", () => {
  // The tree draws every node of a level up to the cap of 10,000 (50 pages of 200), about 80,000
  // elements. Left uncontained, every change in the main area makes the browser walk and
  // commit them again: on the Large catalog it added about 20 ms to each frame and pushed the
  // warm sort result past the 100 ms budget. Layout, paint and style containment (not size,
  // so the stacked narrow layout keeps its height) stops the tree and the rest of the page from
  // costing each other; the Playwright warm-browse budgets are the proof.
  const css = sources.find((s) => s.name === "fs-app-shell.ts")?.css ?? "";

  it("contains the scrolling wrapper of the tree", () => {
    expect(css).toMatch(/\.tree\s*\{[^}]*contain:\s*layout paint style/);
  });

  it("does not use size containment, which would collapse the tree in the stacked layout", () => {
    expect(css).not.toMatch(/\.tree\s*\{[^}]*contain:[^;}]*(size|strict)/);
  });

  it("positions a tree row only while it is the current one", () => {
    // A positioned element is its own paint layer: 10,000 rows would be 10,000 layers to walk on
    // every frame. Only the current row needs a positioned box, for its 2.5 px brand marker.
    const tree = sources.find((s) => s.name === "fs-nav-tree.ts")?.css ?? "";
    // Every rule is checked, comma-separated selector lists included: a rule that names a row
    // (`a`, `a[data-kind=...]`, `a:hover`) other than the current one must not position it.
    const rules = [...tree.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((m) => ({
      selectors: (m[1] ?? "").split(",").map((s) => s.trim()),
      body: m[2] ?? "",
    }));
    const rowRules = rules.filter((r) =>
      r.selectors.some((s) => /^a(?![\w-])/.test(s) && !s.includes('[aria-current="page"]')),
    );
    expect(rowRules.length).toBeGreaterThan(0);
    for (const rule of rowRules)
      expect(rule.body, rule.selectors.join(", ")).not.toMatch(/position:/);
    const current = rules.find((r) => r.selectors.includes('a[aria-current="page"]'));
    expect(current?.body).toMatch(/position:\s*relative/);
  });
});
