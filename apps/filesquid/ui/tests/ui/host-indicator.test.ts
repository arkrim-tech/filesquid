/**
 * UI tests for `<fs-host-indicator>` (UI-S-004, UI-S-018): strict DOM assertions on what it
 * shows for each host type, its loading and error states, and how it follows the host
 * descriptor through `host.change`, `events.resync` and a reconnect.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  ApiError,
  accessibilityViolations,
  buildHostStatement,
  defineElements,
  describeHost,
  type ApiEvent,
  type FsHostIndicator,
  type HostClient,
  type HostDescriptor,
} from "../../src/index";
import labels from "../../src/theme/labels.en.json";

const NONE = { enrolled: false, backup_enabled: false, sync_enabled: false, service_host: null };
const SERVICE = "backup.filesquid.tech";

/** A descriptor with overrides. */
function descriptor(over: Record<string, unknown> = {}): HostDescriptor {
  return {
    schema_version: 1,
    host_type: "desktop",
    operator_name: null,
    hosted_service: NONE,
    ...over,
  } as HostDescriptor;
}

/** A hosted-service state. */
function service(backup: boolean, sync: boolean) {
  return {
    enrolled: true,
    backup_enabled: backup,
    sync_enabled: sync,
    service_host: SERVICE,
  };
}

/** A fake client: queue answers for `host()`, and drive the event channel by hand. */
function fakeClient(...answers: (HostDescriptor | Error)[]) {
  const queue = [...answers];
  const reads = { count: 0 };
  let listener: ((event: ApiEvent) => void) | undefined;
  let onOpen: (() => void) | undefined;
  const subscriptions = { opened: 0, closed: 0 };
  const client: HostClient = {
    host: async () => {
      reads.count += 1;
      const next = queue.length > 1 ? queue.shift() : queue[0];
      if (next instanceof Error) throw next;
      return next as HostDescriptor;
    },
    subscribe: (l, open) => {
      subscriptions.opened += 1;
      listener = l;
      onOpen = open;
      return () => void (subscriptions.closed += 1);
    },
  };
  return {
    client,
    reads,
    subscriptions,
    queue,
    emit: (event: ApiEvent) => listener?.(event),
    open: () => onOpen?.(),
  };
}

/** Wait for pending promise callbacks. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

/** Mount an indicator on a fake client. */
async function mount(fake: { client: HostClient }): Promise<FsHostIndicator> {
  const indicator = document.createElement("fs-host-indicator") as FsHostIndicator;
  indicator.configure({ labels, client: fake.client });
  document.body.replaceChildren(indicator);
  await settle();
  return indicator;
}

/** Text of the first element matching `selector` in the indicator's shadow root. */
function text(indicator: FsHostIndicator, selector: string): string {
  return indicator.shadowRoot?.querySelector(selector)?.textContent?.trim() ?? "";
}

/** Texts of every element matching `selector`. */
function texts(indicator: FsHostIndicator, selector: string): string[] {
  return [...(indicator.shadowRoot?.querySelectorAll(selector) ?? [])].map(
    (el) => el.textContent?.trim() ?? "",
  );
}

beforeEach(() => defineElements());
afterEach(() => vi.restoreAllMocks());

describe("<fs-host-indicator> structure", () => {
  it("is a defined custom element: a named group with a badge button and a closed popover", async () => {
    expect(customElements.get("fs-host-indicator")).toBeDefined();
    const indicator = await mount(fakeClient(descriptor()));
    const group = indicator.shadowRoot?.querySelector('[role="group"]');
    expect(group?.getAttribute("aria-label")).toBe(labels.hostIndicator);
    const badge = indicator.shadowRoot?.querySelector<HTMLButtonElement>("button.badge");
    expect(badge?.type).toBe("button");
    expect(badge?.getAttribute("aria-expanded")).toBe("false");
    expect(badge?.getAttribute("aria-haspopup")).toBe("dialog");
    const popover = indicator.shadowRoot?.querySelector<HTMLElement>('[role="dialog"]');
    expect(badge?.getAttribute("aria-controls")).toBe(popover?.id);
    expect(popover?.hidden).toBe(true);
    expect(text(indicator, '[role="dialog"] h2')).toBe(labels.hostIndicator);
    expect(popover?.getAttribute("aria-labelledby")).toBe(
      indicator.shadowRoot?.querySelector('[role="dialog"] h2')?.id,
    );
    expect(accessibilityViolations(indicator)).toEqual([]);
  });
});

