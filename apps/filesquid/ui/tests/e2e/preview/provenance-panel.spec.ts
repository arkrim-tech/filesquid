/**
 * The file panel's File info, Description and Provenance tabs against the production build
 * (`vite build` plus `vite preview`) at the mockups' 1440 × 900 window, in light and dark (Task
 * UI-F2.15; UI-S-020, UI-S-022, UI-S-024, UI-S-031; UI-S-013): the File info tab of mockup 35
 * with the facts and the technical group, the Provenance tab of mockup 36, the panel states of
 * mockup 25, the media frame of mockup 22 beside the panel, and the behaviour of the tabs: rows,
 * copy, in-tab search, loading, empty and error states, keyboard, a reconstructed activity, View
 * in Logs, the JSON-LD export and the absence of a per-file audit log tab. Each test asserts the
 * custom elements are defined, the strings come from the active label table, the regions' boxes
 * match the mockup within 2 px (a mockup y of `n` is `n - 28` here, below its title bar; regions
 * held at the window's foot keep the mockup's y), the constants are the browser's computed
 * styles, and there is no console error. The routes are faked from the mockups' data, and the
 * clock is fixed at 14:00 UTC on 16 October 2026.
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
import {
  FACADE,
  PANEL_OBJECT,
  PANEL_ROUTE,
  servePanel,
  type PanelApi,
} from "../support/file-panel-api";
import { PROV_INSTALLATION } from "../../support/provenance-fakes";

const labels = activeLabels();
const SCHEMES: readonly Scheme[] = ["light", "dark"];
const NOW = new Date("2026-10-16T14:00:00Z");
const FILE_ID = PANEL_OBJECT.slice("urn:uuid:".length);

// A 2x (Retina) display, as the macOS desktop target has.
test.use({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });

/** Fill the placeholders of a label. */
function fill(key: string, values: Record<string, string>): string {
  return label(labels, key).replace(/\{(\w+)\}/g, (whole, name: string) => values[name] ?? whole);
}

/** The shell's health, host (the desktop, so View in Logs is offered) and event channel. */
async function serveShell(page: Page): Promise<void> {
  await page.route("**/api/v1/health", (route) =>
    route.fulfill({ json: { schema_version: 1, status: "ok", product: "filesquid" } }),
  );
  await page.route("**/api/v1/host", (route) =>
    route.fulfill({
      json: {
        schema_version: 1,
        host_type: "desktop",
        operator_name: null,
        hosted_service: {
          enrolled: false,
          backup_enabled: false,
          sync_enabled: false,
          service_host: null,
        },
      },
    }),
  );
  await page.routeWebSocket(/\/api\/v1\/events$/, () => undefined);
}

/** Serve the panel and open `facade-study.jpg`, so the file panel shows File info. */
async function openPanel(page: Page, setup?: (api: PanelApi) => void): Promise<PanelApi> {
  await page.clock.setFixedTime(NOW);
  await serveShell(page);
  const api = await servePanel(page, FACADE);
  setup?.(api);
  await page.goto(PANEL_ROUTE);
  await expect(page.locator("fs-file-browser")).toHaveAttribute("data-state", "ready");
  await page.getByRole("button", { name: FACADE.displayName }).click();
  await expect(page.locator("fs-file-panel")).toBeVisible();
  await expect(page.locator("fs-details-panel")).toHaveAttribute("data-state", "ready");
  return api;
}

/** Select a tab of the file panel by its label. */
async function selectTab(page: Page, key: string): Promise<void> {
  await page.getByRole("tab", { name: label(labels, key) }).click();
}

/** Open the Provenance tab and wait for its activities. */
async function openProvenance(page: Page, setup?: (api: PanelApi) => void): Promise<PanelApi> {
  const api = await openPanel(page, setup);
  await selectTab(page, "filePanelProvenance");
  await expect(page.locator("fs-provenance-panel")).toHaveAttribute("data-state", "ready");
  return api;
}

