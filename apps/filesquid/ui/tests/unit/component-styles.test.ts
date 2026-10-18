// @vitest-environment node
/**
 * SRS-UI-007: custom elements style colour, type size, spacing and radius only through
 * tokens. The checks read the component stylesheets (the `STYLE` template of each component),
 * not the rest of the source, so routes such as `#/monitor` or ids are never mistaken for CSS.
 */
import { readFileSync, readdirSync } from "node:fs";
import { describe, expect, it } from "vitest";

const dir = new URL("../../src/components/", import.meta.url);
const sources = readdirSync(dir)
  .filter((name) => name.endsWith(".ts"))
  .map((name) => ({ name, css: stylesheet(readFileSync(new URL(name, dir), "utf8")) }));

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

describe("component stylesheets", () => {
  it("cover every component source", () => {
    expect(sources.map((s) => s.name)).toEqual(
      expect.arrayContaining(["fs-app-shell.ts", "fs-monitor.ts", "fs-theme-control.ts"]),
    );
    for (const name of ["fs-app-shell.ts", "fs-monitor.ts", "fs-theme-control.ts"]) {
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