describe("<fs-host-indicator> compact badge", () => {
  it.each([
    ["desktop", descriptor(), "hostBadgeDesktop"],
    ["server", descriptor({ host_type: "server", operator_name: "Ops" }), "hostBadgeServer"],
    ["browser", descriptor({ host_type: "browser" }), "hostBadgeBrowser"],
  ] as const)("shows an icon and the short %s label, not the statement", async (_t, d, key) => {
    const indicator = await mount(fakeClient(d));
    expect(text(indicator, "button.badge .label")).toBe(labels[key]);
    const icon = indicator.shadowRoot?.querySelector("button.badge .icon");
    expect(icon?.getAttribute("aria-hidden")).toBe("true");
    const badge = indicator.shadowRoot?.querySelector("button.badge");
    expect(badge?.textContent).not.toContain(labels.hostDesktop);
  });

  it("says the host is being checked while loading, and unknown when it cannot be read", async () => {
    let release: (value: HostDescriptor) => void = () => undefined;
    const pending = new Promise<HostDescriptor>((resolve) => (release = resolve));
    const loading = await mount({
      client: { host: () => pending, subscribe: () => () => undefined },
    });
    expect(text(loading, "button.badge .label")).toBe(labels.hostBadgeLoading);
    release(descriptor());
    await settle();
    expect(text(loading, "button.badge .label")).toBe(labels.hostBadgeDesktop);
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const failed = await mount(fakeClient(new ApiError(500, "/api/v1/host")));
    expect(text(failed, "button.badge .label")).toBe(labels.hostBadgeUnknown);
  });

  it("announces the short label in a polite live region when the host changes", async () => {
    const fake = fakeClient(descriptor());
    const indicator = await mount(fake);
    const live = indicator.shadowRoot?.querySelector('button.badge .label[aria-live="polite"]');
    expect(live?.textContent).toBe(labels.hostBadgeDesktop);
    fake.emit({ topic: "host.change", data: descriptor({ host_type: "server" }) });
    expect(live?.textContent).toBe(labels.hostBadgeServer);
  });

  it("opens the popover on click with focus in it, and closes on Escape with focus on the badge", async () => {
    const indicator = await mount(fakeClient(descriptor()));
    const root = indicator.shadowRoot as ShadowRoot;
    const badge = root.querySelector<HTMLButtonElement>("button.badge") as HTMLButtonElement;
    const popover = root.querySelector<HTMLElement>('[role="dialog"]') as HTMLElement;
    badge.click();
    expect(badge.getAttribute("aria-expanded")).toBe("true");
    expect(popover.hidden).toBe(false);
    expect(root.activeElement).toBe(popover);
    expect(texts(indicator, '[role="dialog"] .statement')).toEqual([labels.hostDesktop]);
    popover.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(popover.hidden).toBe(true);
    expect(badge.getAttribute("aria-expanded")).toBe("false");
    expect(root.activeElement).toBe(badge);
  });

  it("closes again when the badge is pressed a second time", async () => {
    const indicator = await mount(fakeClient(descriptor()));
    const badge = indicator.shadowRoot?.querySelector<HTMLButtonElement>("button.badge");
    badge?.click();
    badge?.click();
    expect(indicator.shadowRoot?.querySelector<HTMLElement>('[role="dialog"]')?.hidden).toBe(true);
  });

  it("closes on a click outside the indicator, and when focus leaves it", async () => {
    const indicator = await mount(fakeClient(descriptor()));
    const outside = document.createElement("button");
    document.body.append(outside);
    const badge = indicator.shadowRoot?.querySelector<HTMLButtonElement>("button.badge");
    const popover = () => indicator.shadowRoot?.querySelector<HTMLElement>('[role="dialog"]');
    badge?.click();
    outside.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true, composed: true }));
    expect(popover()?.hidden).toBe(true);
    badge?.click();
    popover()?.dispatchEvent(
      new FocusEvent("focusout", { bubbles: true, composed: true, relatedTarget: outside }),
    );
    expect(popover()?.hidden).toBe(true);
  });

  it("keeps the popover open on a click inside it", async () => {
    const indicator = await mount(fakeClient(descriptor()));
    indicator.shadowRoot?.querySelector<HTMLButtonElement>("button.badge")?.click();
    const popover = indicator.shadowRoot?.querySelector<HTMLElement>('[role="dialog"]');
    popover?.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true, composed: true }));
    expect(popover?.hidden).toBe(false);
  });

  it("stops listening for outside clicks once detached", async () => {
    const indicator = await mount(fakeClient(descriptor()));
    const remove = vi.spyOn(document, "removeEventListener");
    indicator.remove();
    expect(remove).toHaveBeenCalledWith("pointerdown", expect.any(Function), true);
  });
});

