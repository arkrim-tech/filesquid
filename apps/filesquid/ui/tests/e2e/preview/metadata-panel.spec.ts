/**
 * The Description tab against the production build (`vite build` plus `vite preview`) at the
 * mockups' 1440 × 900 window, in light and dark (Task UI-F2.4; UI-S-003, UI-S-022, UI-S-024,
 * UI-S-031; UI-S-013): the Description tab of mockup 28, the Add property dialog of mockup 29
 * and the panel states of mockup 25. Each test asserts the custom elements are defined, the
 * strings come from the active label table, the regions' boxes match the mockup within 2 px (a
 * mockup y of `n` is `n - 28` here, below its title bar; the dialog is centred in the window
 * as the mockup draws it), the constants are the browser's computed styles, and there is no
 * console error. The routes are faked from the mockups' data.
 */
import { expect, test, type Locator, type Page } from "@playwright/test";
import {
  activeLabels,
  expectElementsDefined,
  label,
  rgbOf,
  setScheme,
  trackErrors,
  type Scheme,
} from "../support/strict-dom";
import { COLOURS, expectBox, look, renderedBorder } from "../support/fidelity";
import { FACADE, PANEL_ROUTE, PANEL_STORED, servePanel } from "../support/file-panel-api";
import {
  REGISTRY,
  serveMetadata,
  type MetadataApi,
  type MetadataOptions,
} from "../support/metadata-api";

const labels = activeLabels();
const SCHEMES: readonly Scheme[] = ["light", "dark"];
const NOW = new Date("2026-10-16T14:00:00Z");
const TRANSPARENT = "rgba(0, 0, 0, 0)";

// A 2x (Retina) display, as the macOS desktop target has.
test.use({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });

/** Fill the placeholders of a label. */
function fill(key: string, values: Record<string, string>): string {
  return label(labels, key).replace(/\{(\w+)\}/g, (whole, name: string) => values[name] ?? whole);
}

/** Serve the fixture, open the file and select the Description tab. */
async function openDescription(page: Page, options: MetadataOptions = {}): Promise<MetadataApi> {
  await page.clock.setFixedTime(NOW);
  await servePanel(page, FACADE);
  const api = await serveMetadata(page, options);
  await page.goto(PANEL_ROUTE);
  await expect(page.locator("fs-file-browser")).toHaveAttribute("data-state", "ready");
  await page.getByRole("button", { name: FACADE.displayName }).click();
  await page.getByRole("tab", { name: label(labels, "filePanelDescription") }).click();
  return api;
}

/** Every text and attribute value in every shadow root of the page, as one string. */
function deepText(page: Page): Promise<string> {
  return page.evaluate(() => {
    const parts: string[] = [];
    const walk = (root: ParentNode) => {
      for (const el of root.querySelectorAll("*")) {
        parts.push(el.textContent ?? "", ...[...el.attributes].map((a) => `${a.name}=${a.value}`));
        if (el.shadowRoot) walk(el.shadowRoot);
      }
    };
    walk(document);
    return parts.join("\n");
  });
}

/** The block-end border of an element: width and colour. */
function endEdge(locator: Locator): Promise<{ width: string; colour: string }> {
  return locator.evaluate((el) => {
    const css = getComputedStyle(el);
    return { width: css.borderBlockEndWidth, colour: css.borderBlockEndColor };
  });
}

/** The computed opacity of an element. */
function opacity(locator: Locator): Promise<string> {
  return locator.evaluate((el) => getComputedStyle(el).opacity);
}

