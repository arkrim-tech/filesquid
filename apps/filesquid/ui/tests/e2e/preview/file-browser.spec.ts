/**
 * Production-artifact proof of `<fs-file-browser>` (UI-F2.2, UI-S-002, UI-S-019, UI-S-014,
 * UI-S-016): the FileSquid UI built by `vite build` and served by `vite preview`, with the file
 * routes answered by the test from an in-memory object that answers as the API does. Strict DOM
 * assertions: custom elements defined, every word from the active label table, computed token
 * values exact in light and dark, no console errors, real media decoded through content handles,
 * display names only, and the handle never anywhere but a media `src`.
 */
import { expect, test, type Page } from "@playwright/test";
import { MemoryFileSource } from "../../../src/files/memory-source";
import { PNG, WEBM, serveFiles, wav, type FilesApi } from "../support/files-api";
import {
  activeLabels,
  expectElementsDefined,
  label,
  setScheme,
  tokenColour,
  trackErrors,
} from "../support/strict-dom";

const labels = activeLabels();

/** A label with its placeholders filled, the way the UI words it. */
function fill(key: string, values: Record<string, string>): string {
  return label(labels, key).replace(
    /\{([a-zA-Z]+)\}/g,
    (whole, name: string) => values[name] ?? whole,
  );
}
const LONG_TEXT = `${"0123456789abcdef".repeat(4096)}\nTAIL-MARKER`;
const STORED = [
  "3f9a0001",
  "3f9a0002",
  "3f9a0003",
  "3f9a0004",
  "3f9a0005",
  "3f9a0006",
  "3f9a0007",
  "3f9a0008",
  "3f9a0009",
];

/** The fixture object: stored names that differ from the names users see. */
function fixture(extra: { path: string; content: string }[] = []): MemoryFileSource {
  return new MemoryFileSource({
    library_id: "lib",
    object_id: "obj",
    files: [
      { path: "Contracts/3f9a0001.pdf", displayName: "harbour-masterplan-v7.pdf", content: "%PDF" },
      { path: "Contracts/3f9a0002.pdf", displayName: "tender-response.pdf", content: "%PDF" },
      { path: "Drawings/3f9a0003.txt", displayName: "notes.txt", content: "drawing notes" },
      { path: "3f9a0004.png", displayName: "facade-study.png", content: PNG },
      { path: "3f9a0005.txt", displayName: "readme.txt", content: "hello world" },
      { path: "3f9a0006.blend", displayName: "model-export.blend", content: "blend" },
      { path: "3f9a0007.webm", displayName: "site-walkthrough.webm", content: WEBM },
      { path: "3f9a0008.wav", displayName: "interview.wav", content: wav(2) },
      { path: "3f9a0009.txt", displayName: "long.txt", content: LONG_TEXT },
      ...extra,
    ],
  });
}

/**
 * Open the file browser on a route and wait for its first listing.
 *
 * @param page - The page.
 * @param source - The object to serve.
 * @param hash - The route.
 * @returns What the fake API saw.
 */