/** The block-start border of an element: width and colour. */
function topEdge(locator: Locator): Promise<{ width: string; colour: string }> {
  return locator.evaluate((el) => {
    const css = getComputedStyle(el);
    return { width: css.borderBlockStartWidth, colour: css.borderBlockStartColor };
  });
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

for (const scheme of SCHEMES) {
  const c = COLOURS[scheme];

  test.describe(`${scheme} theme`, () => {
    test.beforeEach(async ({ page }) => setScheme(page, scheme));

    test("File info (35): the import facts rows, the technical group and the footer", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await openPanel(page, (api) => {
        api.provenance.technical = [{ curie: "exif:Duration", value: "6:18" }];
      });
      await expectElementsDefined(page, ["fs-app-shell", "fs-file-panel", "fs-details-panel"]);
      const details = page.locator("fs-details-panel");
      await expectBox(details.locator("input.filter"), { x: 1140, y: 168, width: 280, height: 32 });
      await expect(details.locator(".count")).toHaveText(
        fill("filePanelInfoCount", { count: "11" }),
      );
      const uploaded = details.locator('.row[data-key="uploaded"]');
      const source = details.locator('.row[data-key="source"]');
      await expect(uploaded.locator("dt")).toHaveText(label(labels, "filePanelUploaded"));
      await expect(uploaded.locator(".value")).toHaveText(
        fill("filePanelUploadedValue", { local: "Oct 12, 2026, 14:20", utc: "14:20" }),
      );
      await expect(source.locator(".value")).toHaveText(
        fill("filePanelSourceValue", {
          method: label(labels, "filePanelMethodFolder"),
          volume: "FUJIFILM",
        }),
      );
      await expect(details.locator('.row[data-key="format"] .value')).toHaveText(
        fill("filePanelFormatValue", { format: "JPEG image", mime: "image/jpeg" }),
      );
      expect(await look(uploaded.locator("dt"))).toMatchObject({
        size: "13px",
        colour: rgbOf(c.muted),
      });
      expect(await look(uploaded.locator(".value"))).toMatchObject({
        size: "13px",
        colour: rgbOf(c.body),
      });
      const group = details.locator(".group-title");
      await expect(group).toHaveText(label(labels, "filePanelTechnicalGroup"));
      expect(await look(group)).toMatchObject({
        size: "11px",
        weight: "400",
        transform: "uppercase",
        spacing: "1.1px",
        colour: rgbOf(c.muted),
      });
      await expect(details.locator('.row[data-key="technical-Duration"] .value')).toHaveText(
        "6:18",
      );
      await expect(details.locator('.row[data-key="technical-Duration"] dt')).toHaveText(
        "Duration",
      );
      await expect(details.locator(".footer")).toHaveText(label(labels, "filePanelInfoFooter"));
      await expectBox(details.locator(".footer"), { x: 1121, y: 856, width: 319, height: 44 });
      const everything = await deepText(page);
      for (const secret of ["/Users/", "import_root", "host_name"]) {
        expect(everything).not.toContain(secret);
      }
      expect(errors).toEqual([]);
    });

    test("File info (24): a format with nothing to read says so", async ({ page }) => {
      const errors = trackErrors(page);
      await openPanel(page);
      const details = page.locator("fs-details-panel");
      await expect(details.locator(".group-title")).toHaveText(
        label(labels, "filePanelTechnicalGroup"),
      );
      const note = details.locator(".note-row");
      await expect(note).toHaveText(label(labels, "filePanelTechnicalNone"));
      expect(await look(note)).toMatchObject({ size: "13px", colour: rgbOf(c.variant) });
      expect(errors).toEqual([]);
    });

    test("Provenance (36): the newest activity expanded, the timeline and the footer", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await openProvenance(page);
      await expectElementsDefined(page, ["fs-app-shell", "fs-file-panel", "fs-provenance-panel"]);
      const prov = page.locator("fs-provenance-panel");
      const panel = page.locator("fs-file-panel");
      const tab = panel.locator('[role="tab"][aria-selected="true"]');
      await expect(tab).toHaveText(label(labels, "filePanelProvenance"));
      // The selected tab's colour fades in over --fs-motion-duration.
      await expect.poll(async () => (await look(tab)).colour).toBe(rgbOf(c.active));
      expect(await look(tab)).toMatchObject({ size: "13px", weight: "600" });
      await expectBox(prov.locator("input.filter"), { x: 1140, y: 168, width: 280, height: 32 });
      await expect(prov.locator("input.filter")).toHaveAttribute(
        "placeholder",
        label(labels, "provenanceFilter"),
      );
      await expect(prov.locator(".count")).toHaveText(label(labels, "provenanceCount"));
      expect(await look(prov.locator(".count"))).toMatchObject({
        size: "11px",
        weight: "400",
        transform: "uppercase",
        spacing: "1.1px",
        colour: rgbOf(c.muted),
      });
      const rows = prov.locator("li.activity");
      await expect(rows).toHaveCount(4);
      await expect(rows.locator(".kind")).toHaveText([
        "metadata edit",
        "restore",
        "remove",
        "import",
      ]);
      await expect(rows.locator(".chip.version")).toHaveText(["v4", "v3", "v2", "v1"]);
      const first = rows.first();
      await expectBox(first.locator(".kind"), { x: 1178 });
      await expectBox(first.locator(".chip.version"), { x: 1391.5, width: 28.5, height: 18 });
      expect(await look(first.locator(".kind"))).toMatchObject({
        size: "13px",
        weight: "600",
        colour: rgbOf(c.body),
      });
      expect(await look(first.locator(".type"))).toMatchObject({
        size: "11px",
        colour: rgbOf(c.muted),
      });
      expect((await look(first.locator(".type"))).family).toMatch(/Mono|monospace/);
      expect(await look(first.locator(".chip.version"))).toMatchObject({
        size: "11px",
        weight: "500",
        colour: rgbOf(c.variant),
        background: rgbOf(c.raised),
        radius: "4px",
      });
      expect((await look(first.locator(".chip.version"))).family).toMatch(/Mono|monospace/);
      expect(await look(first.locator(".when"))).toMatchObject({
        size: "13px",
        colour: rgbOf(c.variant),
      });
      await expect(first.locator(".when")).toHaveText("Oct 16, 2026, 11:05 UTC");
      const names = first.locator(".attr .name");
      await expect(names).toHaveText([
        "prov:startedAtTime",
        "prov:endedAtTime",
        "prov:wasAssociatedWith",
        "prov:used",
        fill("provenanceGeneratedBy", { version: "v4" }),
        "prov:wasDerivedFrom",
      ]);
      expect(await look(names.first())).toMatchObject({
        size: "13px",
        weight: "500",
        colour: rgbOf(c.muted),
      });
      expect((await look(names.first())).family).toMatch(/Mono|monospace/);
      const values = first.locator(".attr .val .text");
      await expect(values.first()).toHaveText("2026-10-16T11:05:02Z");
      expect((await look(values.first())).family).toMatch(/Mono|monospace/);
      expect(await look(values.first())).toMatchObject({ size: "13px", colour: rgbOf(c.body) });
      const association = first.locator('.attr[data-name="prov:wasAssociatedWith"] .val .text');
      await expect(association).toHaveText([
        "urn:uuid:5a0c41e2-…-55e19b",
        label(labels, "provenanceAgentInstallation"),
        fill("provenanceAgentSoftware", { product: "FileSquid", version: "0.1.0" }),
        "prov:SoftwareAgent",
      ]);
      const used = first.locator('.attr[data-name="prov:used"] .val .text');
      await expect(used).toHaveText([
        "urn:uuid:7f3c9a1e-…-41d2b8@v3",
        "Harbour project/facade-study.jpg",
        "sha256 9b1f4e…c07a2d",
      ]);
      await expect(first.locator('.attr[data-name="prov:wasDerivedFrom"] .val .text')).toHaveText(
        "…41d2b8@v3",
      );
      // The timeline: the latest dot is filled with the primary, the others are outlined.
      expect(await look(first.locator(".dot"))).toMatchObject({
        background: rgbOf(c.primary),
        radius: "999px",
      });
      expect(await look(rows.nth(1).locator(".dot"))).toMatchObject({
        background: rgbOf(c.card),
        borderWidth: await renderedBorder(page, "1.5px"),
        borderColour: rgbOf(c.strong),
      });
      await expect(rows.nth(1).locator("button.toggle")).toHaveAttribute("aria-expanded", "false");
      await expect(rows.nth(1).locator(".attr")).toHaveCount(0);
      // The footer: View in Logs (ghost) and JSON-LD (secondary) over a 1 px outline.
      const footer = prov.locator(".footer");
      await expectBox(footer, { x: 1121, y: 856, width: 319, height: 44 });
      expect(await topEdge(footer)).toEqual({ width: "1px", colour: rgbOf(c.outline) });
      const logs = footer.locator("a.logs");
      await expect(logs).toHaveText(label(labels, "provenanceViewInLogs"));
      expect(await look(logs)).toMatchObject({
        size: "14px",
        weight: "600",
        colour: rgbOf(c.primary),
        background: "rgba(0, 0, 0, 0)",
      });
      const exportButton = footer.locator("button.export");
      await expect(exportButton).toHaveText(label(labels, "provenanceExport"));
      expect(await look(exportButton)).toMatchObject({
        size: "14px",
        weight: "600",
        colour: rgbOf(c.body),
        background: rgbOf(c.card),
        borderColour: rgbOf(c.strong),
        borderWidth: await renderedBorder(page, "1.5px"),
        radius: "7px",
      });
      expect(errors).toEqual([]);
    });

    test("Panel states (25): loading, no match and error keep the header, tabs and filter", async ({
      page,
    }) => {
      const errors = trackErrors(page, /status of 503 \(/);
      let release: (() => void) | undefined;
      await openPanel(page, (a) => {
        release = a.provenance.holdHistory();
      });
      await selectTab(page, "filePanelProvenance");
      const prov = page.locator("fs-provenance-panel");
      await expect(prov).toHaveAttribute("data-state", "loading");
      await expect(prov.locator(".count")).toHaveText(label(labels, "provenanceLoading"));
      await expect(prov.locator(".placeholder")).toHaveCount(10);
      await expect(prov.locator("input.filter")).toBeDisabled();
      await expect(prov.locator(".footer .note")).toHaveText(label(labels, "provenanceFooter"));
      expect(await look(prov.locator(".placeholder .bar").first())).toMatchObject({
        background: rgbOf(c.outline),
      });
      await expect(page.locator("fs-file-panel h2.name")).toHaveText("facade-study.jpg");
      release?.();
      await expect(prov).toHaveAttribute("data-state", "ready");
      // No match: the word, the hint, Clear filter, and "0 of 4 activities".
      await prov.locator("input.filter").fill("copyright");
      await expect(prov).toHaveAttribute("data-state", "no-match");
      await expect(prov.locator(".state .title")).toHaveText(
        fill("provenanceNoMatch", { word: "copyright" }),
      );
      await expect(prov.locator(".state .detail")).toHaveText(
        label(labels, "filePanelNoMatchHint"),
      );
      await expect(prov.locator(".count")).toHaveText(
        fill("provenanceCountFiltered", { shown: "0", count: "4" }),
      );
      expect(await look(prov.locator(".state .title"))).toMatchObject({
        size: "14px",
        weight: "600",
        colour: rgbOf(c.body),
      });
      expect(await look(prov.locator(".state .detail"))).toMatchObject({
        size: "13px",
        colour: rgbOf(c.variant),
      });
      const clear = prov.locator(".state button.clear");
      await expect(clear).toHaveText(label(labels, "filePanelClearFilter"));
      expect(await look(clear)).toMatchObject({
        size: "14px",
        weight: "600",
        radius: "7px",
        borderColour: rgbOf(c.strong),
      });
      await expect(prov.locator(".footer .note")).toHaveText(label(labels, "provenanceFooter"));
      await clear.click();
      await expect(prov).toHaveAttribute("data-state", "ready");
      await expect(prov.locator("li.activity")).toHaveCount(4);
      expect(errors).toEqual([]);
    });

    test("Panel states (25): an error is its own state, with the code, Try again and View in Logs", async ({
      page,
    }) => {
      const errors = trackErrors(page, /status of 503 \(/);
      await openPanel(page, (api) => api.provenance.failHistory(503, "unavailable"));
      await selectTab(page, "filePanelProvenance");
      const prov = page.locator("fs-provenance-panel");
      await expect(prov).toHaveAttribute("data-state", "error");
      const state = prov.locator(".state");
      await expect(state).toHaveAttribute("role", "alert");
      await expect(state.locator(".title")).toHaveText(label(labels, "provenanceError"));
      await expect(state.locator(".detail").first()).toHaveText(label(labels, "stateUnavailable"));
      await expect(state.locator(".detail").nth(1)).toHaveText(
        label(labels, "filePanelNothingChanged"),
      );
      await expect(state.locator(".code")).toHaveText("unavailable");
      expect((await look(state.locator(".code"))).family).toMatch(/Mono|monospace/);
      await expect(state.locator("button.retry")).toHaveText(label(labels, "stateRetry"));
      const logs = state.locator("a.logs");
      await expect(logs).toHaveText(label(labels, "provenanceViewInLogs"));
      expect(await look(logs)).toMatchObject({ colour: rgbOf(c.primary) });
      await expect(prov.locator("input.filter")).toBeDisabled();
      await expect(prov.locator(".footer .note")).toHaveText(label(labels, "provenanceFooter"));
      expect(errors).toEqual([]);
    });

    test("Preview (22): the media frame has the 8 px radius token on the raised surface", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await openPanel(page);
      const media = page.locator("fs-file-preview img");
      await expect(media).toBeVisible();
      expect(await look(media)).toMatchObject({ radius: "8px", background: rgbOf(c.raised) });
      await expectBox(page.locator("fs-app-shell aside.panel"), {
        x: 1120,
        y: 61,
        width: 320,
        height: 840,
      });
      expect(errors).toEqual([]);
    });
  });
}

