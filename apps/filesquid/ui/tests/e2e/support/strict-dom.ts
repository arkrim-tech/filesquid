/**
 * Strict DOM helpers shared by every Playwright proof (UI-S-013): console errors fail the
 * test, custom elements must be defined, visible words come from the active label table and
 * token colours are read as the browser computes them. Pure page reads, no pixel comparison.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { expect, type Page } from "@playwright/test";

/** A label table: label key to the word the user sees. */
export type LabelTable = Readonly<Record<string, string>>;

/** Colour scheme the browser reports through `prefers-color-scheme`. */
export type Scheme = "light" | "dark";

/**
 * Collect console errors and uncaught page errors for the whole test.
 *
 * @param page - The page to observe.
 * @param tolerate - Console lines to ignore, for a deliberately refused call. Page errors are
 *   never tolerated.
 * @returns The live list; assert it equals `[]` at the end of the test.
 */
export function trackErrors(page: Page, tolerate?: RegExp): string[] {
  const errors: string[] = [];
  page.on("console", (msg) => {
    if (msg.type() === "error" && !tolerate?.test(msg.text())) errors.push(msg.text());
  });
  page.on("pageerror", (err) => errors.push(err.message));
  return errors;
}

/**
 * Load the active label table of the package under test.
 *
 * @param file - Package-relative path of the JSON table (default: `src/theme/labels.en.json`).
 * @returns The parsed table.
 * @throws Error when the file is missing or is not a JSON object of strings.
 */
export function activeLabels(file = "src/theme/labels.en.json"): LabelTable {
  const parsed: unknown = JSON.parse(readFileSync(resolve(process.cwd(), file), "utf8"));
  if (typeof parsed !== "object" || parsed === null) {
    throw new Error(`${file} is not a label table object`);
  }
  return parsed as LabelTable;
}

/**
 * Read one label of a table.
 *
 * @param table - The active label table.
 * @param key - The label key.
 * @returns The word the user sees.
 * @throws Error when the key is absent, so a typo fails loudly instead of matching "undefined".
 */
export function label(table: LabelTable, key: string): string {
  const word = table[key];
  if (word === undefined) throw new Error(`label table has no key "${key}"`);
  return word;
}

/**
 * Assert every named custom element is defined in the page.
 *
 * @param page - The loaded page.
 * @param tags - Custom element tag names.
 */
export async function expectElementsDefined(page: Page, tags: readonly string[]): Promise<void> {
  const defined = await page.evaluate(
    (names) => names.map((name) => [name, customElements.get(name) !== undefined] as const),
    tags,
  );
  expect(defined.filter(([, ok]) => !ok).map(([name]) => name)).toEqual([]);
}

/**
 * Resolve a colour token to the browser's computed `rgb(...)` form through a probe element,
 * so the check does not depend on how the build minified the hex value.
 *
 * @param page - The loaded page.
 * @param name - Custom property name, for example `--fs-primary`.
 * @returns The computed colour.
 */
export function tokenColour(page: Page, name: string): Promise<string> {
  return page.evaluate((prop) => {
    const probe = document.createElement("span");
    probe.style.color = `var(${prop})`;
    document.body.append(probe);
    const colour = getComputedStyle(probe).color;
    probe.remove();
    return colour;
  }, name);
}

/**
 * Computed body background colour.
 *
 * @param page - The loaded page.
 * @returns The computed `rgb(...)` colour.
 */
export function bodyBackground(page: Page): Promise<string> {
  return page.evaluate(() => getComputedStyle(document.body).backgroundColor);
}

/**
 * Convert a `#rrggbb` token value to the browser's computed `rgb(r, g, b)` form.
 *
 * @param hex - Colour as `#rrggbb`.
 * @returns The same colour as `rgb(r, g, b)`.
 * @throws Error when the value is not `#rrggbb`.
 */
export function rgbOf(hex: string): string {
  const match = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
  if (!match) throw new Error(`not a #rrggbb colour: ${hex}`);
  const [r, g, b] = match.slice(1).map((part) => parseInt(part, 16));
  return `rgb(${r}, ${g}, ${b})`;
}

/**
 * Switch the browser's colour scheme preference.
 *
 * @param page - The page.
 * @param scheme - `light` or `dark`.
 */
export async function setScheme(page: Page, scheme: Scheme): Promise<void> {
  await page.emulateMedia({ colorScheme: scheme });
}
