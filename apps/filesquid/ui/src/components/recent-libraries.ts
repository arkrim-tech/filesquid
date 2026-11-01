/**
 * The RECENT LIBRARIES column of the welcome screen (UI-1.3, UI-S-027, API-S-038, mockup
 * `resources/ui-mockups/v3/01-welcome.svg`): the libraries this host opened, newest first, each
 * a row with the library icon, its name (14 px 600), "Opened {when} · {free} free on its disk"
 * (13 px; no free figure when the volume cannot be read) and a ghost Open button (76 x 24 px).
 * Rows are 60 px apart, divided by 1 px `--fs-outline` lines, in a 332 px column that sits at
 * the right of the title. No path, location or library ID is shown. Pure DOM construction.
 */
import { recentMeta } from "../libraries/format";
import type { RecentLibrary } from "../libraries/model";
import type { LabelTable } from "../theme/labels";
import { icon } from "./icons";
import { actionButton, node } from "./library-parts";

/**
 * The column's rules: 332 px wide, rows 60 px apart (11 px above the name, 9 px below the
 * meta), the name 14 px 600, the meta 13 px, a 16 px icon 10 px before the text and a ghost
 * Open button 76 x 24 px at the row's right edge.
 */
export const RECENT_STYLE = `
  .start { grid-template-columns: minmax(0, 1fr) minmax(0, 20.75rem); column-gap: var(--fs-space-4); }
  .start > * { grid-column: 1 / -1; }
  .start.has-recent > h2, .start.has-recent > .intro { grid-column: 1; }
  .start > .recent { grid-column: 2; grid-row: 1 / span 2; min-inline-size: 0; align-self: start; }
  .recent { display: grid; gap: 0; align-content: start; }
  .recent ul { gap: 0; align-content: start; }
  .recent li { display: grid; grid-template-columns: 1rem minmax(0, 1fr) auto; grid-template-areas: "icon name open" "icon meta meta"; column-gap: 0.625rem; align-items: start; padding: 0.6875rem 0 0.5625rem; border-block-end: 1px solid var(--fs-outline); }
  .recent .icon { grid-area: icon; margin-block-start: 0.1875rem; }
  .recent .name { grid-area: name; min-inline-size: 0; font-size: var(--fs-font-size-ui); font-weight: 600; line-height: 1.4286; color: var(--fs-on-surface); overflow-wrap: anywhere; }
  .recent .meta { grid-area: meta; font-size: var(--fs-font-size-small); line-height: 1.5385; color: var(--fs-on-surface-variant); }
  .recent .button { grid-area: open; box-sizing: border-box; inline-size: 4.75rem; min-block-size: 1.5rem; padding: 0; margin-block: -0.0625rem -0.1875rem; }
`;

/**
 * Build the column.
 *
 * @param libraries The libraries the host lists, newest first, at most 20.
 * @param labels The active label table.
 * @param now The current time, for "today".
 * @param onOpen Called with the library whose Open button was pressed.
 * @returns The section, to add to the welcome page.
 */
export function recentSection(
  libraries: readonly RecentLibrary[],
  labels: LabelTable,
  now: Date,
  onOpen: (library: RecentLibrary) => void,
): HTMLElement {
  const eyebrow = node("p", "eyebrow", labels.libraryRecentTitle);
  eyebrow.id = "recent-title";
  const list = node("ul");
  list.append(...libraries.map((library) => recentRow(library, labels, now, onOpen)));
  const section = node("section", "recent");
  section.setAttribute("aria-labelledby", "recent-title");
  section.append(eyebrow, list);
  return section;
}

/** One row: icon, name, meta line and the Open button named after its library. */
function recentRow(
  library: RecentLibrary,
  labels: LabelTable,
  now: Date,
  onOpen: (library: RecentLibrary) => void,
): HTMLElement {
  const open = actionButton("open-recent", labels.libraryOpenRecent);
  open.className = "button ghost";
  open.setAttribute("aria-label", `${labels.libraryOpenRecent} ${library.name}`);
  open.addEventListener("click", () => onOpen(library));
  const row = node("li");
  row.append(
    icon("library", 16),
    node("span", "name", library.name),
    node("span", "meta", recentMeta(library, now, labels)),
    open,
  );
  return row;
}