describe("<fs-host-indicator> host statements", () => {
  it("shows the desktop statement and the not-enrolled state with neither backup nor sync on", async () => {
    const indicator = await mount(fakeClient(descriptor()));
    expect(indicator.dataset.state).toBe("ready");
    expect(indicator.dataset.hostType).toBe("desktop");
    expect(texts(indicator, ".statement")).toEqual([labels.hostDesktop]);
    expect(text(indicator, ".service h3")).toBe(labels.hostServiceTitle);
    expect(text(indicator, ".service .not-enrolled")).toBe(labels.hostNotEnrolled);
    expect(indicator.shadowRoot?.querySelector(".service dl")).toBeNull();
    expect(indicator.shadowRoot?.querySelector('[data-on="true"]')).toBeNull();
  });

  it("names a server's operator and states the at-rest duty", async () => {
    const indicator = await mount(
      fakeClient(descriptor({ host_type: "server", operator_name: "Garderen Family" })),
    );
    expect(indicator.dataset.hostType).toBe("server");
    expect(texts(indicator, ".statement")).toEqual([
      "Your files are stored on a server run by Garderen Family. That operator can read them. Other users see only what they are permitted to.",
      labels.hostServerAtRest,
    ]);
  });

  it("uses the unnamed-operator label for a server without an operator name", async () => {
    const indicator = await mount(fakeClient(descriptor({ host_type: "server" })));
    expect(texts(indicator, ".statement")[0]).toContain(labels.hostOperatorUnnamed);
    expect(texts(indicator, ".statement")[0]).toContain("That operator can read them.");
  });

  it("renders the operator name as text, never as markup", async () => {
    const indicator = await mount(
      fakeClient(
        descriptor({ host_type: "server", operator_name: "<img src=x onerror=alert(1)>" }),
      ),
    );
    expect(indicator.shadowRoot?.querySelector(".statement img")).toBeNull();
    expect(texts(indicator, ".statement")[0]).toContain("<img src=x onerror=alert(1)>");
  });

  it("says the browser host decrypts only in this browser", async () => {
    const indicator = await mount(fakeClient(descriptor({ host_type: "browser" })));
    expect(texts(indicator, ".statement")).toEqual([labels.hostBrowser]);
  });
});

describe("<fs-host-indicator> hosted service", () => {
  it("shows backup and sync as two separate states, with the service host and ciphertext only", async () => {
    const indicator = await mount(fakeClient(descriptor({ hosted_service: service(true, false) })));
    expect(texts(indicator, ".service dt")).toEqual([labels.hostBackup, labels.hostSync]);
    expect(texts(indicator, ".service dd")).toEqual([labels.hostOn, labels.hostOff]);
    expect(text(indicator, ".service .service-host")).toBe(`${labels.hostServiceAt} ${SERVICE}`);
    expect(text(indicator, ".service .ciphertext")).toBe(labels.hostServiceCiphertext);
    expect(texts(indicator, ".statement")).toEqual([labels.hostDesktop, labels.hostDesktopBackup]);
    expect(indicator.shadowRoot?.querySelector(".not-enrolled")).toBeNull();
  });

  it("never presents sync as backup: sync on with backup off labels only sync as on", async () => {
    const indicator = await mount(fakeClient(descriptor({ hosted_service: service(false, true) })));
    const rows = [...(indicator.shadowRoot?.querySelectorAll(".service dd") ?? [])];
    expect(rows.map((row) => row.getAttribute("data-on"))).toEqual(["false", "true"]);
    expect(texts(indicator, ".service dd")).toEqual([labels.hostOff, labels.hostOn]);
  });

  it("shows both on for the browser host, without the desktop-only sentence", async () => {
    const indicator = await mount(
      fakeClient(descriptor({ host_type: "browser", hosted_service: service(true, true) })),
    );
    expect(texts(indicator, ".service dd")).toEqual([labels.hostOn, labels.hostOn]);
    expect(texts(indicator, ".statement")).toEqual([labels.hostBrowser]);
  });
});