test("the tabs are exactly File info, Description and Provenance, with no audit log tab", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await openPanel(page);
  const tabs = page.locator('fs-file-panel [role="tab"]');
  await expect(tabs).toHaveText([
    label(labels, "filePanelInfo"),
    label(labels, "filePanelDescription"),
    label(labels, "filePanelProvenance"),
  ]);
  const text = (await deepText(page)).toLowerCase();
  expect(text).not.toContain("audit log");
  await expect(
    page.locator("fs-file-panel").getByRole("tab", { name: /audit|journal/i }),
  ).toHaveCount(0);
  expect(errors).toEqual([]);
});

test("the Provenance tab reads only its own routes and mounts without the other panels", async ({
  page,
}) => {
  const errors = trackErrors(page);
  const api = await openPanel(page);
  expect(api.provenance.historyRequests).toHaveLength(0);
  await expect(page.locator("fs-provenance-panel")).toHaveCount(0);
  await selectTab(page, "filePanelProvenance");
  await expect(page.locator("fs-provenance-panel")).toHaveAttribute("data-state", "ready");
  expect(api.provenance.historyRequests).toEqual([null]);
  await expect.poll(() => api.provenance.recordRequests).toEqual(["v4"]);
  // The Description tab was never shown, so its panel was never created or asked for anything.
  await expect(page.locator("fs-metadata-panel")).toHaveCount(0);
  expect(errors).toEqual([]);
});