async function open(page: Page, source = fixture(), hash = "#/files/lib/obj"): Promise<FilesApi> {
  const api = await serveFiles(page, source);
  await page.goto(`/${hash}`);
  await expect(page.locator("fs-file-browser")).toHaveAttribute("data-state", "ready");
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

/** The name controls of the rows, in order. */
const rowNames = (page: Page) => page.locator("fs-file-browser .row .name");

test("the browser is a defined element and lists display names in the list layout from the label table", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await open(page);
  await expectElementsDefined(page, [
    "fs-file-browser",
    "fs-file-preview",
    "fs-states",
    "fs-breadcrumb",
  ]);
  await expect(page.getByRole("table", { name: label(labels, "files") })).toBeVisible();
  await expect(page.getByRole("columnheader")).toHaveText(
    [
      "fileColumnName",
      "fileColumnSize",
      "fileColumnType",
      "fileColumnModified",
      "fileColumnVersion",
    ].map((key) => label(labels, key)),
  );
  await expect(rowNames(page)).toHaveText([
    "Contracts",
    "Drawings",
    "facade-study.png",
    "readme.txt",
    "model-export.blend",
    "site-walkthrough.webm",
    "interview.wav",
    "long.txt",
  ]);
  await expect(page.getByRole("toolbar", { name: label(labels, "fileToolbar") })).toBeVisible();
  await expect(page.getByRole("button", { name: label(labels, "fileViewList") })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await expect(page.locator("fs-file-browser .summary")).toContainText(
    fill("fileSummary", { folders: "2", files: "6", size: "" }).replace(/ Loaded size:.*$/, ""),
  );
  const everything = await deepText(page);
  for (const stored of STORED) expect(everything).not.toContain(stored);
  expect(errors).toEqual([]);
});

test("a file row shows size, type hint, modified date and version", async ({ page }) => {
  await open(page);
  const row = page.locator("fs-file-browser .row", { hasText: "readme.txt" });
  await expect(row.locator(".size")).toHaveText("11 B");
  await expect(row.locator(".type")).toHaveText("TXT");
  await expect(row.locator(".modified time")).toHaveAttribute(
    "datetime",
    "2026-10-16T09:00:00.000Z",
  );
  await expect(row.locator(".version")).toHaveText("v1");
});

test("computed token values are exact in the browser in light and dark", async ({ page }) => {
  const errors = trackErrors(page);
  for (const scheme of ["light", "dark"] as const) {
    await setScheme(page, scheme);
    await open(page);
    const upload = page.getByRole("button", { name: label(labels, "fileUpload") });
    const colours = await upload.evaluate((el) => {
      const style = getComputedStyle(el);
      return { border: style.borderTopColor, text: style.color };
    });
    expect(colours.border).toBe(await tokenColour(page, "--fs-primary"));
    const name = await rowNames(page)
      .first()
      .evaluate((el) => getComputedStyle(el).color);
    expect(name).toBe(await tokenColour(page, "--fs-on-surface"));
    const surface = await page
      .locator("fs-file-browser .table")
      .evaluate((el) => getComputedStyle(el).backgroundColor);
    expect(surface).toBe(await tokenColour(page, "--fs-surface"));
  }
  expect(errors).toEqual([]);
});

test("opening a folder updates the breadcrumb and the address, and Back returns", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await open(page);
  await rowNames(page).first().click();
  await expect(rowNames(page)).toHaveText(["harbour-masterplan-v7.pdf", "tender-response.pdf"]);
  expect(new URL(page.url()).hash).toBe("#/files/lib/obj/Contracts");
  await expect(page.locator("fs-breadcrumb a")).toHaveText([
    label(labels, "fileRoot"),
    "Contracts",
  ]);
  await page.goBack();
  await expect(rowNames(page).first()).toHaveText("Contracts");
  await page.goForward();
  await expect(rowNames(page)).toHaveText(["harbour-masterplan-v7.pdf", "tender-response.pdf"]);
  await page.locator("fs-breadcrumb a", { hasText: label(labels, "fileRoot") }).click();
  await expect(rowNames(page).first()).toHaveText("Contracts");
  expect(errors).toEqual([]);
});

test("an image previews through a content handle that is used only as the media src", async ({
  page,
}) => {
  const errors = trackErrors(page);
  const api = await open(page);
  await page.getByRole("button", { name: "facade-study.png" }).click();
  await expect(page.locator("fs-file-browser .viewer-title")).toHaveText("facade-study.png");
  const img = page.locator("fs-file-preview img");
  await expect(img).toHaveAttribute("alt", "facade-study.png");
  await expect.poll(() => img.evaluate((el: HTMLImageElement) => el.naturalWidth)).toBe(16);
  const src = (await img.getAttribute("src")) ?? "";
  expect(src).toMatch(/^\/api\/v1\/content\/[A-Za-z0-9_-]{43}$/);
  const handle = src.split("/").pop() ?? "";
  expect(api.mints()).toBe(1);
  expect(api.contentRequests.every((r) => !r.authorised)).toBe(true);
  expect(page.url()).not.toContain(handle);
  const hygiene = await page.evaluate((h) => {
    const links = [...document.querySelectorAll("a")].map((a) => a.href);
    return {
      storage: JSON.stringify([
        { ...localStorage },
        { ...sessionStorage },
        document.cookie,
      ]).includes(h),
      links: links.some((l) => l.includes(h)),
      history: String(history.state ?? "").includes(h),
    };
  }, handle);
  expect(hygiene).toEqual({ storage: false, links: false, history: false });
  // The handle appears once in the page: as the media src, never as text or in another attribute.
  expect((await deepText(page)).split(handle)).toHaveLength(2);
  for (const stored of STORED) expect(await deepText(page)).not.toContain(stored);
  expect(errors).toEqual([]);
});

