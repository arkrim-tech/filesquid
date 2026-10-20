/**
 * `<fs-host-indicator>`: the host privacy indicator (UI-S-004, UI-S-018). The header shows a
 * compact badge, an icon and a short host label ("This device" on the desktop host), and the
 * badge opens a popover with the full statement: who can read files on the connected host,
 * then, apart from that, the state of encrypted backup and sync to the hosted service (owner
 * decision 2026-10-19, option 1a; mockup `resources/ui-mockups/v2/40-host-indicator.svg`).
 * Every fact comes from the host descriptor (`GET /api/v1/host`, API-S-020) and every word from
 * the active label table; the UI infers nothing.
 *
 * The descriptor is read at start, replaced by the whole descriptor a `host.change` event
 * carries, and read again after `events.resync` or when the event channel opens again. Until
 * it arrives the badge says the host is being checked and the popover shows the loading state;
 * when it cannot be read (or breaks the contract) the badge says the host is unknown and the
 * popover shows the error, offline, denied or unavailable state with Retry where it helps: it
 * never shows a guessed host, and a failed re-read replaces what was shown.
 *
 * Accessibility (UI-S-016): the badge is a button in a named group (`aria-expanded`,
 * `aria-haspopup="dialog"`, `aria-controls`); click, Enter or Space opens the popover, a
 * non-modal dialog named by its title, and moves focus into it so it is announced; Escape or
 * the badge closes it and returns focus to the badge; a click outside or focus leaving closes
 * it. The short label is a polite live region, so a host change is announced while
 * the popover is closed. The popover ends with a "Privacy settings" link to the Settings page.
 *
 * With `inline: true` the same element is the Settings page's Privacy section: no badge, the
 * full statement shown in place as a named region, following the host the same way.
 */
import { failureState, type ViewState } from "./view-state";
import type { ApiEvent, HostDescriptor } from "../api/client";
import { describeHost, parseHostDescriptor, type HostType } from "../host/descriptor";
import type { LabelKey, LabelTable } from "../theme/labels";
import { A11Y_STYLE } from "./a11y-style";
import { RETRY_EVENT, type FsStates } from "./fs-states";
import { HOST_STATEMENT_STYLE, buildHostStatement } from "./host-statement";
import { focusElement } from "../a11y/focus";

/** The part of the API client the indicator needs (injectable in tests). */
export interface HostClient {
  /** Read the host descriptor. */
  host(): Promise<HostDescriptor>;
  /**
   * Subscribe to push events; `onOpen` is called each time the channel opens, so the
   * indicator can read again after a reconnect.
   */
  subscribe(listener: (event: ApiEvent) => void, onOpen?: () => void): () => void;
}

/** What the indicator needs: the active label table and the client. */
export interface HostIndicatorOptions {
  /** Label table for every word shown. */
  labels: LabelTable;
  /** Client that reads the descriptor and delivers events. */
  client: HostClient;
  /**
   * Show the full statement in place, with no badge or popover: the Settings page's Privacy
   * section. The header indicator leaves it unset.
   */
  inline?: boolean;
  /** Where the popover's "Privacy settings" link goes; defaults to `#/settings`. */
  settingsHref?: string;
}

/** The short badge label of each host type. */
const BADGE_LABEL: Readonly<Record<HostType, LabelKey>> = {
  desktop: "hostBadgeDesktop",
  server: "hostBadgeServer",
  browser: "hostBadgeBrowser",
};

const STYLE = `
  :host { position: relative; display: inline-block; min-width: 0; font-family: var(--fs-font-sans); font-size: var(--fs-font-size-small); }
  .badge { display: inline-flex; align-items: center; gap: var(--fs-space-2); padding: var(--fs-space-1) var(--fs-space-3); border: 1px solid var(--fs-outline); border-radius: var(--fs-radius); background: var(--fs-surface-container); color: var(--fs-on-surface); font: inherit; cursor: pointer; transition: background-color var(--fs-motion-duration); }
  .badge:hover, .badge[aria-expanded="true"] { background: var(--fs-primary-container); color: var(--fs-on-primary-container); }
  .icon { display: inline-block; inline-size: 0.9em; block-size: 0.65em; border: 2px solid currentColor; border-radius: var(--fs-radius); }
  [role="dialog"] { position: absolute; inset-block-start: calc(100% + var(--fs-space-2)); inset-inline-end: 0; z-index: 2; display: grid; gap: var(--fs-space-2); box-sizing: border-box; inline-size: max-content; max-inline-size: min(24rem, calc(100vw - 2 * var(--fs-space-4))); padding: var(--fs-space-3); border: 1px solid var(--fs-outline); border-radius: var(--fs-radius); background: var(--fs-surface); color: var(--fs-on-surface); overflow-wrap: anywhere; }
  [hidden] { display: none !important; }
  .popover.inline { position: static; display: grid; gap: var(--fs-space-2); inline-size: auto; max-inline-size: none; padding: 0; border: 0; background: none; color: var(--fs-on-surface); overflow-wrap: anywhere; }
  .settings-link { color: var(--fs-primary); text-decoration: none; border-radius: var(--fs-radius); }
  .settings-link:hover { text-decoration: underline; }
  h2 { margin: 0; font-size: var(--fs-font-size-small); font-weight: 700; color: var(--fs-on-surface-variant); }
  @media (forced-colors: active) {
    .badge[aria-expanded="true"] { forced-color-adjust: none; background: Highlight; color: HighlightText; }
  }
`;

