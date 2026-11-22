/**
 * Packaged-window proof of the distribution's brand (Task DE-F3.1; DE-S-007, UI-S-025,
 * UI-S-033, ADR 035 Decision 3): in a real Tauri webview the sidebar shows the product name
 * and the logo mark, Settings → About shows the logo, the product name with its version and
 * the statement that the macOS build is not signed or notarised by Apple yet, and nothing in
 * the window says "early access" (owner decision 2026-10-19).
 *
 * The binary under test must embed the production UI: build `apps/filesquid/ui` (`npm run
 * build`) before the test-instrumented binary. A binary built over the build script's stub
 * page fails the first test with that instruction.
 *
 * Environment (for a distribution other than the default):
 * - `FILESQUID_E2E_PRODUCT`: the product name the overlay under test gives (default
 *   `FileSquid`).
 * - `FILESQUID_E2E_LOGO`: a text the logo's file name contains (default `filesquid-mark`).
 *
 * Each `browser.execute` body runs in the app page, so it cannot use this module's scope;
 * what it needs is passed in as arguments, and the page-side helpers are defined inside it.
 */
import { browser, expect } from "@wdio/globals";

const PRODUCT = process.env.FILESQUID_E2E_PRODUCT ?? "FileSquid";
const LOGO = process.env.FILESQUID_E2E_LOGO ?? "filesquid-mark";

/** The About statement, in the label-table wording of mockup `32-settings.svg` (UI-S-033). */
const UNSIGNED_STATEMENT = "This macOS build is not signed or notarised by Apple yet.";

/** Any spelling of the label the app must not show. */
const EARLY_ACCESS = /early[\s _-]*access/i;

/**
 * Read the brand the window shows, through every open shadow root.
 *
 * Runs in the app page.
 *
 * @returns The sidebar's product name and logo, the About box's version line, notes and logo,
 *   the page title, and every text and text alternative the window carries.
 */
function readBrand() {
  /** Every element under `root`, shadow trees included. */
  const everyElement = (root, found = []) => {
    for (const element of root.querySelectorAll("*")) {
      found.push(element);
      if (element.shadowRoot) everyElement(element.shadowRoot, found);
    }
    return found;
  };
  /** What an image shows: its file, whether it decoded, and its text alternative. */
  const image = (img) =>
    img ? { src: img.getAttribute("src") ?? "", drawn: img.naturalWidth > 0, alt: img.alt } : null;
  const elements = everyElement(document);
  const first = (selector) => elements.find((element) => element.matches(selector));
  // Each element's complete text, so a label split across child elements ("<span>Early</span>
  // <span>access</span>") is read as one string, and each shadow root's complete text, which
  // its host's textContent leaves out; plus the text alternatives a reader announces.
  const texts = [
    ...elements
      .filter((element) => !["SCRIPT", "STYLE"].includes(element.tagName))
      .flatMap((element) => [
        element.textContent ?? "",
        ...["alt", "title", "aria-label", "placeholder"].map(
          (name) => element.getAttribute(name) ?? "",
        ),
      ]),
    ...elements
      .filter((element) => element.shadowRoot)
      .map((element) => element.shadowRoot.textContent ?? ""),
  ]
    .map((text) => text.replace(/\s+/g, " ").trim())
    .filter((text) => text !== "");
  return {
    shell: Boolean(first("fs-app-shell")?.shadowRoot),
    title: document.title,
    sidebarName: first(".brand h1")?.textContent ?? null,
    sidebarLogo: image(first(".brand img")),
    aboutVersion: first(".about .version")?.textContent ?? null,
    aboutNotes: elements
      .filter((element) => element.matches(".about .about-note"))
      .map((element) => element.textContent ?? ""),
    aboutLogo: image(first(".about > img")),
    texts,
  };
}

describe("the distribution's brand in a packaged window", () => {
  before(async () => {
    await browser.waitUntil(
      async () => (await browser.execute(() => document.readyState)) === "complete",
      { timeout: 60000, timeoutMsg: "the app page did not load" },
    );
  });

  it("runs the production UI, not the build script's stub page", async () => {
    await browser.waitUntil(async () => (await browser.execute(readBrand)).shell, {
      timeout: 30000,
      timeoutMsg:
        "the window shows no <fs-app-shell>: build apps/filesquid/ui (npm run build) and then " +
        "rebuild the test-instrumented binary, so it embeds the production UI",
    });
  });

  it("shows the product name and the logo mark at the top of the sidebar", async () => {
    await browser.waitUntil(async () => (await browser.execute(readBrand)).sidebarLogo?.drawn, {
      timeout: 30000,
      timeoutMsg: "the sidebar logo did not load",
    });
    const brand = await browser.execute(readBrand);
    expect(brand.sidebarName).toBe(PRODUCT);
    expect(brand.sidebarLogo.src).toContain(LOGO);
    expect(brand.sidebarLogo.alt).not.toBe("");
    expect(brand.title).toBe(PRODUCT);
  });

  it("states in Settings, About that the macOS build is not signed or notarised", async () => {
    await browser.execute(() => {
      window.location.hash = "#/settings";
    });
    await browser.waitUntil(async () => (await browser.execute(readBrand)).aboutLogo?.drawn, {
      timeout: 30000,
      timeoutMsg: "the About box and its logo did not appear on the Settings page",
    });
    const brand = await browser.execute(readBrand);
    expect(brand.aboutNotes).toContain(UNSIGNED_STATEMENT);
    expect(brand.aboutVersion.startsWith(`${PRODUCT} `)).toBe(true);
    expect(brand.aboutVersion).toMatch(/\d+\.\d+\.\d+/);
    expect(brand.aboutLogo.src).toContain(LOGO);
  });

  it("detects an early-access label split across elements in a shadow tree", async () => {
    await browser.execute(() => {
      const probe = document.createElement("p");
      probe.id = "split-label-probe";
      probe.append(Object.assign(document.createElement("span"), { textContent: "Early" }));
      probe.append(" ");
      probe.append(Object.assign(document.createElement("strong"), { textContent: "access" }));
      document.querySelector("fs-app-shell").shadowRoot.append(probe);
    });
    const probed = await browser.execute(readBrand);
    await browser.execute(() => {
      document
        .querySelector("fs-app-shell")
        .shadowRoot.getElementById("split-label-probe")
        .remove();
    });
    expect(probed.texts.some((text) => EARLY_ACCESS.test(text))).toBe(true);
    const cleared = await browser.execute(readBrand);
    expect(cleared.texts.some((text) => EARLY_ACCESS.test(text))).toBe(false);
  });

  it("shows no early-access label on the Settings page or the file browser", async () => {
    const settings = await browser.execute(readBrand);
    expect(settings.texts.length).toBeGreaterThan(10);
    expect(settings.texts.filter((text) => EARLY_ACCESS.test(text))).toEqual([]);
    await browser.execute(() => {
      window.location.hash = "#/";
    });
    await browser.waitUntil(
      async () => (await browser.execute(readBrand)).aboutNotes.length === 0,
      { timeout: 30000, timeoutMsg: "the window did not leave the Settings page" },
    );
    const browserPage = await browser.execute(readBrand);
    expect(browserPage.texts.length).toBeGreaterThan(10);
    expect(browserPage.texts.filter((text) => EARLY_ACCESS.test(text))).toEqual([]);
    expect(EARLY_ACCESS.test(browserPage.title)).toBe(false);
  });
});