test("a handle that fails is replaced once; a second failure shows the error and mints no third", async ({
  page,
}) => {
  const errors = trackErrors(page, /status of 410/);
  const api = await open(page);
  api.failContent(2);
  await page.getByRole("button", { name: "facade-study.png" }).click();
  await expect(page.locator("fs-file-preview [role='alert']")).toHaveText(
    label(labels, "filePreviewError"),
  );
  await expect(page.locator("fs-file-preview")).toHaveAttribute("data-state", "error");
  expect(api.mints()).toBe(2);
  await expect(page.locator("fs-file-preview img")).toBeHidden();
  expect(errors).toEqual([]);
});

test("a video resumes at the same position and paused state after one new handle", async ({
  page,
}) => {
  const errors = trackErrors(page);
  const api = await open(page);
  await page.getByRole("button", { name: "site-walkthrough.webm" }).click();
  const video = page.locator("fs-file-preview video");
  await expect(video).toHaveAttribute("aria-label", "site-walkthrough.webm");
  await expect
    .poll(() => video.evaluate((el: HTMLVideoElement) => el.readyState))
    .toBeGreaterThanOrEqual(2);
  await video.evaluate((el: HTMLVideoElement) => {
    el.currentTime = 1;
    el.pause();
  });
  await expect
    .poll(() => video.evaluate((el: HTMLVideoElement) => el.currentTime))
    .toBeCloseTo(1, 0);
  const before = await video.getAttribute("src");
  await video.evaluate((el) => el.dispatchEvent(new Event("error")));
  await expect.poll(() => api.mints()).toBe(2);
  await expect.poll(() => video.getAttribute("src")).not.toBe(before);
  await expect
    .poll(() => video.evaluate((el: HTMLVideoElement) => el.readyState))
    .toBeGreaterThanOrEqual(2);
  const after = await video.evaluate((el: HTMLVideoElement) => ({
    time: el.currentTime,
    paused: el.paused,
  }));
  expect(after.paused).toBe(true);
  expect(after.time).toBeGreaterThan(0.5);
  expect(after.time).toBeLessThan(1.5);
  await expect(page.locator("fs-file-preview")).toHaveAttribute("data-state", "ready");
  expect(errors).toEqual([]);
});

test("audio plays natively and is named by the display name", async ({ page }) => {
  const errors = trackErrors(page);
  await open(page);
  await page.getByRole("button", { name: "interview.wav" }).click();
  const audio = page.locator("fs-file-preview audio");
  await expect(audio).toHaveAttribute("aria-label", "interview.wav");
  await expect
    .poll(() => audio.evaluate((el: HTMLAudioElement) => el.readyState))
    .toBeGreaterThanOrEqual(2);
  expect(await audio.evaluate((el: HTMLAudioElement) => el.duration)).toBeCloseTo(2, 0);
  expect(errors).toEqual([]);
});

test("text previews through one ranged read of the first 64 KiB and says it is cut", async ({
  page,
}) => {
  const errors = trackErrors(page);
  const api = await open(page);
  await page.getByRole("button", { name: "long.txt" }).click();
  const pre = page.locator("fs-file-preview pre");
  await expect(pre).toHaveAttribute("aria-label", "long.txt");
  await expect(pre).not.toContainText("TAIL-MARKER");
  await expect(page.locator("fs-file-preview .note")).toHaveText(
    fill("fileTextTruncated", { size: "64 KB" }),
  );
  expect(api.ranges).toEqual(["bytes=0-65535"]);
  expect(errors).toEqual([]);
});

