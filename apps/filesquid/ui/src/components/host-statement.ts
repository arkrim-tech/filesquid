/**
 * The full host privacy statement as DOM (UI-S-004, UI-S-018, ADR 032 Decision 6): the
 * statements about the connected host first, then the hosted backup and sync service apart
 * from it, with backup and sync each On or Off and never one presented as the other. The
 * indicator's popover shows it, and the Settings page's Privacy section mounts the same
 * builder, so both say the same thing from one source. Builds elements from its arguments
 * only; every word comes from the label table and the operator name is inserted as text.
 */
import type { HostView, ServiceView } from "../host/descriptor";
import type { LabelTable } from "../theme/labels";

/** Styles for the statement, written with design tokens only. */
export const HOST_STATEMENT_STYLE = `
  .statement-body { display: grid; gap: var(--fs-space-2); }
  .statement-body p { margin: 0; }
  .service { display: grid; gap: var(--fs-space-1); padding-block-start: var(--fs-space-2); border-block-start: 1px solid var(--fs-outline); }
  .service h3 { margin: 0; font-size: var(--fs-font-size-small); font-weight: 700; color: var(--fs-on-surface-variant); }
  .service dl { display: flex; flex-wrap: wrap; gap: var(--fs-space-1) var(--fs-space-4); margin: 0; }
  .service dl div { display: flex; gap: var(--fs-space-2); }
  .service dt, .service dd { margin: 0; }
  .service dd[data-on="true"] { font-weight: 700; }
  .service dd[data-on="false"] { color: var(--fs-on-surface-variant); }
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

/**
 * Build the full statement for a described host. Style it with {@link HOST_STATEMENT_STYLE}.
 *
 * @param view What `describeHost` returned for a validated descriptor.
 * @param labels The active label table.
 * @returns A `div.statement-body` holding `p.statement` lines, then `div.service`.
 */
export function buildHostStatement(view: HostView, labels: LabelTable): HTMLElement {
  const body = document.createElement("div");
  body.className = "statement-body";
  const statements = view.statements.map((text) => textElement("p", text, "statement"));
  body.append(...statements, serviceArea(view.service, labels));
  return body;
}
