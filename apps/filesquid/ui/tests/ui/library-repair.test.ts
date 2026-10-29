/**
 * UI tests for `<fs-library-repair>` (UI-1.1, API-S-023, API-S-025, OCFL-S-020): the repair offer
 * lists the objects that can be read but not changed; the consent dialog names those objects and
 * says that one copy of the root inventory and its sidecar is added to each object's latest
 * version directory and that a new version recording the repair is committed to each object's
 * history, with no file content changed; only Confirm calls the repair route, with the
 * consent for exactly those objects; Cancel and Escape change nothing; the result is shown per
 * object; a refused repair says nothing was changed.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  accessibilityViolations,
  defineElements,
  type FsLibraryRepair,
  type FsStates,
} from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import { CANDIDATES, FIRST_CANDIDATE, FakeLibrarySource, apiError } from "../support/library-fakes";

beforeEach(() => defineElements());
afterEach(() => vi.restoreAllMocks());

/** Let pending promises and renders finish. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

/** Mount the repair offer for `lib-1`. */
async function mount(source = new FakeLibrarySource()) {
  const opener = document.createElement("button");
  const page = document.createElement("fs-library-repair") as FsLibraryRepair;
  page.configure({ labels, source, online: () => true });
  page.libraryId = "lib-1";
  document.body.replaceChildren(opener, page);
  await settle();
  return { page, source };
}

const rootOf = (page: FsLibraryRepair) => page.shadowRoot as ShadowRoot;
const q = <T extends Element>(page: FsLibraryRepair, selector: string) =>
  rootOf(page).querySelector(selector) as T;
const click = async (page: FsLibraryRepair, selector: string) => {
  q<HTMLButtonElement>(page, selector).click();
  await settle();
};
const dialogOf = (page: FsLibraryRepair) => rootOf(page).querySelector('[role="dialog"]');

describe("<fs-library-repair> offer", () => {
  it("is a defined element that says the files can be read but not changed and lists the objects", async () => {
    const { page } = await mount();
    expect(customElements.get("fs-library-repair")).toBeDefined();
    expect(q(page, "h2").textContent).toBe(labels.repairTitle);
    expect(q(page, ".intro").textContent).toBe(labels.repairIntro);
    const items = [...rootOf(page).querySelectorAll('[data-list="objects"] li')].map(
      (li) => li.textContent,
    );
    expect(items).toEqual([
      labels.repairObject
        .replace("{object}", "urn:uuid:obj-a")
        .replace("{head}", "v2")
        .replace("{algorithm}", "sha256"),
      [
        labels.repairObject
          .replace("{object}", "urn:uuid:obj-b")
          .replace("{head}", "v1")
          .replace("{algorithm}", "sha512"),
        labels.repairInterrupted,
        labels.repairStaysReadOnly,
      ].join(" "),
    ]);
    expect(rootOf(page).firstElementChild?.tagName).toBe("STYLE");
    expect(accessibilityViolations(page)).toEqual([]);
  });

  it("links back to the library page", async () => {
    const { page } = await mount();
    const link = q<HTMLAnchorElement>(page, "a.back");
    expect(link.textContent).toBe(labels.libraryBack);
    expect(link.getAttribute("href")).toBe("#/library/lib-1");
  });

  it("says when more objects need repair than are listed", async () => {
    const source = new FakeLibrarySource();
    source.candidates = { ...CANDIDATES, truncated: true };
    const { page } = await mount(source);
    expect(q(page, ".truncated").textContent).toBe(
      labels.repairTruncated.replace("{limit}", (1000).toLocaleString()),
    );
  });

  it("says nothing needs repair and offers no repair when the list is empty", async () => {
    const source = new FakeLibrarySource();
    source.candidates = { candidates: [], truncated: false };
    const { page } = await mount(source);
    expect(q<FsStates>(page, "fs-states").state).toEqual({
      kind: "empty",
      message: labels.repairNothing,
    });
    expect(rootOf(page).querySelector('[data-action="review"]')).toBeNull();
  });

  it("shows a load failure as a state with Retry", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new FakeLibrarySource();
    source.candidates = apiError(503, "unavailable");
    const { page } = await mount(source);
    expect(q<FsStates>(page, "fs-states").state).toEqual({
      kind: "unavailable",
    });
    expect(q<FsStates>(page, "fs-states").shadowRoot?.textContent).toContain(labels.stateRetry);
    expect(rootOf(page).querySelector('[data-action="review"]')).toBeNull();
  });
});