test("a format it cannot show says so and shows no viewer", async ({ page }) => {
  const errors = trackErrors(page);
  await open(page);
  await page.getByRole("button", { name: "model-export.blend" }).click();
  await expect(page.locator("fs-file-preview .unsupported .title")).toHaveText(
    label(labels, "fileNoPreview"),
  );
  // UI-S-031, mockup 24: the badge, the 22 px heading and the two help lines.
  await expect(page.locator("fs-file-preview .unsupported .help")).toHaveText([
    label(labels, "fileNoPreviewKeep"),
    label(labels, "fileNoPreviewOpenElsewhere"),
  ]);
  const badge = page.locator("fs-file-preview .unsupported .badge");
  await expect(badge).toHaveText("BLEND");
  expect(await badge.evaluate((el) => getComputedStyle(el).backgroundColor)).toBe(
    await tokenColour(page, "--fs-badge-data"),
  );
  const title = await page.locator("fs-file-preview .unsupported .title").evaluate((el) => {
    const css = getComputedStyle(el);
    return { size: css.fontSize, weight: css.fontWeight, family: css.fontFamily };
  });
  expect(title.size).toBe("22px");
  expect(title.weight).toBe("700");
  expect(title.family).toMatch(/^Georgia/);
  await expect(
    page.locator("fs-file-preview img, fs-file-preview video, fs-file-preview pre"),
  ).toHaveCount(0);
  expect(errors).toEqual([]);
});

test("the viewer steps through files, closes with Escape and returns focus to the row", async ({
  page,
}) => {
  await open(page);
  await page.getByRole("button", { name: "readme.txt" }).click();
  await expect(page.locator("fs-file-browser .position")).toHaveText("2 of 6");
  await page.getByRole("button", { name: label(labels, "fileNext") }).click();
  await expect(page.locator("fs-file-browser .viewer-title")).toHaveText("model-export.blend");
  await page.keyboard.press("Escape");
  await expect(page.locator("fs-file-browser .viewer")).toBeHidden();
  await expect(page.getByRole("button", { name: "model-export.blend" })).toBeFocused();
});

test("an upload is one PUT of the client's facts that creates the file's own object", async ({
  page,
}) => {
  const errors = trackErrors(page);
  const api = await open(page);
  await page.locator("fs-file-browser input[type=file]").setInputFiles({
    name: "Café scan.pdf",
    mimeType: "application/pdf",
    buffer: Buffer.from("%PDF-scan"),
  });
  await expect(page.locator("fs-file-browser .frame > .status")).toHaveText(
    fill("fileUploaded", { count: "1", version: "v1" }),
  );
  expect(api.writes).toEqual(["PUT upload Café scan.pdf"]);
  expect(api.uploads).toEqual([
    {
      library_id: "lib",
      name: "Café scan.pdf",
      size: "9",
      media_type: "application/pdf",
      last_modified: expect.stringMatching(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/),
      name_normalization: "nfc",
    },
  ]);
  expect(api.source.uploads).toEqual([
    expect.objectContaining({ name: "Café scan.pdf", size: 9, version: "v1" }),
  ]);
  expect(api.source.head).toBe("v1");
  await expect(page.getByRole("button", { name: "readme.txt" })).toBeVisible();
  expect(errors).toEqual([]);
});

const FILE_ID = "5b1d2c3e-7a4f-4e8b-9c6d-0a1b2c3d4e5f";

/** One file's own object (API-S-026): the original at `original/<file-uuid>.txt`. */
function fileObject(): MemoryFileSource {
  return new MemoryFileSource({
    library_id: "lib",
    object_id: `urn:uuid:${FILE_ID}`,
    files: [{ path: `original/${FILE_ID}.txt`, displayName: "readme.txt", content: "hello" }],
  });
}

/** Open the file browser on the file object's `original/` folder. */
function openFileObject(page: Page): Promise<FilesApi> {
  const hash = `#/files/lib/${encodeURIComponent(`urn:uuid:${FILE_ID}`)}/original`;
  return open(page, fileObject(), hash);
}

test("a rename posts to the renames route, never to moves, and shows the new display name", async ({
  page,
}) => {
  const errors = trackErrors(page);
  const api = await openFileObject(page);
  await expect(rowNames(page)).toHaveText(["readme.txt"]);
  await page.getByRole("button", { name: "readme.txt" }).focus();
  await page.getByRole("button", { name: label(labels, "fileRenameMove") }).click();
  await expect(
    page.getByRole("form", { name: fill("fileRenameOnlyTitle", { name: "readme.txt" }) }),
  ).toBeVisible();
  await expect(page.getByRole("textbox", { name: label(labels, "fileFieldFolder") })).toBeHidden();
  await page.keyboard.type("holiday-notes.txt");
  await page.keyboard.press("Enter");
  await expect(page.locator("fs-file-browser .frame > .status")).toHaveText(
    fill("fileRenamed", { name: "holiday-notes.txt", version: "v2" }),
  );
  await expect(rowNames(page)).toHaveText(["holiday-notes.txt"]);
  expect(api.writes).toEqual([
    `POST renames ${JSON.stringify({
      library_id: "lib",
      file_id: FILE_ID,
      display_name: "holiday-notes.txt",
    })}`,
  ]);
  expect(await deepText(page)).not.toContain(`${FILE_ID}.txt`);
  expect(errors).toEqual([]);
});

