// @vitest-environment node
/**
 * SRS-UI-007: custom elements style colour only through tokens, so component
 * sources contain no literal colour values (hex, rgb(), hsl() or named colours in CSS).
 */
import { readFileSync, readdirSync } from "node:fs";
import { describe, expect, it } from "vitest";

const dir = new URL("../../src/components/", import.meta.url);
const sources = readdirSync(dir)
  .filter((name) => name.endsWith(".ts"))
  .map((name) => ({ name, text: readFileSync(new URL(name, dir), "utf8") }));

/** Literal colour forms a component stylesheet must not use. */
const LITERAL_COLOUR =
  /#[0-9a-fA-F]{3,8}\b|\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch)\(|:\s*(?:white|black|red|green|blue|gray|grey)\b/;

describe("component stylesheets", () => {
  it("cover every component source", () => {
    expect(sources.map((s) => s.name)).toEqual(
      expect.arrayContaining(["fs-app-shell.ts", "fs-monitor.ts", "fs-theme-control.ts"]),
    );
  });
  it.each(sources)("$name contains no literal colour", ({ text }) => {
    expect(text.match(LITERAL_COLOUR)?.[0]).toBeUndefined();
  });
  it("the check catches a literal colour", () => {
    expect(LITERAL_COLOUR.test("a { color: #1f5fbf; }")).toBe(true);
    expect(LITERAL_COLOUR.test("a { background: rgb(0 0 0); }")).toBe(true);
    expect(LITERAL_COLOUR.test("a { color: white; }")).toBe(true);
    expect(LITERAL_COLOUR.test('a { color: var(--fs-primary); } href="#/monitor"')).toBe(false);
  });
});