test("keyboard: arrows move across the three tabs and the activities toggle with Enter", async ({
  page,
}) => {
  const errors = trackErrors(page);
  const api = await openPanel(page);
  const info = page.getByRole("tab", { name: label(labels, "filePanelInfo") });
  await info.focus();
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("ArrowRight");
  const provenance = page.getByRole("tab", { name: label(labels, "filePanelProvenance") });
  await expect(provenance).toBeFocused();
  await expect(provenance).toHaveAttribute("aria-selected", "true");
  const prov = page.locator("fs-provenance-panel");
  await expect(prov).toHaveAttribute("data-state", "ready");
  const toggles = prov.locator("button.toggle");
  await toggles.nth(1).focus();
  await page.keyboard.press("Enter");
  await expect(toggles.nth(1)).toHaveAttribute("aria-expanded", "true");
  await expect(toggles.nth(1)).toBeFocused();
  expect(api.provenance.recordRequests).toEqual(["v4", "v3"]);
  await page.keyboard.press("Space");
  await expect(toggles.nth(1)).toHaveAttribute("aria-expanded", "false");
  await expect(toggles.nth(1)).toBeFocused();
  // The ring on a toggle is the 2 px solid --fs-focus-ring outline offset by 2 px.
  const ring = await toggles.nth(1).evaluate((el) => {
    const css = getComputedStyle(el);
    return { width: css.outlineWidth, style: css.outlineStyle, offset: css.outlineOffset };
  });
  expect(ring).toEqual({ width: "2px", style: "solid", offset: "2px" });
  expect(errors).toEqual([]);
});