test("a refused rename shows the label for its reason and keeps the display name", async ({
  page,
}) => {
  const errors = trackErrors(page, /status of 409/);
  await openFileObject(page);
  await page.route(/\/api\/v1\/files\/renames$/, (route) =>
    route.fulfill({ status: 409, json: { code: "conflict", message: "x", retryable: false } }),
  );
  await page.getByRole("button", { name: "readme.txt" }).focus();
  await page.getByRole("button", { name: label(labels, "fileRenameMove") }).click();
  await page.keyboard.type("other.txt");
  await page.keyboard.press("Enter");
  await expect(page.locator("fs-file-browser .frame > .alert")).toHaveText(
    label(labels, "fileRenameConflict"),
  );
  await expect(rowNames(page)).toHaveText(["readme.txt"]);
  expect(errors).toEqual([]);
});

test("writes are single calls: new folder, move, remove and Undo", async ({ page }) => {
  const errors = trackErrors(page);
  const api = await open(page);
  await page.getByRole("button", { name: label(labels, "fileNewFolder") }).click();
  await expect(page.getByRole("textbox", { name: label(labels, "fileFolderName") })).toBeFocused();
  await page.keyboard.type("Photos");
  await page.keyboard.press("Enter");
  await expect(page.locator("fs-file-browser .frame > .status")).toHaveText(
    fill("fileFolderCreated", { name: "Photos" }),
  );
  await expect(
    page.locator("fs-file-browser .row", { hasText: "Photos" }).locator(".pending"),
  ).toHaveText(label(labels, "fileFolderPending"));

  await page.getByRole("button", { name: "readme.txt" }).focus();
  await page.getByRole("button", { name: label(labels, "fileRenameMove") }).click();
  await expect(
    page.getByRole("form", { name: fill("fileRenameTitle", { name: "readme.txt" }) }),
  ).toBeVisible();
  await page.keyboard.type("notes-final.txt");
  await page.keyboard.press("Enter");
  await expect(page.locator("fs-file-browser .frame > .status")).toHaveText(
    fill("fileMoved", { name: "notes-final.txt", version: "v2" }),
  );

  await page.getByRole("button", { name: "model-export.blend" }).focus();
  await page.getByRole("button", { name: label(labels, "fileRemove") }).click();
  await expect(page.locator("fs-file-browser .frame > .status")).toContainText(
    fill("fileRemoved", { name: "model-export.blend", version: "v3" }),
  );
  await expect(page.getByRole("button", { name: "model-export.blend" })).toHaveCount(0);
  await page.getByRole("button", { name: label(labels, "fileUndo") }).click();
  await expect(page.locator("fs-file-browser .frame > .status")).toHaveText(
    fill("fileRestored", { name: "model-export.blend", version: "v4" }),
  );
  await expect(page.getByRole("button", { name: "model-export.blend" })).toBeVisible();
  expect(api.writes.map((w) => w.split(" ")[1])).toEqual([
    "folders",
    "moves",
    "removals",
    "restorations",
  ]);
  expect(api.writes.at(-1)).toContain('"version":"v2"');
  expect(await deepText(page)).not.toContain("3f9a0005");
  expect(errors).toEqual([]);
});

test("a refused write shows the label for its reason and changes nothing", async ({ page }) => {
  const errors = trackErrors(page, /status of 409/);
  await open(page);
  await page.getByRole("button", { name: "readme.txt" }).focus();
  await page.route(/\/api\/v1\/files\/removals$/, (route) =>
    route.fulfill({ status: 409, json: { code: "conflict", message: "x", retryable: false } }),
  );
  await page.getByRole("button", { name: label(labels, "fileRemove") }).click();
  await expect(page.locator("fs-file-browser .frame > .alert")).toHaveText(
    label(labels, "fileConflict"),
  );
  await expect(page.getByRole("button", { name: "readme.txt" })).toBeVisible();
  expect(errors).toEqual([]);
});

