/**
 * Production-artifact proof of the preview's top bar and zoom chip (Task UI-F2.19, UI-S-031,
 * UI-S-024, UI-S-019, UI-S-005): the FileSquid UI built by `vite build` and served by
 * `vite preview`, at the 1440 x 900 window of mockup 22
 * (`resources/ui-mockups/v3/22-preview-image.svg` and its dark twin), in light and dark. Each test
 * asserts the custom elements are defined, every string comes from the active label table, the
 * bar, the media and the chip sit within 2 px of the mockup's regions, and the constants are the
 * browser's computed styles, with no console error. The mockup draws a 28 px macOS title bar
 * above the window content; the webview has none, so a mockup y of `n` is `n - 28` here.
 *
 * The file panel is open beside the viewer, as mockup 22 draws it, so the content area is the
 * mockup's 880 px. The filmstrip of mockup 22 is deferred (owner decision 2026-10-30), so the
 * tests assert it is absent.
 */
import { crc32, deflateSync } from "node:zlib";
import { expect, test, type Page } from "@playwright/test";
import { MemoryFileSource } from "../../../src/files/memory-source";
import { COLLECTION, PNG, serveFiles, type FilesApi } from "../support/files-api";
import { COLOURS, expectBox, look } from "../support/fidelity";
import {
  activeLabels,
  expectElementsDefined,
  label,
  rgbOf,
  setScheme,
  trackErrors,
  type Scheme,
} from "../support/strict-dom";

const labels = activeLabels();
const SCHEMES: readonly Scheme[] = ["light", "dark"];
const BROWSER = "fs-file-browser";
const BAR = `${BROWSER} .viewer header.bar`;
const IMAGE = "fs-file-preview img";
const CHIP = "fs-file-preview .zoom";
/** The shown file's natural size: 640 px wide in mockup 22's frame is 38 % of 1684. */
const NATURAL = { width: 1684, height: 1123 } as const;
const COLLECTION_NAME = "Harbour project";

/** Fill the placeholders of a label the way the UI does. */
function fill(key: string, values: Record<string, string>): string {
  return label(labels, key).replace(/\{(\w+)\}/g, (whole, name: string) => values[name] ?? whole);
}

/** A PNG chunk: length, type, data and the CRC of type and data. */
function chunk(type: string, data: Buffer): Buffer {
  const body = Buffer.concat([Buffer.from(type, "latin1"), data]);
  const head = Buffer.alloc(4);
  head.writeUInt32BE(data.length);
  const tail = Buffer.alloc(4);
  tail.writeUInt32BE(crc32(body));
  return Buffer.concat([head, body, tail]);
}