/** The persistent parts of the indicator. */
interface Parts {
  readonly badge: HTMLButtonElement;
  readonly label: HTMLElement;
  readonly link: HTMLAnchorElement;
  readonly popover: HTMLElement;
  readonly title: HTMLElement;
  readonly body: HTMLElement;
}

/** Custom element that shows the host privacy indicator. */
export class FsHostIndicator extends HTMLElement {
  private options?: HostIndicatorOptions;
  private unsubscribe?: () => void;
  /** Number of the newest read or applied descriptor; older answers are discarded. */
  private request = 0;
  private parts?: Parts;
  private readonly onOutside = (event: Event) => {
    if (!event.composedPath().includes(this)) this.close(false);
  };

  constructor() {
    super();
    this.attachShadow({ mode: "open" });
  }

  /**
   * Set the label table and client, then start. Call before or after connecting.
   *
   * @param options The active label table and the client.
   */
  configure(options: HostIndicatorOptions): void {
    this.options = options;
    if (this.isConnected) void this.start();
  }

  /** Render the frame, listen for outside clicks, subscribe to events and read the descriptor. */
  connectedCallback(): void {
    this.ownerDocument.addEventListener("pointerdown", this.onOutside, true);
    void this.start();
  }

  /** End the event subscription, stop listening and discard any read still in flight. */
  disconnectedCallback(): void {
    this.ownerDocument.removeEventListener("pointerdown", this.onOutside, true);
    this.stop();
  }

  /**
   * Start again from the loading state: close the event subscription, open a new one (it
   * carries the token known now, for example one just entered) and read the descriptor.
   * Network I/O via the client.
   *
   * @returns Resolves once the indicator shows the answer of that read; does nothing when the
   *   indicator is not attached to a document.
   */
  async refresh(): Promise<void> {
    if (!this.isConnected) return;
    await this.start();
  }

  /** Close the subscription and make every outstanding read stale. */
  private stop(): void {
    this.request += 1;
    this.unsubscribe?.();
    this.unsubscribe = undefined;
  }

  /** Render the frame, show loading, subscribe, then read the descriptor. */
  private async start(): Promise<void> {
    const options = this.options;
    if (!options) return;
    this.stop();
    this.frame(options.labels);
    this.showState({ kind: "loading" });
    this.subscribe(options.client);
    await this.load();
  }

  /** Open the event subscription; a refusal is logged and the read below reports it. */
  private subscribe(client: HostClient): void {
    try {
      this.unsubscribe = client.subscribe(
        (event) => this.onEvent(event),
        () => void this.load(),
      );
    } catch (error) {
      console.error("FileSquid host indicator could not open the event channel", error);
    }
  }

  /** React to one event: `host.change` carries the new descriptor, `events.resync` means reread. */
  private onEvent(event: ApiEvent): void {
    if (event.topic === "events.resync") {
      void this.load();
      return;
    }
    if (event.topic !== "host.change") return;
    const descriptor = parseHostDescriptor(event.data);
    if (!descriptor) {
      console.warn("FileSquid host.change payload broke the contract; reading the descriptor");
      void this.load();
      return;
    }
    this.request += 1;
    this.showHost(descriptor);
  }

  /** Read the descriptor and show it, or the state for why it cannot be read. */
  private async load(): Promise<void> {
    const client = this.options?.client;
    if (!client) return;
    const request = ++this.request;
    try {
      const answer = await client.host();
      if (request !== this.request) return;
      const descriptor = parseHostDescriptor(answer);
      if (descriptor) this.showHost(descriptor);
      else this.reject();
    } catch (error) {
      if (request !== this.request) return;
      console.warn("FileSquid host descriptor could not be read", error);
      this.showState(failureState(error, navigator.onLine));
    }
  }

  /** Show the error state for a descriptor that does not satisfy the contract. */
  private reject(): void {
    console.error("FileSquid host descriptor does not satisfy the API contract");
    this.showState({ kind: "error" });
  }

  /** Build the persistent frame once, then word it from the label table. */
  private frame(labels: LabelTable): void {
    const root = this.shadowRoot;
    if (!root) return;
    if (!this.parts) this.parts = buildFrame(root, (event) => this.onFrameEvent(event));
    const group = root.querySelector('[role="group"]');
    group?.setAttribute("aria-label", labels.hostIndicator);
    this.parts.title.textContent = labels.hostIndicator;
    this.parts.link.textContent = labels.hostPrivacySettings;
    this.parts.link.setAttribute("href", this.options?.settingsHref ?? "#/settings");
    if (this.options?.inline) showInline(this.parts);
  }

