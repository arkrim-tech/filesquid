/**
 * UI tests for `<fs-states>` (UI-F2.1a): five distinct states worded from the label table,
 * announced through persistent live regions, with Retry for error and offline only, and focus
 * recovery when Retry goes away (UI-S-016).
 */
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  accessibilityViolations,
  defineElements,
  type FsStates,
  type ViewState,
} from "../../src/index";
import labels from "../../src/theme/labels.en.json";

beforeEach(() => defineElements());

/** Mount `<fs-states>` showing `state`. */
function mount(state: ViewState): FsStates {
  const states = document.createElement("fs-states") as FsStates;
  states.configure({ labels });
  states.state = state;
  document.body.replaceChildren(states);
  return states;
}

/** Text of each child of a region, joined by single spaces. */
function text(states: FsStates, selector: string): string {
  const region = states.shadowRoot?.querySelector(selector);
  return [...(region?.children ?? [])].map((child) => child.textContent ?? "").join(" ");
}

describe("<fs-states>", () => {
  it("is a defined custom element", () => {
    expect(customElements.get("fs-states")).toBeDefined();
  });

  it.each([
    [{ kind: "loading" }, "status", labels.stateLoading],
    [{ kind: "empty" }, "status", labels.stateEmpty],
    [{ kind: "empty", message: labels.collections }, "status", labels.collections],
    [{ kind: "error" }, "alert", `${labels.stateError} ${labels.stateRetry}`],
    [{ kind: "offline" }, "alert", `${labels.stateOffline} ${labels.stateRetry}`],
    [{ kind: "denied" }, "alert", labels.stateDenied],
    [
      { kind: "denied", reason: "not_permitted" },
      "alert",
      `${labels.stateDenied} ${labels.authNotPermitted}`,
    ],
  ] as [ViewState, string, string][])("shows %o in the %s region", (state, role, expected) => {
    const states = mount(state);
    const other = role === "status" ? "alert" : "status";
    expect(text(states, `[role="${role}"]`)).toBe(expected);
    expect(text(states, `[role="${other}"]`)).toBe("");
    expect(states.dataset.state).toBe(state.kind);
    expect(accessibilityViolations(states)).toEqual([]);
  });

  it("keeps the same live regions across state changes, so they are announced", () => {
    const states = mount({ kind: "loading" });
    const status = states.shadowRoot?.querySelector('[role="status"]');
    const alert = states.shadowRoot?.querySelector('[role="alert"]');
    states.state = { kind: "error" };
    states.state = { kind: "empty" };
    expect(states.shadowRoot?.querySelector('[role="status"]')).toBe(status);
    expect(states.shadowRoot?.querySelector('[role="alert"]')).toBe(alert);
  });

  it("puts the shared accessibility stylesheet first", () => {
    const states = mount({ kind: "loading" });
    expect(states.shadowRoot?.querySelector("style")?.textContent).toMatch(
      /^\s*:focus-visible \{ outline: 2px solid var\(--fs-primary\)/,
    );
  });

  it("asks for a retry with a bubbling, composed fs-retry event", () => {
    const states = mount({ kind: "offline" });
    const listener = vi.fn();
    document.body.addEventListener("fs-retry", listener);
    states.shadowRoot?.querySelector<HTMLButtonElement>("button")?.click();
    expect(listener).toHaveBeenCalledOnce();
    expect((listener.mock.calls[0]?.[0] as Event).composed).toBe(true);
    document.body.removeEventListener("fs-retry", listener);
  });

  it("moves focus to the loading message when Retry is removed", () => {
    const states = mount({ kind: "error" });
    states.shadowRoot?.querySelector<HTMLButtonElement>("button")?.focus();
    states.state = { kind: "loading" };
    const active = states.shadowRoot?.activeElement;
    expect(active?.getAttribute("role")).toBe("status");
    expect(active?.getAttribute("tabindex")).toBe("-1");
  });

  it("does not take focus when the state changes while focus is elsewhere", () => {
    const outside = document.createElement("button");
    outside.textContent = "outside";
    const states = mount({ kind: "loading" });
    document.body.append(outside);
    outside.focus();
    states.state = { kind: "error" };
    expect(document.activeElement).toBe(outside);
  });

  it("re-words itself when configured with another table", () => {
    const states = mount({ kind: "loading" });
    states.configure({ labels: { ...labels, stateLoading: "Chargement" } });
    expect(text(states, '[role="status"]')).toBe("Chargement");
  });
});