/** A flat sky-blue PNG of `width` x `height`, the colour mockup 22 paints its photo's sky. */
function photo(width: number, height: number): Buffer {
  const row = Buffer.alloc(width * 3 + 1);
  for (let x = 0; x < width; x += 1) row.set([0xbf, 0xd7, 0xe6], 1 + x * 3);
  const raw = Buffer.concat(Array.from({ length: height }, () => row));
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header.set([8, 2, 0, 0, 0], 8);
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", header),
    chunk("IDAT", deflateSync(raw, { level: 1 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

const FACADE = photo(NATURAL.width, NATURAL.height);

/** Eleven files of the Harbour project; "facade-study.png" is the third. */
function fixture(): MemoryFileSource {
  const names = [
    "aerial-overview.png",
    "ballast-survey.png",
    "facade-study.png",
    "gallery-north.png",
    "harbour-masterplan.pdf",
    "levels.csv",
    "pier-detail.png",
    "quay-wall.png",
    "site-notes.txt",
    "tender-response.pdf",
    "wharf-section.png",
  ];
  return new MemoryFileSource({
    library_id: "lib",
    object_id: "obj",
    files: names.map((displayName, index) => ({
      path: `3f9a10${String(index).padStart(2, "0")}.${displayName.split(".").pop()}`,
      displayName,
      content:
        displayName === "facade-study.png" ? FACADE : displayName.endsWith(".png") ? PNG : "x",
    })),
  });
}

/** Tell the shown browser which collection it is in, as the route that knows it will. */
async function showCollection(page: Page): Promise<void> {
  await page.locator(BROWSER).evaluate(
    (element, values) => {
      const browser = element as unknown as {
        location: { library_id: string; object_id: string; prefix: string };
      };
      browser.location = {
        ...browser.location,
        collection_id: values.id,
        collection_name: values.name,
      } as never;
    },
    { id: COLLECTION, name: COLLECTION_NAME },
  );
}

/** Serve the object, open its folder in the collection and wait for the listing. */
async function openFolder(page: Page, named = true): Promise<FilesApi> {
  const api = await serveFiles(page, fixture());
  await page.goto("/#/files/lib/obj");
  await expect(page.locator(BROWSER)).toHaveAttribute("data-state", "ready");
  if (named) await showCollection(page);
  return api;
}

/** Open a file and wait until its image has decoded. */
async function openImage(page: Page, name: string): Promise<void> {
  await page.getByRole("button", { name, exact: true }).click();
  await expect(page.locator(`${BROWSER} h2.viewer-title`)).toHaveText(name);
  await expect
    .poll(() =>
      page.locator(IMAGE).evaluate((el: HTMLImageElement) => el.complete && el.naturalWidth),
    )
    .toBeTruthy();
}

/** The bottom border of an element: width and colour. */
function bottomBorder(page: Page, selector: string): Promise<{ width: string; colour: string }> {
  return page.locator(selector).evaluate((el) => {
    const css = getComputedStyle(el);
    return { width: css.borderBottomWidth, colour: css.borderBottomColor };
  });
}

/** The computed value of a token on the document root. */
function token(page: Page, name: string): Promise<string> {
  return page.evaluate(
    (n) => getComputedStyle(document.documentElement).getPropertyValue(n).trim(),
    name,
  );
}

test.use({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });

for (const scheme of SCHEMES) {
  const c = COLOURS[scheme];

  test.describe(`${scheme} theme`, () => {
    test.beforeEach(async ({ page }) => {
      await setScheme(page, scheme);
    });

    test("the top bar names the collection, the badge, the file and its place (22, UI-S-031)", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await openFolder(page);
      await openImage(page, "facade-study.png");
      await expectElementsDefined(page, ["fs-app-shell", "fs-file-browser", "fs-file-preview"]);
      await expect(page.locator("fs-file-panel")).toBeVisible();
      await expectBox(page.locator(BAR), { x: 240, y: 61, width: 880, height: 52 });
      expect(await look(page.locator(BAR))).toMatchObject({ background: rgbOf(c.card) });
      expect(await bottomBorder(page, BAR)).toEqual({ width: "1px", colour: rgbOf(c.outline) });

      const back = page.locator(`${BAR} button.back`);
      await expect(back).toHaveAccessibleName(label(labels, "fileBackToList"));
      await expectBox(back.locator("svg"), { x: 256, width: 16, height: 16 });
      expect(await look(back)).toMatchObject({ colour: rgbOf(c.icon) });

      const collection = page.locator(`${BAR} .collection`);
      await expect(collection).toHaveText(COLLECTION_NAME);
      await expectBox(collection, { x: 280 });
      expect(await look(collection)).toMatchObject({
        size: "13px",
        weight: "400",
        colour: rgbOf(c.variant),
      });

      const badge = page.locator(`${BAR} .badge`);
      await expect(badge).toHaveText("PNG");
      await expectBox(badge, { height: 16 });
      expect(await look(badge)).toMatchObject({
        size: "10px",
        weight: "700",
        transform: "uppercase",
        background: rgbOf("#007af9"),
        colour: rgbOf("#ffffff"),
        radius: "3px",
      });
      expect((await look(badge)).family).toMatch(/mono/i);

      const title = page.locator(`${BAR} h2.viewer-title`);
      expect(await look(title)).toMatchObject({
        size: "14px",
        weight: "600",
        colour: rgbOf(c.heading),
      });
      expect((await look(title)).family).toMatch(/system-ui|-apple-system|BlinkMacSystemFont/);
      const position = page.locator(`${BAR} .position`);
      await expect(position).toHaveText(fill("filePosition", { position: "3", total: "11" }));
      expect(await look(position)).toMatchObject({ size: "13px", colour: rgbOf(c.muted) });
      expect(errors).toEqual([]);
    });

    test("previous and next are 40 px round controls at the media's sides (22, UI-S-031)", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await openFolder(page);
      await openImage(page, "facade-study.png");
      const previous = page.locator(`${BROWSER} button.previous`);
      const next = page.locator(`${BROWSER} button.next`);
      await expect(previous).toHaveAccessibleName(label(labels, "filePrevious"));
      await expect(next).toHaveAccessibleName(label(labels, "fileNext"));
      await expectBox(previous, { x: 264, y: 343, width: 40, height: 40 });
      await expectBox(next, { x: 1056, y: 343, width: 40, height: 40 });
      const pill = await token(page, "--fs-radius-pill");
      for (const control of [previous, next]) {
        expect(await look(control)).toMatchObject({
          background: rgbOf(c.card),
          borderWidth: "1px",
          borderColour: rgbOf(c.outline),
          radius: pill,
          colour: rgbOf(c.icon),
        });
        await expectBox(control.locator("svg"), { width: 20, height: 20 });
      }
      expect(errors).toEqual([]);
    });

    test("the media is fitted in its 8 px frame and the chip states the scale (22, UI-S-031)", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await openFolder(page);
      await openImage(page, "facade-study.png");
      const image = page.locator(IMAGE);
      await expectBox(image, { x: 360, y: 150, width: 640, height: 426 });
      expect(await look(image)).toMatchObject({ radius: "8px", background: rgbOf(c.raised) });
      expect(await look(page.locator(`${BROWSER} .viewer`))).toMatchObject({
        background: rgbOf(scheme === "light" ? "#f5f5f5" : "#1c1c1c"),
      });

      const chip = page.locator(CHIP);
      await expectBox(chip, { y: 596, height: 36 });
      const box = await chip.boundingBox();
      expect(Math.abs((box?.x ?? 0) + (box?.width ?? 0) / 2 - 680)).toBeLessThanOrEqual(2);
      expect(await look(chip)).toMatchObject({
        background: rgbOf(c.card),
        borderWidth: "1px",
        borderColour: rgbOf(c.outline),
        radius: await token(page, "--fs-radius-pill"),
      });
      const fit = page.locator(`${CHIP} .fit`);
      await expect(fit).toHaveText(fill("fileZoomFit", { percent: "38" }));
      expect(await look(fit)).toMatchObject({ size: "13px", weight: "500", colour: rgbOf(c.body) });
      await expect(page.locator(`${CHIP} .loading`)).toBeHidden();
      await expect(page.locator(`${CHIP} [role='progressbar']`)).toBeHidden();
      await expect(page.locator(`${BROWSER} .filmstrip, ${BROWSER} [data-filmstrip]`)).toHaveCount(
        0,
      );
      await expect(page.locator(`${BROWSER} fs-breadcrumb`)).toBeHidden();
      await expect(page.locator(`${BROWSER} .toolbar`)).toBeHidden();
      expect(errors).toEqual([]);
    });

    test("while the full-size image loads the chip says so with its progress bar (22, UI-S-031)", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await openFolder(page);
      let release: () => void = () => undefined;
      const gate = new Promise<void>((resolve) => (release = resolve));
      await page.route(/\/api\/v1\/content\/[A-Za-z0-9_-]{43}$/, async (route) => {
        await gate;
        await route.fallback();
      });
      await page.getByRole("button", { name: "facade-study.png", exact: true }).click();
      const loading = page.locator(`${CHIP} .loading`);
      await expect(loading).toHaveText(label(labels, "fileLoadingFullSize"));
      expect(await look(loading)).toMatchObject({
        size: "13px",
        weight: "400",
        colour: rgbOf(c.muted),
      });
      const bar = page.locator(`${CHIP} [role='progressbar']`);
      await expect(bar).toBeVisible();
      await expectBox(bar, { width: 92, height: 3 });
      expect(await look(bar)).toMatchObject({ background: rgbOf(c.outline) });
      expect(await look(bar.locator(".progress-fill"))).toMatchObject({
        background: rgbOf(c.primary),
      });
      await expect(page.locator(`${BAR} h2.viewer-title`)).toHaveText("facade-study.png");
      release();
      await expect(loading).toBeHidden();
      await expect(page.locator(`${CHIP} .fit`)).toHaveText(fill("fileZoomFit", { percent: "38" }));
      expect(errors).toEqual([]);
    });
  });
}