describe("<fs-host-indicator> loading and error", () => {
  it("shows loading, and no host or statement, until the descriptor arrives", async () => {
    let release: (value: HostDescriptor) => void = () => undefined;
    const pending = new Promise<HostDescriptor>((resolve) => (release = resolve));
    const client: HostClient = { host: () => pending, subscribe: () => () => undefined };
    const indicator = await mount({ client });
    expect(indicator.dataset.state).toBe("loading");
    expect(indicator.dataset.hostType).toBeUndefined();
    expect(texts(indicator, ".statement")).toEqual([]);
    expect(indicator.shadowRoot?.querySelector(".service")).toBeNull();
    const states = indicator.shadowRoot?.querySelector("fs-states");
    expect(states?.shadowRoot?.textContent).toContain(labels.stateLoading);
    release(descriptor());
    await settle();
    expect(indicator.dataset.state).toBe("ready");
    expect(indicator.shadowRoot?.querySelector("fs-states")).toBeNull();
  });

  it("shows the error state and no host when the descriptor cannot be read", async () => {
    const indicator = await mount(fakeClient(new ApiError(500, "/api/v1/host")));
    expect(indicator.dataset.state).toBe("error");
    expect(indicator.dataset.hostType).toBeUndefined();
    expect(texts(indicator, ".statement")).toEqual([]);
    const states = indicator.shadowRoot?.querySelector("fs-states");
    expect(states?.shadowRoot?.textContent).toContain(labels.stateError);
  });

  it("shows the denied state for an authorisation denial", async () => {
    const denial = new ApiError(401, "/api/v1/host", {
      code: "unauthorized",
      reason: "missing_credentials",
    });
    const indicator = await mount(fakeClient(denial));
    expect(indicator.dataset.state).toBe("denied");
  });

  it("shows the error state for a descriptor that breaks the contract, not a guessed host", async () => {
    const bad = { schema_version: 1, host_type: "cloud" } as unknown as HostDescriptor;
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const indicator = await mount(fakeClient(bad));
    expect(indicator.dataset.state).toBe("error");
    expect(texts(indicator, ".statement")).toEqual([]);
    expect(error).toHaveBeenCalled();
  });

  it("shows the offline state with Retry when the read fails while the browser is offline", async () => {
    vi.spyOn(navigator, "onLine", "get").mockReturnValue(false);
    const fake = fakeClient(new TypeError("Failed to fetch"), descriptor());
    const indicator = await mount(fake);
    expect(indicator.dataset.state).toBe("offline");
    expect(texts(indicator, ".statement")).toEqual([]);
    const states = indicator.shadowRoot?.querySelector("fs-states");
    expect(states?.shadowRoot?.textContent).toContain(labels.stateOffline);
    states?.shadowRoot?.querySelector("button")?.click();
    await settle();
    expect(indicator.dataset.state).toBe("ready");
  });

  it("reads again when Retry is pressed and recovers", async () => {
    const fake = fakeClient(new ApiError(500, "/api/v1/host"), descriptor());
    const indicator = await mount(fake);
    const states = indicator.shadowRoot?.querySelector("fs-states");
    const retry = states?.shadowRoot?.querySelector("button");
    expect(retry).not.toBeNull();
    const reached = vi.fn();
    document.body.addEventListener("fs-retry", reached);
    retry?.click();
    await settle();
    expect(indicator.dataset.state).toBe("ready");
    expect(fake.reads.count).toBe(2);
    expect(reached).not.toHaveBeenCalled();
  });
});