  /** Clicks, keys, focus moves and Retry inside the frame. */
  private onFrameEvent(event: Event): void {
    const parts = this.parts;
    if (!parts) return;
    if (event.type === RETRY_EVENT) this.onRetry(event);
    else if (event.type === "click" && parts.badge.contains(event.target as Node)) this.toggle();
    else if (event.type === "click" && parts.link.contains(event.target as Node)) this.close(false);
    else if (event.type === "keydown" && (event as KeyboardEvent).key === "Escape") {
      this.close(true);
    } else if (event.type === "focusout") this.onFocusOut(event as FocusEvent);
  }

  /** Close when focus moves outside the indicator, or out of the page (no related target). */
  private onFocusOut(event: FocusEvent): void {
    const next = event.relatedTarget as Node | null;
    if (!next || (!this.contains(next) && !this.shadowRoot?.contains(next))) this.close(false);
  }

  /** Open the popover, or close it when it is open. */
  private toggle(): void {
    const parts = this.parts;
    if (!parts) return;
    if (!parts.popover.hidden) {
      this.close(true);
      return;
    }
    parts.popover.hidden = false;
    parts.badge.setAttribute("aria-expanded", "true");
    focusElement(parts.popover);
  }

  /** Close the popover; return focus to the badge when asked. Side effect: may move focus. */
  private close(refocus: boolean): void {
    const parts = this.parts;
    if (!parts || parts.popover.hidden || this.options?.inline) return;
    parts.popover.hidden = true;
    parts.badge.setAttribute("aria-expanded", "false");
    if (refocus) focusElement(parts.badge);
  }

  /** Retry from the states element: reread, and keep the event out of other views' Retry. */
  private onRetry(event: Event): void {
    event.stopPropagation();
    this.showState({ kind: "loading" });
    void this.load();
  }

  /** Show the badge label; the live region announces it only when it changed. */
  private setBadge(text: string): void {
    const parts = this.parts;
    if (!parts) return;
    if (parts.label.textContent !== text) parts.label.textContent = text;
  }

  /** Show the loading, error, offline, denied or unavailable state, with no host facts. */
  private showState(state: ViewState): void {
    const labels = this.options?.labels;
    if (!this.parts || !labels) return;
    const states = document.createElement("fs-states") as FsStates;
    states.configure({ labels });
    states.state = state;
    this.parts.body.replaceChildren(states);
    this.setBadge(state.kind === "loading" ? labels.hostBadgeLoading : labels.hostBadgeUnknown);
    this.dataset.state = state.kind;
    delete this.dataset.hostType;
  }

  /** Show a validated descriptor. */
  private showHost(descriptor: HostDescriptor): void {
    const labels = this.options?.labels;
    if (!this.parts || !labels) return;
    const view = describeHost(descriptor, labels);
    this.parts.body.replaceChildren(buildHostStatement(view, labels));
    this.setBadge(labels[BADGE_LABEL[view.hostType]]);
    this.dataset.state = "ready";
    this.dataset.hostType = view.hostType;
  }
}

/** An element with a class and optional attributes. */
function part(tag: string, cls: string, attributes: Record<string, string> = {}): HTMLElement {
  const element = document.createElement(tag);
  if (cls) element.className = cls;
  for (const [name, value] of Object.entries(attributes)) element.setAttribute(name, value);
  return element;
}

/**
 * Build the group, badge, live region and closed popover, and route their events to
 * `onEvent`. Side effect: replaces the shadow root's children.
 */
function buildFrame(root: ShadowRoot, onEvent: (event: Event) => void): Parts {
  const style = document.createElement("style");
  style.textContent = A11Y_STYLE + STYLE + HOST_STATEMENT_STYLE;
  const badge = part("button", "badge", {
    type: "button",
    "aria-expanded": "false",
    "aria-haspopup": "dialog",
    "aria-controls": "popover",
  }) as HTMLButtonElement;
  const label = part("span", "label", { "aria-live": "polite" });
  badge.append(part("span", "icon", { "aria-hidden": "true" }), label);
  const title = part("h2", "", { id: "popover-title" });
  const body = part("div", "body");
  const link = part("a", "settings-link") as HTMLAnchorElement;
  const popover = part("div", "popover", {
    id: "popover",
    role: "dialog",
    "aria-labelledby": "popover-title",
    tabindex: "-1",
  });
  popover.hidden = true;
  popover.append(title, body, link);
  const group = part("div", "indicator", { role: "group" });
  group.append(badge, popover);
  for (const type of ["click", "keydown", "focusout", RETRY_EVENT]) {
    group.addEventListener(type, onEvent);
  }
  root.replaceChildren(style, group);
  return { badge, label, link, popover, title, body };
}

/**
 * Turn the frame into the in-place statement of the Settings page's Privacy section: no badge,
 * no link back to Settings, and the popover shown permanently as a named region.
 */
function showInline(parts: Parts): void {
  parts.badge.hidden = true;
  parts.link.hidden = true;
  parts.popover.hidden = false;
  parts.popover.classList.add("inline");
  parts.popover.setAttribute("role", "region");
  parts.popover.removeAttribute("tabindex");
}