test("next and previous step through the collection and keep the bar", async ({ page }) => {
  const errors = trackErrors(page);
  await openFolder(page);
  await openImage(page, "facade-study.png");
  await page.getByRole("button", { name: label(labels, "fileNext") }).click();
  await expect(page.locator(`${BAR} h2.viewer-title`)).toHaveText("gallery-north.png");
  await expect(page.locator(`${BAR} .position`)).toHaveText(
    fill("filePosition", { position: "4", total: "11" }),
  );
  await page.getByRole("button", { name: label(labels, "filePrevious") }).click();
  await expect(page.locator(`${BAR} h2.viewer-title`)).toHaveText("facade-study.png");
  await expect(page.locator(`${BAR} .position`)).toHaveText(
    fill("filePosition", { position: "3", total: "11" }),
  );
  await expect(page.locator(`${BAR} .collection`)).toHaveText(COLLECTION_NAME);
  await page.getByRole("button", { name: label(labels, "fileBackToList") }).click();
  await expect(page.locator(`${BROWSER} .viewer`)).toBeHidden();
  await expect(page.locator(`${BROWSER} fs-breadcrumb`)).toBeVisible();
  expect(errors).toEqual([]);
});

test("a location that names no collection shows none, never a guessed one", async ({ page }) => {
  const errors = trackErrors(page);
  await openFolder(page, false);
  await openImage(page, "facade-study.png");
  await expect(page.locator(`${BAR} .collection`)).toBeHidden();
  await expect(page.locator(BAR)).not.toContainText(label(labels, "fileRoot"));
  expect(errors).toEqual([]);
});

test("content that cannot be read keeps the bar and shows the error within 2,000 ms", async ({
  page,
}) => {
  const errors = trackErrors(page, /status of 410/);
  const api = await openFolder(page);
  api.failContent(2);
  await page.getByRole("button", { name: "facade-study.png", exact: true }).click();
  await expect(page.locator("fs-file-preview [role='alert']")).toHaveText(
    label(labels, "filePreviewError"),
    { timeout: 2_000 },
  );
  await expect(page.locator(`${BAR} h2.viewer-title`)).toHaveText("facade-study.png");
  await expect(page.locator(`${BAR} .position`)).toHaveText(
    fill("filePosition", { position: "3", total: "11" }),
  );
  await expect(page.locator(CHIP)).toBeHidden();
  expect(errors).toEqual([]);
});
