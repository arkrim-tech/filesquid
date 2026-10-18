/**
 * UI tests for <fs-theme-control>: a labelled group of three native radio buttons
 * (System, Light, Dark) bound to the shared theme store (SRS-UI-007).
 */
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { FsThemeControl, defineElements } from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import { fakeThemeStore } from "../support/theme-fakes";

/** Mount a control for a fresh store. */
function mount(dark = false) {
  const fake = fakeThemeStore(dark);
  const control = document.createElement("fs-theme-control") as FsThemeControl;
  control.configure({ store: fake.store, labels });
  document.body.replaceChildren(control);
  return { control, ...fake };
}

/** The radio input for one choice. */
const radio = (control: FsThemeControl, value: string) =>
  control.shadowRoot?.querySelector<HTMLInputElement>(`input[value="${value}"]`);

describe("<fs-theme-control>", () => {
  beforeEach(() => defineElements());
  afterEach(() => document.body.replaceChildren());

  it("is registered as a custom element", () => {
    expect(customElements.get("fs-theme-control")).toBe(FsThemeControl);
  });

  it("renders a labelled group of three radios worded from the label table", () => {
    const { control } = mount();
    const root = control.shadowRoot;
    expect(root?.querySelector("fieldset legend")?.textContent).toBe("Theme");
    const options = [...(root?.querySelectorAll("label") ?? [])];
    expect(options.map((label) => label.textContent?.trim())).toEqual(["System", "Light", "Dark"]);
    const inputs = [...(root?.querySelectorAll<HTMLInputElement>("input") ?? [])];
    expect(inputs.map((input) => [input.type, input.name, input.value])).toEqual([
      ["radio", "theme", "system"],
      ["radio", "theme", "light"],
      ["radio", "theme", "dark"],
    ]);
    expect(options.every((label) => label.querySelector("input"))).toBe(true);
  });

  it("checks System by default, following the OS", () => {
    const { control } = mount(true);
    expect(radio(control, "system")?.checked).toBe(true);
    expect(control.dataset.resolved).toBe("dark");
  });

  it("sets the store's choice when a radio is chosen", () => {
    const { control, store, root, data } = mount();
    const dark = radio(control, "dark");
    if (!dark) throw new Error("no dark radio");
    dark.checked = true;
    dark.dispatchEvent(new Event("change", { bubbles: true }));
    expect(store.choice).toBe("dark");
    expect(root.getAttribute("data-theme")).toBe("dark");
    expect(data.get("filesquid.theme")).toBe("dark");
    expect(control.dataset.resolved).toBe("dark");
  });

  it("follows a choice made elsewhere (another control or a restored setting)", () => {
    const { control, store } = mount();
    store.setChoice("light");
    expect(radio(control, "light")?.checked).toBe(true);
    expect(radio(control, "system")?.checked).toBe(false);
  });

  it("stops listening once removed from the document", () => {
    const { control, store } = mount();
    control.remove();
    store.setChoice("dark");
    expect(radio(control, "dark")?.checked).toBe(false);
  });

  it("renders nothing until configured", () => {
    const control = document.createElement("fs-theme-control");
    document.body.replaceChildren(control);
    expect(control.shadowRoot?.querySelector("fieldset")).toBeNull();
  });
});
