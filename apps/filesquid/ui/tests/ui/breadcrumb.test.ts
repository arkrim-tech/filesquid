/**
 * UI tests for `<fs-breadcrumb>` (UI-F2.1a): a navigation landmark named from the label table
 * holding an ordered list of links, the last marked as the current page; hidden when empty.
 */
import { beforeEach, describe, expect, it } from "vitest";
import {
  accessibilityViolations,
  defineElements,
  type BreadcrumbItem,
  type FsBreadcrumb,
} from "../../src/index";
import labels from "../../src/theme/labels.en.json";

beforeEach(() => defineElements());

const trail: BreadcrumbItem[] = [
  { node: "col-1", name: "Holidays", href: "#/collection/col-1" },
  { node: "fold-1", name: "2025", href: "#/collection/col-1/2025" },
];

/** Mount a breadcrumb showing `items`. */
function mount(items: readonly BreadcrumbItem[]): FsBreadcrumb {
  const crumb = document.createElement("fs-breadcrumb") as FsBreadcrumb;
  crumb.configure({ labels });
  crumb.items = items;
  document.body.replaceChildren(crumb);
  return crumb;
}

describe("<fs-breadcrumb>", () => {
  it("is a defined custom element", () => {
    expect(customElements.get("fs-breadcrumb")).toBeDefined();
  });

  it("names its nav from the label table and lists the trail in order", () => {
    const crumb = mount(trail);
    const root = crumb.shadowRoot;
    expect(root?.querySelector("nav")?.getAttribute("aria-label")).toBe(
      labels.navigationBreadcrumb,
    );
    const links = [...(root?.querySelectorAll("ol > li > a") ?? [])];
    expect(links.map((a) => a.textContent)).toEqual(["Holidays", "2025"]);
    expect(links.map((a) => a.getAttribute("href"))).toEqual(trail.map((item) => item.href));
    expect(links.map((a) => a.getAttribute("aria-current"))).toEqual([null, "page"]);
    expect(crumb.hidden).toBe(false);
    expect(accessibilityViolations(crumb)).toEqual([]);
  });

  it("is hidden when the trail is empty", () => {
    const crumb = mount([]);
    expect(crumb.hidden).toBe(true);
    expect(crumb.shadowRoot?.querySelectorAll("li")).toHaveLength(0);
  });

  it("moves focus to the new current page when a re-render removes the focused link", () => {
    const crumb = mount(trail);
    crumb.shadowRoot?.querySelector<HTMLAnchorElement>("a")?.focus();
    crumb.items = [trail[0] as BreadcrumbItem];
    expect(crumb.shadowRoot?.activeElement?.getAttribute("aria-current")).toBe("page");
  });
});