test("a long folder is virtualised, pages in on demand and stays keyboard reachable", async ({
  page,
}) => {
  const errors = trackErrors(page);
  const many = Array.from({ length: 1000 }, (_, i) => ({
    path: `m${String(i).padStart(4, "0")}.txt`,
    displayName: `Document ${String(i).padStart(4, "0")}.txt`,
    content: "x",
  }));
  const api = await open(
    page,
    new MemoryFileSource({ library_id: "lib", object_id: "obj", files: many }),
  );
  expect(await page.locator("fs-file-browser .row").count()).toBeLessThan(60);
  await expect(page.locator("fs-file-browser .table")).toHaveAttribute("aria-rowcount", "-1");
  await page.locator("fs-file-browser .scroller").evaluate((el) => {
    el.scrollTop = el.scrollHeight;
  });
  await expect.poll(() => api.listings.length).toBeGreaterThan(1);
  expect(api.listings[1]?.cursor).not.toBeNull();
  await page.locator("fs-file-browser .scroller").evaluate((el) => {
    el.scrollTop = 48 * 150;
  });
  await expect(page.getByRole("button", { name: "Document 0155.txt" })).toBeVisible();
  expect(await page.locator("fs-file-browser .row").count()).toBeLessThan(60);
  await page.getByRole("button", { name: "Document 0155.txt" }).focus();
  await page.keyboard.press("ArrowDown");
  await expect(page.getByRole("button", { name: "Document 0156.txt" })).toBeFocused();
  await page.keyboard.press("End");
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          document.querySelector("fs-file-browser")?.shadowRoot?.activeElement?.textContent ?? "",
      ),
    )
    .toMatch(/^Document 0(19\d|[2-9]\d\d)\.txt$/);
  expect(errors).toEqual([]);
});

test("the layout does not scroll sideways at 320 px, at 200 % text size, or both, in list and grid", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 640 });
  await open(page);
  for (const grid of [false, true]) {
    if (grid) await page.getByRole("button", { name: label(labels, "fileViewGrid") }).click();
    for (const zoom of ["100%", "200%"]) {
      await page.evaluate((size) => (document.documentElement.style.fontSize = size), zoom);
      const overflow = await page.evaluate(
        () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
      );
      expect(overflow).toBeLessThanOrEqual(0);
    }
  }
});

test("motion stops under reduced motion and focus is the 2px --fs-focus-ring ring", async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await open(page);
  const button = page.getByRole("button", { name: label(labels, "fileNewFolder") });
  expect(await button.evaluate((el) => getComputedStyle(el).transitionDuration)).toBe("0s");
  await page.keyboard.press("Tab");
  await button.focus();
  expect(await button.evaluate((el) => el.matches(":focus-visible"))).toBe(true);
  const outline = await button.evaluate((el) => {
    const s = getComputedStyle(el);
    return {
      width: s.outlineWidth,
      style: s.outlineStyle,
      offset: s.outlineOffset,
      colour: s.outlineColor,
    };
  });
  expect(outline).toEqual({
    width: "2px",
    style: "solid",
    offset: "2px",
    colour: await tokenColour(page, "--fs-focus-ring"),
  });
});

test("a failed listing shows the error state with Try again, and retrying loads it", async ({
  page,
}) => {
  const errors = trackErrors(page, /status of 500/);
  const source = fixture();
  const api = await serveFiles(page, source);
  let fail = true;
  await page.route(/\/api\/v1\/files\/listing\?/, (route) => {
    if (!fail) return route.fallback();
    return route.fulfill({
      status: 500,
      json: { code: "internal", message: "x", retryable: false },
    });
  });
  await page.goto("/#/files/lib/obj");
  await expect(page.locator("fs-file-browser")).toHaveAttribute("data-state", "error");
  await expect(page.locator("fs-file-browser fs-states [role='alert']")).toContainText(
    label(labels, "stateError"),
  );
  fail = false;
  await page.getByRole("button", { name: label(labels, "stateRetry") }).click();
  await expect(page.locator("fs-file-browser")).toHaveAttribute("data-state", "ready");
  expect(api.listings.length).toBeGreaterThan(0);
  expect(errors).toEqual([]);
});