for (const scheme of SCHEMES) {
  const c = COLOURS[scheme];

  test.describe(`${scheme} theme`, () => {
    test.beforeEach(async ({ page }) => setScheme(page, scheme));

    test("Description (28): tab, filter, count, rows, chips, actions and Add property", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await openDescription(page);
      await expectElementsDefined(page, ["fs-app-shell", "fs-file-panel", "fs-metadata-panel"]);
      const panel = page.locator("fs-file-panel");
      const description = page.locator("fs-metadata-panel");
      await expect(description).toHaveAttribute("data-state", "ready");
      const tab = panel.locator('[role="tab"][aria-selected="true"]');
      await expect(tab).toHaveText(label(labels, "filePanelDescription"));
      // The active colour fades in over --fs-motion-duration.
      await expect
        .poll(() => look(tab))
        .toMatchObject({ size: "13px", weight: "600", colour: rgbOf(c.active) });
      expect(await endEdge(tab)).toEqual({ width: "2px", colour: rgbOf(c.primary) });
      const other = panel.locator('[role="tab"]').first();
      expect(await look(other)).toMatchObject({
        size: "13px",
        weight: "500",
        colour: rgbOf(c.variant),
      });
      const filter = description.locator("input.filter");
      await expectBox(filter, { x: 1140, y: 168, width: 280, height: 32 });
      await expect(filter).toHaveAttribute("placeholder", label(labels, "metadataFilter"));
      expect(await look(filter)).toMatchObject({
        size: "13px",
        borderWidth: await renderedBorder(page, "1.5px"),
        borderColour: rgbOf(c.control),
        radius: "6px",
        background: rgbOf(c.card),
      });
      const count = description.locator(".count");
      await expect(count).toHaveText(fill("metadataCount", { count: "8" }));
      expect(await look(count)).toMatchObject({
        size: "11px",
        weight: "400",
        transform: "uppercase",
        spacing: "1.1px",
        colour: rgbOf(c.muted),
      });
      const rows = description.locator(".row");
      await expect(rows).toHaveCount(8);
      await expectBox(rows.first(), { x: 1121, y: 238, width: 319, height: 44 });
      await expectBox(rows.nth(1), { y: 282, height: 44 });
      await expect(rows.locator("dt .key")).toHaveText([
        "dcterms:title",
        "dcterms:creator",
        "dcterms:subject",
        "dcterms:description",
        "dcterms:created",
        "dcterms:rights",
        "schema:contentLocation",
        "schema:copyrightYear",
      ]);
      await expect(rows.locator(".value")).toHaveText([
        "Facade study, north pier",
        "Mara Okafor",
        "harbour · facade · restoration",
        "Morning light on the restored north pier facade.",
        "2026-10-12 14:03 (no time zone)",
        "© 2026 Mara Okafor",
        "North pier, Harbour",
        "2026",
      ]);
      const ownerKey = rows.first().locator("dt .key");
      expect(await look(ownerKey)).toMatchObject({
        size: "13px",
        weight: "500",
        colour: rgbOf(c.primary),
      });
      expect((await look(ownerKey)).family).toMatch(/Mono|monospace/);
      const extracted = rows.nth(4);
      expect(await look(extracted.locator("dt .key"))).toMatchObject({ colour: rgbOf(c.variant) });
      expect(await look(rows.first().locator(".value"))).toMatchObject({
        size: "13px",
        colour: rgbOf(c.body),
      });
      const you = rows.first().locator(".chip.source");
      await expect(you).toHaveText(label(labels, "metadataSourceOwner"));
      await expectBox(you, { y: 243, height: 18 });
      const youBox = await you.boundingBox();
      expect(Math.abs((youBox?.x ?? 0) + (youBox?.width ?? 0) - 1420)).toBeLessThanOrEqual(2);
      expect(await look(you)).toMatchObject({
        size: "11px",
        weight: "500",
        radius: "4px",
        colour: rgbOf(c.active),
        background: rgbOf(c.tint),
      });
      const file = extracted.locator(".chip.source");
      await expect(file).toHaveText(label(labels, "metadataSourceFile"));
      expect(await look(file)).toMatchObject({
        colour: rgbOf(c.variant),
        background: rgbOf(c.raised),
      });
      await expect(extracted.locator("button.edit")).toHaveCount(0);
      await expect(extracted.locator("button.remove")).toHaveCount(0);
      await expect(extracted.locator("button.copy")).toHaveCount(1);
      const subject = rows.nth(2);
      const edit = subject.locator("button.edit");
      expect(await opacity(edit)).toBe("0");
      await subject.hover();
      await expect.poll(() => look(subject)).toMatchObject({ background: rgbOf(c.raised) });
      await expect.poll(() => opacity(edit)).toBe("1");
      await expect(edit).toHaveAttribute(
        "aria-label",
        fill("metadataEdit", { property: "dcterms:subject" }),
      );
      await expect(subject.locator("button.remove")).toHaveAttribute(
        "aria-label",
        fill("metadataRemove", { property: "dcterms:subject" }),
      );
      expect(await look(edit)).toMatchObject({ colour: rgbOf(c.muted) });
      await edit.hover();
      await expect.poll(() => look(edit)).toMatchObject({ colour: rgbOf(c.icon) });
      const add = description.locator(".body > button.add");
      await expect(add).toHaveText(label(labels, "metadataAdd"));
      const last = await rows.last().boundingBox();
      await expectBox(add, { y: (last?.y ?? 0) + (last?.height ?? 0) + 8, height: 36 });
      await expectBox(add.locator("svg"), { x: 1140, width: 16, height: 16 });
      expect(await look(add)).toMatchObject({
        size: "14px",
        weight: "600",
        colour: rgbOf(c.primary),
        background: TRANSPARENT,
        radius: "7px",
      });
      await expect(description.locator(".footer")).toHaveCount(0);
      const everything = await deepText(page);
      for (const stored of PANEL_STORED) expect(everything).not.toContain(stored);
      expect(everything).not.toContain("exif:");
      expect(errors).toEqual([]);
    });

    test("Add property (29): the dialog, the groups in the add order, one set and the new row", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      const api = await openDescription(page);
      const description = page.locator("fs-metadata-panel");
      await description.locator(".body > button.add").click();
      const dialog = description.locator('[role="dialog"]');
      await expect(dialog).toBeVisible();
      await expectBox(dialog, { x: 420, width: 600 });
      expect(await look(dialog)).toMatchObject({
        background: rgbOf(c.overlay),
        borderWidth: "1px",
        borderColour: rgbOf(c.strong),
        radius: "10px",
      });
      const scrim = await description.locator(".scrim").evaluate((el) => {
        const css = getComputedStyle(el, "::before");
        return { background: css.backgroundColor, opacity: css.opacity };
      });
      expect(scrim).toEqual({ background: "rgb(0, 0, 0)", opacity: "0.32" });
      const title = dialog.locator(".dialog-title");
      await expect(title).toHaveText(label(labels, "metadataAdd"));
      expect(await look(title)).toMatchObject({
        size: "22px",
        weight: "700",
        colour: rgbOf(c.heading),
      });
      expect((await look(title)).family).toMatch(/^Georgia/);
      await expect(dialog.locator(".file-name")).toHaveText("facade-study.jpg");
      expect(await look(dialog.locator(".file-name"))).toMatchObject({
        size: "13px",
        colour: rgbOf(c.variant),
      });
      const search = dialog.locator("input.search");
      await expect(search).toBeFocused();
      await expectBox(search, { height: 36 });
      await expect
        .poll(() => look(search))
        .toMatchObject({
          size: "14px",
          radius: "6px",
          borderWidth: "2px",
          borderColour: rgbOf(c.focus),
        });
      await expect(dialog.locator("button.confirm")).toBeDisabled();
      await search.fill("righ");
      await expect(dialog.locator(".group")).toHaveCount(3);
      expect(api.lookups).toEqual(["righ"]);
      const eyebrows = dialog.locator(".group .eyebrow");
      await expect(eyebrows).toHaveText([
        label(labels, "metadataGroupDcterms"),
        label(labels, "metadataGroupSchema"),
        label(labels, "metadataGroupRegistered"),
      ]);
      expect(await look(eyebrows.first())).toMatchObject({
        size: "11px",
        weight: "400",
        transform: "uppercase",
        colour: rgbOf(c.muted),
      });
      const dcterms = dialog.locator('.group[data-group="dcterms"] .candidate');
      await expect(dcterms.locator(".curie")).toHaveText([
        "dcterms:rights",
        "dcterms:rightsHolder",
        "dcterms:accessRights",
        "dcterms:dateCopyrighted",
      ]);
      await expect(dcterms.first().locator(".description")).toHaveText(
        label(labels, "dctermsRights"),
      );
      expect(await look(dcterms.first().locator(".curie"))).toMatchObject({
        size: "13px",
        weight: "600",
        colour: rgbOf(c.primary),
      });
      expect((await look(dcterms.first().locator(".curie"))).family).toMatch(/Mono|monospace/);
      expect(await look(dcterms.first().locator(".description"))).toMatchObject({
        size: "13px",
        colour: rgbOf(c.variant),
      });
      const schema = dialog.locator('.group[data-group="schema"] .candidate');
      await expect(schema.locator(".curie")).toHaveText(["schema:righ"]);
      await expect(schema.locator(".description")).toHaveText([
        label(labels, "metadataSchemaNote"),
      ]);
      expect(await look(schema.locator(".curie"))).toMatchObject({ colour: rgbOf(c.variant) });
      const registered = dialog.locator('.group[data-group="registered"] .candidate');
      await expect(registered.locator(".curie")).toHaveText(["rico:righ", "harbour:righ"]);
      await expect(registered.first().locator(".description")).toHaveText(REGISTRY[0].iri);
      const custom = dialog.locator("button.custom-key");
      await expect(custom).toHaveText(label(labels, "metadataCustomKey"));
      expect(await look(custom)).toMatchObject({
        size: "14px",
        weight: "600",
        colour: rgbOf(c.primary),
      });
      await dcterms.first().click();
      await expect(dcterms.first()).toHaveAttribute("aria-pressed", "true");
      await expect.poll(() => look(dcterms.first())).toMatchObject({ background: rgbOf(c.tint) });
      await expect
        .poll(() => look(dcterms.first().locator(".curie")))
        .toMatchObject({ colour: rgbOf(c.active) });
      const valueLabel = dialog.locator(".value-label");
      await expect(valueLabel).toHaveText(fill("metadataValueFor", { property: "dcterms:rights" }));
      expect(await look(valueLabel)).toMatchObject({
        size: "13px",
        weight: "600",
        colour: rgbOf(c.body),
      });
      const value = dialog.locator("input.value-input");
      await expect(value).toBeFocused();
      await expectBox(value, { height: 36 });
      const cancel = dialog.locator("button.cancel");
      const confirm = dialog.locator("button.confirm");
      await expectBox(cancel, { height: 38 });
      await expectBox(confirm, { height: 38 });
      expect(await look(cancel)).toMatchObject({
        size: "14px",
        weight: "600",
        background: rgbOf(c.card),
        borderColour: rgbOf(c.strong),
        radius: "7px",
      });
      expect(await look(confirm)).toMatchObject({
        size: "14px",
        weight: "600",
        background: rgbOf(c.primary),
        colour: rgbOf(c.onPrimary),
        radius: "7px",
      });
      await expect(cancel).toHaveText(label(labels, "fileCancel"));
      await expect(confirm).toHaveText(label(labels, "metadataAddConfirm"));
      await expect(dialog.locator(".button.primary")).toHaveCount(1);
      await value.fill("© 2026 Mara Okafor. All rights reserved.");
      await confirm.click();
      await expect(dialog).toHaveCount(0);
      expect(api.sets).toEqual([
        {
          library_id: "lib",
          file_id: "7f3c9a1e-0b2c-4d5e-8f90-a1b2c341d2b8",
          curie: "dcterms:rights",
          value: "© 2026 Mara Okafor. All rights reserved.",
        },
      ]);
      await expect(description.locator(".announce").first()).toHaveText(
        fill("metadataSet", { property: "dcterms:rights", version: "v5" }),
      );
      await expect(description.locator('.row[data-key="dcterms:rights"] .value')).toHaveText(
        "© 2026 Mara Okafor. All rights reserved.",
      );
      await expect(description.locator(".body > button.add")).toBeFocused();
      expect(errors).toEqual([]);
    });

    test("edit and remove (28): one PUT, one POST, the rows and the announcements", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      const api = await openDescription(page);
      const description = page.locator("fs-metadata-panel");
      const subject = description.locator('.row[data-key="dcterms:subject"]');
      await subject.hover();
      await subject.locator("button.edit").click();
      const dialog = description.locator('[role="dialog"]');
      await expect(dialog.locator(".dialog-title")).toHaveText(
        fill("metadataEditTitle", { property: "dcterms:subject" }),
      );
      await expect(dialog.locator("input.search")).toHaveCount(0);
      const value = dialog.locator("input.value-input");
      await expect(value).toBeFocused();
      await expect(value).toHaveValue("harbour · facade · restoration");
      await expect(dialog.locator("button.confirm")).toHaveText(label(labels, "fileSave"));
      await value.fill("harbour · facade · restoration · 2026");
      await dialog.locator("button.confirm").click();
      await expect(dialog).toHaveCount(0);
      expect(api.sets).toEqual([
        {
          library_id: "lib",
          file_id: "7f3c9a1e-0b2c-4d5e-8f90-a1b2c341d2b8",
          curie: "dcterms:subject",
          value: "harbour · facade · restoration · 2026",
        },
      ]);
      await expect(description.locator(".announce").first()).toHaveText(
        fill("metadataSet", { property: "dcterms:subject", version: "v5" }),
      );
      await expect(subject.locator(".value")).toHaveText("harbour · facade · restoration · 2026");
      await expect(subject.locator("button.edit")).toBeFocused();

      const year = description.locator('.row[data-key="schema:copyrightYear"]');
      await year.hover();
      await year.locator("button.remove").click();
      await expect(description.locator(".announce").first()).toHaveText(
        fill("metadataRemoved", { property: "schema:copyrightYear", version: "v6" }),
      );
      expect(api.removals).toEqual([
        {
          library_id: "lib",
          file_id: "7f3c9a1e-0b2c-4d5e-8f90-a1b2c341d2b8",
          curie: "schema:copyrightYear",
        },
      ]);
      expect(api.methods).toEqual(["PUT", "POST"]);
      await expect(description.locator(".row")).toHaveCount(7);
      await expect(year).toHaveCount(0);
      await expect(description.locator(".count")).toHaveText(fill("metadataCount", { count: "7" }));
      await expect(description.locator(".body > button.add")).toBeFocused();
      expect(errors).toEqual([]);
    });

    test("panel states (25): empty, loading, no match and error keep the header, tabs and filter", async ({
      page,
    }) => {
      // Only the deliberately refused statements read is excused.
      const errors = trackErrors(
        page,
        (text, url) => /status of 503/.test(text) && /\/api\/v1\/files\/metadata\?/.test(url),
      );
      const api = await openDescription(page, { empty: true });
      const description = page.locator("fs-metadata-panel");
      await expect(description).toHaveAttribute("data-state", "empty");
      await expect(description.locator(".count")).toHaveText(label(labels, "metadataCountEmpty"));
      const empty = description.locator(".state.empty");
      await expect(empty.locator(".title")).toHaveText(label(labels, "metadataEmpty"));
      await expect(empty.locator(".detail")).toHaveText(label(labels, "metadataEmptyHint"));
      expect(await look(empty.locator(".title"))).toMatchObject({
        size: "14px",
        weight: "600",
        colour: rgbOf(c.body),
      });
      expect(await look(empty.locator(".detail"))).toMatchObject({
        size: "13px",
        colour: rgbOf(c.variant),
      });
      const add = empty.locator("button.add");
      await expect(add).toHaveText(label(labels, "metadataAdd"));
      await expectBox(add, { height: 36 });
      expect(await look(add)).toMatchObject({ colour: rgbOf(c.primary), background: TRANSPARENT });
      await expect(description.locator("input.filter")).toBeDisabled();
      await expect(page.locator("fs-file-panel h2.name")).toHaveText("facade-study.jpg");

      const release = api.holdStatements();
      await page.locator("fs-file-panel button.close").click();
      await page.getByRole("button", { name: label(labels, "fileNext") }).click();
      await expect(description).toHaveAttribute("data-state", "loading");
      await expect(description.locator(".count")).toHaveText(label(labels, "metadataLoading"));
      expect(await look(description.locator(".count"))).toMatchObject({
        size: "11px",
        weight: "500",
        transform: "none",
        colour: rgbOf(c.muted),
      });
      await expect(description.locator(".placeholder")).toHaveCount(10);
      expect(await look(description.locator(".bar").first())).toMatchObject({
        background: rgbOf(c.outline),
      });
      release();
      await expect(description).toHaveAttribute("data-state", "empty");

      api.failStatements(503, "unavailable");
      await page.locator("fs-file-panel button.close").click();
      await page.getByRole("button", { name: label(labels, "filePrevious") }).click();
      await expect(description).toHaveAttribute("data-state", "error");
      const alert = description.locator('.state[role="alert"]');
      await expect(alert.locator(".title")).toHaveText(label(labels, "metadataError"));
      await expect(alert.locator(".detail")).toHaveText([
        label(labels, "stateUnavailable"),
        label(labels, "filePanelNothingChanged"),
      ]);
      await expect(alert.locator(".code")).toHaveText("unavailable");
      expect((await look(alert.locator(".code"))).family).toMatch(/Mono|monospace/);
      expect(await alert.locator("svg.alerting").evaluate((el) => getComputedStyle(el).color)).toBe(
        rgbOf(c.error),
      );
      await expect(alert.locator("button.retry")).toHaveText(label(labels, "stateRetry"));
      expect(errors).toEqual([]);
    });

    test("no match and keyboard: the filter, Escape in the dialog, and the focus ring", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await openDescription(page);
      const description = page.locator("fs-metadata-panel");
      await description.locator("input.filter").fill("zzz");
      await expect(description).toHaveAttribute("data-state", "no-match");
      const state = description.locator(".state");
      await expect(state.locator(".title")).toHaveText(fill("metadataNoMatch", { word: "zzz" }));
      await expect(state.locator(".detail")).toHaveText(label(labels, "filePanelNoMatchHint"));
      await state.locator("button.clear").click();
      await expect(description.locator(".row")).toHaveCount(8);
      const add = description.locator(".body > button.add");
      // Reach the button by keyboard, so the focus ring (:focus-visible) applies.
      await description.locator(".row").last().locator("button.copy").focus();
      await page.keyboard.press("Tab");
      await expect(add).toBeFocused();
      const outline = await add.evaluate((el) => {
        const css = getComputedStyle(el);
        return { style: css.outlineStyle, width: css.outlineWidth, colour: css.outlineColor };
      });
      expect(outline).toEqual({ style: "solid", width: "2px", colour: rgbOf(c.focus) });
      await page.keyboard.press("Enter");
      const dialog = description.locator('[role="dialog"]');
      await expect(dialog).toBeVisible();
      await page.keyboard.press("Escape");
      await expect(dialog).toHaveCount(0);
      await expect(add).toBeFocused();
      expect(errors).toEqual([]);
    });
  });
}
