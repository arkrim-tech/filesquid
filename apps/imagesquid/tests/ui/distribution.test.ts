/**
 * UI test: the FileSquid shell rendered as the ImageSquid distribution, with the brand from
 * the ImageSquid manifest and brand assets (IS-S-009) in the UI-S-025 frame: the sidebar holds
 * the mark and name, the header holds no theme control and no API status (owner answers 7 and
 * 12), and the theme control lives on the shared Settings page.
 */
import { beforeEach, expect, it } from "vitest";
import {
  MemoryNavigationSource,
  NavigationStore,
  ThemeStore,
  accessibilityViolations,
  breadcrumbItems,
  defineElements,
  resolveLocale,
  type FsAppShell,
  type FsBreadcrumb,
  type FsNavTree,
  type FsStates,
  type ThemeMedia,
} from "@filesquid/ui";
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
    client: {
      host: async () => ({
        schema_version: 1,
        host_type: "desktop",
        operator_name: null,
        hosted_service: {
          enrolled: false,
          backup_enabled: false,
          sync_enabled: false,
          service_host: null,
        },
      }),
      subscribe: () => () => undefined,
    },
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
    "All photos",
    "Recent",
    "Libraries",
    "Albums",
    "Galleries",
    "Exhibits",
    "Recently deleted",
    "Create or open a library",
    "Settings",
    "Logs",
  ]);
  expect(root?.querySelector("nav .new")?.textContent).toBe(imagesquid.labels.fileNew);
  expect(root?.querySelector("header .monitor")?.textContent).toBe(imagesquid.labels.monitor);
  expect(root?.querySelector("header")?.textContent).not.toContain("API");
});

it("inherits the FileSquid accessibility contracts (UI-S-016)", async () => {
  const shell = await mount(false);
  const root = shell.shadowRoot;
  expect(accessibilityViolations(shell)).toEqual([]);
  expect(root?.querySelector(".skip")?.textContent).toBe(imagesquid.labels.skipToContent);
  expect(root?.querySelector("nav")?.getAttribute("aria-label")).toBe(
    imagesquid.labels.navigationMain,
  );
  expect(resolveLocale(imagesquid)).toEqual({ lang: "en", dir: "ltr" });
});

it("keeps the official mark in the dark theme and no theme control in the header", async () => {
  const shell = await mount(true);
  const root = shell.shadowRoot;
  expect(root?.querySelector("nav .brand img")?.getAttribute("src")).toBe("/imagesquid-logo.svg");
  expect(root?.querySelector("fs-theme-control")).toBeNull();
});

it("words the navigation tree, breadcrumb and states in ImageSquid's vocabulary (UI-F2.1a)", async () => {
  const { labels } = imagesquid;
  const source = new MemoryNavigationSource({
    node: "r",
    kind: "repository",
    name: null,
    children: [
      {
        node: "l1",
        kind: "library",
        name: "Family",
        children: [{ node: "a1", kind: "collection", name: "Coastal Days" }],
      },
    ],
  });
  const store = new NavigationStore({});
  store.start();
  const tree = document.createElement("fs-nav-tree") as FsNavTree;
  tree.configure({ labels, source, store });
  const crumb = document.createElement("fs-breadcrumb") as FsBreadcrumb;
  crumb.configure({ labels });
  const { ancestors } = await source.list("a1");
  crumb.items = breadcrumbItems(ancestors, labels);
  const states = document.createElement("fs-states") as FsStates;
  states.configure({ labels });
  states.state = { kind: "offline" };
  document.body.replaceChildren(tree, crumb, states);
  await new Promise((resolve) => setTimeout(resolve, 0));
  const treeRoot = tree.shadowRoot;
  expect([...(treeRoot?.querySelectorAll("section h2") ?? [])].map((h) => h.textContent)).toEqual([
    "Albums",
    "Galleries",
    "Folders",
  ]);
  expect(treeRoot?.querySelector("button.down")?.getAttribute("aria-label")).toBe(
    "Move Albums down",
  );
  expect([...(crumb.shadowRoot?.querySelectorAll("a") ?? [])].map((a) => a.textContent)).toEqual([
    "Repo",
    "Family",
    "Coastal Days",
  ]);
  expect(crumb.shadowRoot?.querySelector("nav")?.getAttribute("aria-label")).toBe(
    labels.navigationBreadcrumb,
  );
  expect(states.shadowRoot?.querySelector('[role="alert"] p')?.textContent).toBe(
    labels.stateOffline,
  );
  for (const element of [tree, crumb, states]) {
    expect(accessibilityViolations(element)).toEqual([]);
  }
});