describe("<fs-host-indicator> follows the host", () => {
  it("reads the descriptor once at start and subscribes to events", async () => {
    const fake = fakeClient(descriptor());
    await mount(fake);
    expect(fake.reads.count).toBe(1);
    expect(fake.subscriptions.opened).toBe(1);
  });

  it("updates on host.change without a reload or another read", async () => {
    const fake = fakeClient(descriptor());
    const indicator = await mount(fake);
    expect(texts(indicator, ".service .not-enrolled")).toEqual([labels.hostNotEnrolled]);
    fake.emit({ topic: "host.change", data: descriptor({ hosted_service: service(false, true) }) });
    expect(texts(indicator, ".service dd")).toEqual([labels.hostOff, labels.hostOn]);
    expect(fake.reads.count).toBe(1);
  });

  it("reads again when a host.change payload breaks the contract", async () => {
    const fake = fakeClient(descriptor(), descriptor({ host_type: "browser" }));
    const indicator = await mount(fake);
    fake.emit({ topic: "host.change", data: { nonsense: true } });
    await settle();
    expect(fake.reads.count).toBe(2);
    expect(indicator.dataset.hostType).toBe("browser");
  });

  it("ignores events of other topics", async () => {
    const fake = fakeClient(descriptor());
    await mount(fake);
    fake.emit({ topic: "health.change", data: {} });
    fake.emit({ topic: "process.update", data: {} });
    await settle();
    expect(fake.reads.count).toBe(1);
  });

  it.each([
    [
      "events.resync",
      (fake: ReturnType<typeof fakeClient>) =>
        fake.emit({ topic: "events.resync", data: { missed: 3 } }),
    ],
    ["a reconnect", (fake: ReturnType<typeof fakeClient>) => fake.open()],
  ])("reads again after %s and shows what it read", async (_name, interrupt) => {
    const fake = fakeClient(descriptor(), descriptor({ host_type: "server", operator_name: "Op" }));
    const indicator = await mount(fake);
    interrupt(fake);
    await settle();
    expect(fake.reads.count).toBe(2);
    expect(indicator.dataset.hostType).toBe("server");
  });

  it("drops a slow read that was overtaken by a newer descriptor", async () => {
    let releaseOld: (value: HostDescriptor) => void = () => undefined;
    let calls = 0;
    const client: HostClient = {
      host: () => {
        calls += 1;
        return calls === 1
          ? new Promise<HostDescriptor>((resolve) => (releaseOld = resolve))
          : Promise.resolve(descriptor({ host_type: "browser" }));
      },
      subscribe: () => () => undefined,
    };
    const indicator = await mount({ client });
    await indicator.refresh();
    expect(indicator.dataset.hostType).toBe("browser");
    releaseOld(descriptor({ host_type: "desktop" }));
    await settle();
    expect(indicator.dataset.hostType).toBe("browser");
  });

  it("closes its subscription when removed and opens a fresh one when re-attached", async () => {
    const fake = fakeClient(descriptor());
    const indicator = await mount(fake);
    indicator.remove();
    expect(fake.subscriptions.closed).toBe(1);
    document.body.append(indicator);
    await settle();
    expect(fake.subscriptions.opened).toBe(2);
    indicator.remove();
    expect(fake.subscriptions.closed).toBe(2);
  });

  it("refresh() does nothing once the indicator is detached, so no channel leaks", async () => {
    const fake = fakeClient(descriptor());
    const indicator = await mount(fake);
    indicator.remove();
    await indicator.refresh();
    expect(fake.subscriptions.opened).toBe(1);
    expect(fake.subscriptions.closed).toBe(1);
  });

  it("follows a new label table when configured again while connected", async () => {
    const fake = fakeClient(descriptor());
    const indicator = await mount(fake);
    indicator.configure({ labels: { ...labels, hostIndicator: "Dónde" }, client: fake.client });
    await settle();
    expect(text(indicator, "h2")).toBe("Dónde");
  });

  it("refresh() resubscribes (a new token) and reads again", async () => {
    const fake = fakeClient(descriptor());
    const indicator = await mount(fake);
    await indicator.refresh();
    expect(fake.subscriptions.closed).toBe(1);
    expect(fake.subscriptions.opened).toBe(2);
    expect(fake.reads.count).toBe(2);
  });

  it("logs and still reads when the event channel cannot be opened", async () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const client: HostClient = {
      host: async () => descriptor(),
      subscribe: () => {
        throw new Error("no channel");
      },
    };
    const indicator = await mount({ client });
    expect(indicator.dataset.state).toBe("ready");
    expect(error).toHaveBeenCalled();
  });
});

describe("buildHostStatement (for the Settings page's Privacy section)", () => {
  it("builds the full statement, the host first and the service apart", () => {
    const view = describeHost(descriptor({ hosted_service: service(true, false) }), labels);
    const element = buildHostStatement(view, labels);
    const all = (selector: string) =>
      [...element.querySelectorAll(selector)].map((el) => el.textContent);
    expect(all(".statement")).toEqual([labels.hostDesktop, labels.hostDesktopBackup]);
    expect(all(".service dd")).toEqual([labels.hostOn, labels.hostOff]);
    expect(all(".ciphertext")).toEqual([labels.hostServiceCiphertext]);
  });
});