test("a reconstructed activity is marked and its agent is shown as unrecorded", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await openProvenance(page, (api) => {
    const entries = api.provenance.entries;
    entries[0] = {
      ...(entries[0] as (typeof entries)[number]),
      reconstructed: true,
      started_at: null,
      agents: ["urn:filesquid:agent:unrecorded:installation"],
    };
  });
  const rows = page.locator("fs-provenance-panel li.activity");
  const oldest = rows.last();
  await expect(oldest.locator(".chip.reconstructed")).toHaveText(
    label(labels, "provenanceReconstructed"),
  );
  await oldest.locator("button.toggle").click();
  await expect(oldest.locator('.attr[data-name="prov:startedAtTime"] .val')).toHaveText(
    label(labels, "provenanceNotRecorded"),
  );
  await expect(oldest.locator('.attr[data-name="prov:wasAssociatedWith"] .val')).toHaveText(
    label(labels, "provenanceAgentUnrecordedInstallation"),
  );
  await expect(rows.first().locator(".chip.reconstructed")).toHaveCount(0);
  expect(errors).toEqual([]);
});

test("in-tab search filters by kind, operation and version, and Clear filter restores", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await openProvenance(page);
  const prov = page.locator("fs-provenance-panel");
  const filter = prov.locator("input.filter");
  await filter.fill("restore");
  await expect(prov.locator("li.activity")).toHaveCount(1);
  await expect(prov.locator(".count")).toHaveText(
    fill("provenanceCountFiltered", { shown: "1", count: "4" }),
  );
  await filter.fill("op-2");
  await expect(prov.locator("li.activity .kind")).toHaveText("remove");
  await filter.fill("");
  await expect(prov.locator("li.activity")).toHaveCount(4);
  expect(errors).toEqual([]);
});

