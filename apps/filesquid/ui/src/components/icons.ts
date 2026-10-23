/**
 * Line icons of the active mockup suite (UI-S-024): 1.6 px stroke, round caps and joins, no
 * fill, drawn on a 24-unit grid and sized by the component (16, 20 or 24 px). The colour is
 * `currentColor`, so a component sets it through `--fs-icon`, `--fs-icon-muted`,
 * `--fs-icon-disabled` or `--fs-primary`. Every icon is decorative (`aria-hidden`); the text
 * beside it carries the meaning.
 */

/** The names of the icons. */
export type IconName =
  | "plus"
  | "filter"
  | "files"
  | "clock"
  | "pin"
  | "library"
  | "collection"
  | "layers"
  | "folder"
  | "trash"
  | "settings"
  | "logs"
  | "search"
  | "device"
  | "monitor"
  | "chevron"
  | "more"
  | "alert"
  | "error"
  | "offline"
  | "denied"
  | "info"
  | "arrow-up"
  | "arrow-down";

/** The path data of each icon on the 24-unit grid. */
const PATHS: Readonly<Record<IconName, string>> = {
  plus: "M12 5v14M5 12h14",
  filter: "M4 6h16M7 12h10M10 18h4",
  files: "M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8zM14 3v5h5",
  clock: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 7v5l3 2",
  pin: "M12 17v5M8 3h8l-1 7 3 3v2H6v-2l3-3z",
  library: "M4 5h4v15H4zM10 4h4v16h-4zM16 6l4-1 3 14-4 1z",
  collection: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
  layers: "M12 3l9 5-9 5-9-5zM3 13l9 5 9-5M3 17l9 5 9-5",
  folder: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2zM3 11h18",
  trash: "M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13M10 11v6M14 11v6",
  settings:
    "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z",
  logs: "M4 6h16M4 12h16M4 18h10",
  search: "M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16zM21 21l-4.3-4.3",
  device: "M4 5h16a1 1 0 0 1 1 1v10H3V6a1 1 0 0 1 1-1zM2 19h20",
  monitor: "M3 12h4l3-8 4 16 3-8h4",
  chevron: "M9 6l6 6-6 6",
  more: "M6 12h.01M12 12h.01M18 12h.01",
  alert:
    "M12 9v4M12 17h.01M10.3 3.9L2.5 17.4A2 2 0 0 0 4.2 20.4h15.6a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z",
  error: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 8v4M12 16h.01",
  offline: "M2 2l20 20M8.5 16.4a5 5 0 0 1 7 0M5 12.9a10 10 0 0 1 5.2-2.8M12 20h.01",
  denied: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM5.7 5.7l12.6 12.6",
  info: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 16v-4M12 8h.01",
  "arrow-up": "M12 19V5M6 11l6-6 6 6",
  "arrow-down": "M12 5v14M6 13l6 6 6-6",
};

/** Default size of an icon in CSS pixels. */
const DEFAULT_SIZE = 16;

/**
 * Build one decorative line icon.
 *
 * @param name The icon.
 * @param size Its width and height in CSS pixels (16, 20 or 24).
 * @returns An inline `<svg>` with `aria-hidden="true"` and the class `icon icon-<name>`.
 */
export function icon(name: IconName, size = DEFAULT_SIZE): SVGSVGElement {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", String(size));
  svg.setAttribute("height", String(size));
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "1.6");
  svg.setAttribute("stroke-linecap", "round");
  svg.setAttribute("stroke-linejoin", "round");
  svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("focusable", "false");
  svg.setAttribute("class", `icon icon-${name}`);
  svg.dataset.icon = name;
  const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
  path.setAttribute("d", PATHS[name]);
  // The stroke stays 1.6 CSS px at 16, 20 and 24 px, not 1.6 viewBox units.
  path.setAttribute("vector-effect", "non-scaling-stroke");
  svg.append(path);
  return svg;
}
