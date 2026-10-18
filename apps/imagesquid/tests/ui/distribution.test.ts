/**
 * UI test: the FileSquid shell rendered as the ImageSquid distribution, with the brand from
 * the ImageSquid manifest and brand assets (IS-S-009) and the shared theme control.
 */
import { beforeEach, expect, it } from "vitest";
import { ThemeStore, defineElements, type FsAppShell, type ThemeMedia } from "@filesquid/ui";
import manifest from "../../distribution.json";
import { imagesquid } from "../../src/distribution";

/** A `prefers-color-scheme: dark` query that never changes. */
const media = (matches: boolean): ThemeMedia => ({
  matches,
  addEventListener: () => undefined,
  removeEventListener: () => undefined,
});

/** Mount the shell as ImageSquid with a started theme store. */
async function mount(dark: boolean) {
  const theme = new ThemeStore({ media: media(dark), root: document.createElement("html") });
  theme.start();
  const shell = document.createElement("fs-app-shell") as FsAppShell;
  shell.configure({
    distribution: imagesquid,
    client: { health: async () => ({ schema_version: 1, status: "ok", product: "filesquid" }) },
    theme,
  });
  document.body.replaceChildren(shell);
  await new Promise((resolve) => setTimeout(resolve, 0));
  return shell;
}

beforeEach(() => defineElements());

it("takes its id and displayed name from distribution.json (IS-S-009)", () => {
  expect(imagesquid.id).toBe(manifest.id);
  expect(imagesquid.productName).toBe(manifest.productName);
  expect(imagesquid.productName).toBe("ImageSquid");
  expect(imagesquid.logo).toEqual({ src: "/imagesquid-logo.svg", alt: "ImageSquid logo" });
});

it("renders ImageSquid brand and vocabulary in the shared shell", async () => {
  const shell = await mount(false);
  const root = shell.shadowRoot;
  expect(root?.querySelector("h1")?.textContent).toBe("ImageSquid");
  expect(root?.querySelector("img")?.getAttribute("alt")).toBe("ImageSquid logo");
  expect([...(root?.querySelectorAll("nav li") ?? [])].map((li) => li.textContent)).toEqual([
    "Libraries",
    "Albums",
    "Galleries",
    "Exhibits",
    "Monitor",
  ]);
  expect(root?.querySelector('[role="status"]')?.textContent).toBe("Archives API: ok (filesquid)");
});

it("keeps the official mark in the dark theme and shows the theme control", async () => {
  const shell = await mount(true);
  const root = shell.shadowRoot;
  expect(root?.querySelector("header img")?.getAttribute("src")).toBe("/imagesquid-logo.svg");
  const control = root?.querySelector("header fs-theme-control");
  expect(control?.shadowRoot?.querySelector("legend")?.textContent).toBe("Theme");
  expect(
    [...(control?.shadowRoot?.querySelectorAll("label") ?? [])].map((l) => l.textContent),
  ).toEqual(["System", "Light", "Dark"]);
});