test("copy puts the whole value on the clipboard and announces it", async ({ page, context }) => {
  const errors = trackErrors(page);
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await openProvenance(page);
  const prov = page.locator("fs-provenance-panel");
  const attribute = prov.locator(
    'li.activity >> nth=0 >> .attr[data-name="prov:wasAssociatedWith"]',
  );
  await attribute.locator("dd").first().hover();
  await attribute.locator("button.copy").first().click();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(PROV_INSTALLATION);
  await expect(prov.locator("p.announce")).toHaveText(
    fill("filePanelCopied", { property: "prov:wasAssociatedWith" }),
  );
  expect(errors).toEqual([]);
});

test("View in Logs opens the Logs address filtered to the file and to an operation", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await openProvenance(page);
  const prov = page.locator("fs-provenance-panel");
  await expect(prov.locator(".footer a.logs")).toHaveAttribute("href", `#/logs?file=${FILE_ID}`);
  const own = prov.locator("li.activity").first().locator("a.logs");
  await expect(own).toHaveAttribute("href", "#/logs?operation_id=op-4");
  await expect(own).toHaveAttribute(
    "aria-label",
    fill("provenanceViewOperationInLogs", { kind: "metadata edit", version: "v4" }),
  );
  await prov.locator(".footer a.logs").click();
  await expect(page).toHaveURL(new RegExp(`#/logs\\?file=${FILE_ID}$`));
  expect(errors).toEqual([]);
});

