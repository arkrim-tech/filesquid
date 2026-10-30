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
 * True for the browser's own network line for a refused fetch of a route the UI reads at start
 * that the host may not serve yet. The desktop and server hosts answer `GET /api/v1/navigation`
 * with 503 `unavailable` until they install the navigation service (DE-1.1), and the UI shows
 * that as its unavailable state. `GET /api/v1/preferences` (UI-F2.8, API-S-011) and
 * `GET /api/v1/libraries/recent` (API-S-038) answer 503 until a host installs a preference
 * store or a library registry, and the production-build specs that fake other routes leave them
 * to the preview server's proxy, which answers 500 to 504 when no host runs; the UI keeps
 * working on both (the choices apply until the page closes, file actions stay off). The browser
 * still logs each refused fetch as a console error. Only those lines, from those routes, are
 * excused; the app's own messages and every other route's errors still fail a test.
 *
 * @param text - The console message text.
 * @param url - The URL the message is attributed to.
 * @returns Whether it is that line.
 */
export function isNavigationUnavailable(text: string, url: string): boolean {
  const navigation = /status of 503 \(/.test(text) && /\/api\/v1\/navigation(\?|$)/.test(url);
  const optional =
    /status of 50[0-4] \(/.test(text) &&
    /\/api\/v1\/(preferences|libraries\/recent)(\?|$)/.test(url);
  return navigation || optional;
}

/**
 * Collect console errors and uncaught page errors for the whole test. The browser's network
 * line for a 503 from the navigation listing is not collected (see
 * {@link isNavigationUnavailable}).
 *
 * @param page - The page to observe.
 * @param tolerate - Console lines to ignore, for a deliberately refused call: a pattern of the
 *   text, or a predicate of the text and the URL the line is attributed to (to excuse one
 *   route's refusal only). Page errors are never tolerated.
 * @returns The live list; assert it equals `[]` at the end of the test.
 */
export function trackErrors(
  page: Page,
  tolerate?: RegExp | ((text: string, url: string) => boolean),
): string[] {
  const errors: string[] = [];
  const excused = (text: string, url: string) =>
    typeof tolerate === "function" ? tolerate(text, url) : (tolerate?.test(text) ?? false);
  page.on("console", (msg) => {
    if (msg.type() !== "error" || excused(msg.text(), msg.location().url)) return;
    if (isNavigationUnavailable(msg.text(), msg.location().url)) return;
    errors.push(msg.text());
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
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    throw new Error(`${file} is not a label table object`);
  }
  if (!Object.values(parsed).every((word) => typeof word === "string")) {
    throw new Error(`${file} has a label that is not a string`);
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
  if (!Object.prototype.hasOwnProperty.call(table, key)) {
    throw new Error(`label table has no key "${key}"`);
  }
  return table[key] as string;
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
 * @throws Error when the token is not defined, so a missing token cannot pass by inheriting the
 *   body colour.
 */
export function tokenColour(page: Page, name: string): Promise<string> {
  return page.evaluate((prop) => {
    const probe = document.createElement("span");
    document.body.append(probe);
    const defined = getComputedStyle(probe).getPropertyValue(prop).trim() !== "";
    probe.style.color = `var(${prop})`;
    const colour = getComputedStyle(probe).color;
    probe.remove();
    if (!defined) throw new Error(`token ${prop} is not defined`);
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