describe("<fs-library-repair> consent", () => {
  it("opens a modal dialog that names the objects and states exactly what the repair adds", async () => {
    const { page } = await mount();
    await click(page, '[data-action="review"]');
    const dialog = dialogOf(page) as HTMLElement;
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    const title = rootOf(page).getElementById(dialog.getAttribute("aria-labelledby") ?? "");
    expect(title?.textContent).toBe(labels.repairDialogTitle.replace("{count}", "2"));
    expect(dialog.querySelector(".consent")?.textContent).toBe(labels.repairConsent);
    expect(labels.repairConsent).toContain("One copy of the root inventory and its sidecar");
    expect(labels.repairConsent).toContain("latest version directory");
    expect(labels.repairConsent).toContain(
      "a new version recording this repair will be committed to each object's history",
    );
    expect(labels.repairConsent).toContain("No file content changes.");
    expect(labels.repairConsent).not.toContain("Nothing else changes.");
    const named = [...dialog.querySelectorAll("li")].map((li) => li.textContent);
    expect(named).toEqual(["urn:uuid:obj-a", "urn:uuid:obj-b"]);
    expect(dialog.querySelector('[data-action="confirm"]')?.textContent).toBe(
      labels.repairConfirm.replace("{count}", "2"),
    );
    expect(dialog.querySelector('[data-action="cancel"]')?.textContent).toBe(labels.repairCancel);
    expect(accessibilityViolations(page)).toEqual([]);
  });

  it("draws mockup 42: one primary per screen and dialog, the overlay dialog on the scrim", async () => {
    const { page } = await mount();
    const root = rootOf(page);
    expect(root.querySelector('[data-action="review"]')?.className).toBe("button primary");
    expect(root.querySelectorAll(".button.primary")).toHaveLength(1);
    await click(page, '[data-action="review"]');
    const dialog = dialogOf(page) as HTMLElement;
    expect(dialog.classList.contains("dialog")).toBe(true);
    expect(dialog.parentElement?.classList.contains("scrim")).toBe(true);
    const kinds = [...dialog.querySelectorAll("button")].map((b) => b.className);
    expect(kinds).toEqual(["button secondary", "button primary"]);
    expect(dialog.querySelector(".dialog-actions")).not.toBeNull();
    const css = root.querySelector("style")?.textContent ?? "";
    expect(css).toMatch(
      /\.scrim::before \{[^}]*background: var\(--fs-scrim\); opacity: var\(--fs-scrim-opacity\)/,
    );
    expect(css).toMatch(
      /\.dialog \{[^}]*inline-size: min\(37\.5rem, 100%\)[^}]*border-radius: var\(--fs-radius-dialog\); background: var\(--fs-surface-overlay\)/,
    );
    expect(css).toMatch(
      /\[role="dialog"\] h3 \{[^}]*font-size: var\(--fs-font-size-heading\); font-weight: 700/,
    );
    expect(css).toMatch(/\.offer li[^{]*\{ font-family: var\(--fs-font-mono\)/);
  });

  it("moves focus into the dialog, to Cancel, without calling the repair route", async () => {
    const { page, source } = await mount();
    await click(page, '[data-action="review"]');
    expect(rootOf(page).activeElement?.getAttribute("data-action")).toBe("cancel");
    expect(source.repairs).toEqual([]);
  });

  it("Cancel closes the dialog, sends nothing, changes nothing and returns focus to the opener", async () => {
    const { page, source } = await mount();
    q<HTMLButtonElement>(page, '[data-action="review"]').focus();
    await click(page, '[data-action="review"]');
    await click(page, '[data-action="cancel"]');
    expect(dialogOf(page)).toBeNull();
    expect(source.repairs).toEqual([]);
    expect(source.calls).toEqual(["candidates:lib-1"]);
    expect(rootOf(page).activeElement?.getAttribute("data-action")).toBe("review");
    expect(rootOf(page).querySelector('[data-section="results"]')).toBeNull();
  });

  it("Confirm keeps focus on the opener while the repair runs", async () => {
    const source = new FakeLibrarySource();
    source.repaired = () => new Promise(() => undefined);
    const { page } = await mount(source);
    await click(page, '[data-action="review"]');
    await click(page, '[data-action="confirm"]');
    expect(rootOf(page).activeElement?.getAttribute("data-action")).toBe("review");
  });

  it("a repair still running for another library does not block this library's offer", async () => {
    const source = new FakeLibrarySource();
    source.repaired = () => new Promise(() => undefined);
    const { page } = await mount(source);
    await click(page, '[data-action="review"]');
    await click(page, '[data-action="confirm"]');
    page.libraryId = "lib-2";
    await settle();
    await click(page, '[data-action="review"]');
    expect(dialogOf(page)).not.toBeNull();
  });

  it("a repair still running for this library cannot be sent again after leaving and returning", async () => {
    const source = new FakeLibrarySource();
    source.repaired = () => new Promise(() => undefined);
    const { page } = await mount(source);
    await click(page, '[data-action="review"]');
    await click(page, '[data-action="confirm"]');
    page.libraryId = "lib-2";
    await settle();
    page.libraryId = "lib-1";
    await settle();
    await click(page, '[data-action="review"]');
    expect(dialogOf(page)).toBeNull();
    expect(source.repairs).toHaveLength(1);
  });

  it("Escape cancels like the Cancel button", async () => {
    const { page, source } = await mount();
    await click(page, '[data-action="review"]');
    (dialogOf(page) as HTMLElement).dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
    );
    await settle();
    expect(dialogOf(page)).toBeNull();
    expect(source.repairs).toEqual([]);
  });

  it("keeps Tab inside the dialog", async () => {
    const { page } = await mount();
    await click(page, '[data-action="review"]');
    const dialog = dialogOf(page) as HTMLElement;
    const confirm = dialog.querySelector<HTMLButtonElement>('[data-action="confirm"]');
    confirm?.focus();
    const tab = new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true });
    confirm?.dispatchEvent(tab);
    expect(tab.defaultPrevented).toBe(true);
    expect(rootOf(page).activeElement?.getAttribute("data-action")).toBe("cancel");
    const back = new KeyboardEvent("keydown", {
      key: "Tab",
      shiftKey: true,
      bubbles: true,
      cancelable: true,
    });
    dialog.querySelector('[data-action="cancel"]')?.dispatchEvent(back);
    expect(rootOf(page).activeElement?.getAttribute("data-action")).toBe("confirm");
  });

  it("Confirm calls the repair route once with the consent for exactly the named objects", async () => {
    const { page, source } = await mount();
    await click(page, '[data-action="review"]');
    await click(page, '[data-action="confirm"]');
    const ids = ["urn:uuid:obj-a", "urn:uuid:obj-b"];
    expect(source.repairs).toEqual([
      {
        library_id: "lib-1",
        object_ids: ids,
        consent: {
          library_id: "lib-1",
          object_ids: ids,
          statement_id: "version_inventory_repair.v2",
          confirmed: true,
        },
      },
    ]);
    expect(dialogOf(page)).toBeNull();
  });

  it("shows the result per object, then reads the candidates again", async () => {
    const { page, source } = await mount();
    await click(page, '[data-action="review"]');
    await click(page, '[data-action="confirm"]');
    const results = [...rootOf(page).querySelectorAll('[data-section="results"] li')].map(
      (li) => li.textContent,
    );
    expect(results).toEqual([
      labels.repairRepaired.replace("{object}", "urn:uuid:obj-a").replace("{version}", "v2"),
      labels.repairRefused
        .replace("{object}", "urn:uuid:obj-b")
        .replace("{code}", "nothing_to_repair"),
    ]);
    expect(rootOf(page).querySelector('[data-section="results"]')?.getAttribute("role")).toBe(
      "status",
    );
    expect(source.calls).toEqual(["candidates:lib-1", "repair:lib-1", "candidates:lib-1"]);
  });

  it("names a provenance record that was not written", async () => {
    const source = new FakeLibrarySource();
    source.repaired = {
      library_id: "lib-1",
      run_id: "r",
      objects: [
        {
          object_id: "urn:uuid:obj-a",
          version: "v2",
          result: "repaired",
          error_code: null,
          retryable: false,
          provenance_error_code: "unavailable",
        },
      ],
    };
    const { page } = await mount(source);
    await click(page, '[data-action="review"]');
    await click(page, '[data-action="confirm"]');
    expect(q(page, '[data-section="results"] li').textContent).toBe(
      labels.repairRepairedNoProvenance
        .replace("{object}", "urn:uuid:obj-a")
        .replace("{version}", "v2")
        .replace("{code}", "unavailable"),
    );
  });

  it.each([
    [apiError(403, "forbidden", "not_permitted"), labels.repairRequestRefusedOwner],
    [apiError(409, "conflict"), labels.repairRequestRefusedConflict],
    [apiError(404, "not_found"), labels.repairRequestRefusedGone],
    [apiError(503, "unavailable"), labels.repairRequestRefusedUnavailable],
  ])("a refused repair (%#) says nothing was changed", async (error, message) => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new FakeLibrarySource();
    source.repaired = error;
    const { page } = await mount(source);
    await click(page, '[data-action="review"]');
    await click(page, '[data-action="confirm"]');
    expect(q(page, '[role="alert"].alert').textContent).toBe(message);
    expect(message).toContain("Nothing was changed");
    expect(rootOf(page).querySelector('[data-section="results"]')).toBeNull();
    expect(dialogOf(page)).toBeNull();
  });

  it("sends nothing and says so when the list cannot be named in a consent (over 1,000 objects)", async () => {
    const source = new FakeLibrarySource();
    const one = FIRST_CANDIDATE;
    source.candidates = {
      candidates: Array.from({ length: 1001 }, (_, i) => ({ ...one, object_id: `o${i}` })),
      truncated: false,
    };
    const { page } = await mount(source);
    expect(rootOf(page).querySelector('[data-action="review"]')).toBeNull();
    expect(q(page, '[role="alert"].alert').textContent).toBe(labels.repairRequestRefused);
  });
});