test("JSON-LD saves the file's history and announces it; a failure is an alert", async ({
  page,
}) => {
  const errors = trackErrors(page, /status of 500 \(/);
  const api = await openProvenance(page);
  const prov = page.locator("fs-provenance-panel");
  const button = prov.locator(".footer button.export");
  await expect(button).toHaveAttribute("aria-label", label(labels, "provenanceExportName"));
  const [download] = await Promise.all([page.waitForEvent("download"), button.click()]);
  expect(download.suggestedFilename()).toBe("facade-study.jpg.history.jsonld");
  expect(api.provenance.exports).toBe(1);
  await expect(prov.locator(".footer [role=status]")).toHaveText(
    fill("provenanceExported", { name: "facade-study.jpg" }),
  );
  api.provenance.truncatedExport = true;
  await Promise.all([page.waitForEvent("download"), button.click()]);
  await expect(prov.locator(".footer [role=status]")).toContainText(
    label(labels, "provenanceExportPartial"),
  );
  // A failed export is an alert, saves nothing and keeps the status line empty.
  api.provenance.failExport(500, "internal");
  await button.click();
  await expect(prov.locator(".footer [role=alert]")).toHaveText(
    label(labels, "provenanceExportFailed"),
  );
  await expect(prov.locator(".footer [role=status]")).toHaveText("");
  expect(api.provenance.exports).toBe(3);
  expect(errors).toEqual([]);
});

test("a record that cannot be read is an alert inside its activity; Try again reads it", async ({
  page,
}) => {
  const errors = trackErrors(page, /status of 500 \(/);
  const api = await openProvenance(page, (a) => a.provenance.failRecords.add("v4"));
  const first = page.locator("fs-provenance-panel li.activity").first();
  await expect(first.locator("[role=alert]")).toContainText(
    fill("provenanceRecordError", { version: "v4" }),
  );
  api.provenance.failRecords.clear();
  await first.locator("button.retry-record").click();
  await expect(first.locator('.attr[data-name="prov:used"]')).toBeVisible();
  expect(errors).toEqual([]);
});

test("an empty history is its own state", async ({ page }) => {
  const errors = trackErrors(page);
  await openPanel(page, (api) => {
    api.provenance.entries = [];
  });
  await selectTab(page, "filePanelProvenance");
  const prov = page.locator("fs-provenance-panel");
  await expect(prov).toHaveAttribute("data-state", "empty");
  await expect(prov.locator(".state .title")).toHaveText(label(labels, "provenanceEmpty"));
  await expect(prov.locator(".state .detail")).toHaveText(label(labels, "provenanceEmptyHint"));
  expect(errors).toEqual([]);
});
