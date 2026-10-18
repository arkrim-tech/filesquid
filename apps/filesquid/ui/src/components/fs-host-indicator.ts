/**
 * `<fs-host-indicator>`: the persistent host privacy indicator (UI-S-004, UI-S-018). It says
 * which host the UI is connected to and who can read files there, then, apart from that, the
 * state of encrypted backup and sync to the hosted service. Every fact comes from the host
 * descriptor (`GET /api/v1/host`, API-S-020) and every word from the active label table; the
 * UI infers nothing.
 *
 * The descriptor is read at start, replaced by the whole descriptor a `host.change` event
 * carries, and read again after `events.resync` or when the event channel opens again. Until
 * it arrives the indicator shows the loading state, and when it cannot be read (or breaks the
 * contract) the error, offline or denied state: it never shows a guessed host, and a failed
 * re-read replaces what was shown, so a stale host is not left on screen.
 *
 * Accessibility (UI-S-016): the indicator is one named, focusable group, so it is reachable
 * by keyboard (visible focus ring from the shared stylesheet) and by screen reader, and its
 * content is a polite live region so a change is announced.
 */
import { failureState, type ViewState } from "./view-state";
import type { ApiEvent, HostDescriptor } from "../api/client";
import {
  describeHost,
  parseHostDescriptor,
  type HostView,
  type ServiceView,
} from "../host/descriptor";
import type { LabelTable } from "../theme/labels";
import { A11Y_STYLE } from "./a11y-style";
import { RETRY_EVENT, type FsStates } from "./fs-states";

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
}

const STYLE = `
  :host { display: block; min-width: 0; font-family: var(--fs-font-sans); }
  section { display: grid; gap: var(--fs-space-2); box-sizing: border-box; padding: var(--fs-space-2) var(--fs-space-3); border: 1px solid var(--fs-outline); border-radius: var(--fs-radius); background: var(--fs-surface-container); color: var(--fs-on-surface); font-size: var(--fs-font-size-small); overflow-wrap: anywhere; }
  h2, h3 { margin: 0; font-size: var(--fs-font-size-small); font-weight: 700; color: var(--fs-on-surface-variant); }
  p { margin: 0; }
  .body { display: grid; gap: var(--fs-space-2); }
  .service { display: grid; gap: var(--fs-space-1); padding-block-start: var(--fs-space-2); border-block-start: 1px solid var(--fs-outline); }
  dl { display: flex; flex-wrap: wrap; gap: var(--fs-space-1) var(--fs-space-4); margin: 0; }
  dl div { display: flex; gap: var(--fs-space-2); }
  dt, dd { margin: 0; }
  dd[data-on="true"] { font-weight: 700; }
  dd[data-on="false"] { color: var(--fs-on-surface-variant); }
`;

/** A text element with an optional class. */
function textElement(tag: string, text: string, cls?: string): HTMLElement {
  const element = document.createElement(tag);
  element.textContent = text;
  if (cls) element.className = cls;
  return element;
}

/** One backup or sync row: a term and its On or Off value. */
function stateRow(term: string, on: boolean, labels: LabelTable): HTMLElement {
  const row = document.createElement("div");
  const value = textElement("dd", on ? labels.hostOn : labels.hostOff);
  value.dataset.on = String(on);
  row.append(textElement("dt", term), value);
  return row;
}

/** The hosted service area: the not-enrolled state, or backup and sync apart with the host. */
function serviceArea(service: ServiceView, labels: LabelTable): HTMLElement {
  const area = document.createElement("div");
  area.className = "service";
  area.append(textElement("h3", labels.hostServiceTitle));
  if (!service.enrolled) {
    area.append(textElement("p", service.text, "not-enrolled"));
    return area;
  }
  const states = document.createElement("dl");
  states.append(
    stateRow(labels.hostBackup, service.backup, labels),
    stateRow(labels.hostSync, service.sync, labels),
  );
  const at = `${labels.hostServiceAt} ${service.serviceHost}`;
  area.append(
    states,
    textElement("p", at, "service-host"),
    textElement("p", service.text, "ciphertext"),
  );
  return area;
}

/** The nodes that show a described host: its statements, then the service apart. */
function hostContent(view: HostView, labels: LabelTable): Node[] {
  const statements = view.statements.map((text) => textElement("p", text, "statement"));
  return [...statements, serviceArea(view.service, labels)];
}

/** Custom element that shows the host privacy indicator. */
export class FsHostIndicator extends HTMLElement {
  private options?: HostIndicatorOptions;
  private unsubscribe?: () => void;
  /** Number of the newest read or applied descriptor; older answers are discarded. */
  private request = 0;

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

  /** Render the frame, subscribe to events and read the descriptor. */
  connectedCallback(): void {
    void this.start();
  }

  /** End the event subscription and discard any read still in flight. */
  disconnectedCallback(): void {
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

  /** Build the persistent frame once: the named group, its title and its body. */
  private frame(labels: LabelTable): void {
    const root = this.shadowRoot;
    if (!root) return;
    const existing = root.querySelector("h2");
    if (existing) {
      existing.textContent = labels.hostIndicator;
      return;
    }
    const style = document.createElement("style");
    style.textContent = A11Y_STYLE + STYLE;
    const section = document.createElement("section");
    section.tabIndex = 0;
    section.setAttribute("role", "group");
    const title = textElement("h2", labels.hostIndicator);
    title.id = "title";
    section.setAttribute("aria-labelledby", title.id);
    const body = document.createElement("div");
    body.className = "body";
    body.setAttribute("aria-live", "polite");
    section.append(title, body);
    section.addEventListener(RETRY_EVENT, (event) => this.onRetry(event));
    root.replaceChildren(style, section);
  }

  /** Retry from the states element: reread, and keep the event out of other views' Retry. */
  private onRetry(event: Event): void {
    event.stopPropagation();
    this.showState({ kind: "loading" });
    void this.load();
  }

  /** Show the loading, error, offline or denied state, with no host facts. */
  private showState(state: ViewState): void {
    const body = this.shadowRoot?.querySelector(".body");
    const labels = this.options?.labels;
    if (!body || !labels) return;
    const states = document.createElement("fs-states") as FsStates;
    states.configure({ labels });
    states.state = state;
    body.replaceChildren(states);
    this.dataset.state = state.kind;
    delete this.dataset.hostType;
  }

  /** Show a validated descriptor. */
  private showHost(descriptor: HostDescriptor): void {
    const body = this.shadowRoot?.querySelector(".body");
    const labels = this.options?.labels;
    if (!body || !labels) return;
    const view = describeHost(descriptor, labels);
    body.replaceChildren(...hostContent(view, labels));
    this.dataset.state = "ready";
    this.dataset.hostType = view.hostType;
  }
}
