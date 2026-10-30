/**
 * The popup menu of the Drive-style views (UI-F2.8, UI-S-026, mockups 06 and 10): the New menu,
 * the item menu (the actions button and the right click) and the grid's sort menu. One small
 * builder, so every menu has the same keys and the same look: `role="menu"` with
 * `menuitem`, `menuitemradio` or `menuitemcheckbox` items, ArrowUp, ArrowDown, Home and End
 * move between enabled items, Enter and Space choose, Escape and Tab close, and a press outside
 * closes. Closing returns focus to the control that opened the menu (UI-S-016). A disabled item
 * stays focusable, carries `aria-disabled` and says why in its `title`.
 *
 * Words come from the caller (the label table); styling is {@link STYLE}, tokens only.
 */
import { icon } from "./icons";

/** One entry of a menu. */
export type MenuEntry =
  | { readonly kind: "eyebrow"; readonly text: string }
  | { readonly kind: "divider" }
  | {
      readonly kind: "item" | "radio";
      readonly id: string;
      readonly label: string;
      /** A shortcut shown at the right end, for example `⌘O`. */
      readonly shortcut?: string;
      /** Whether a radio item is the chosen one (drawn with a check). */
      readonly checked?: boolean;
      /** Drawn in the error colour (Remove). */
      readonly destructive?: boolean;
      /** Not available now: focusable, not chosen. */
      readonly disabled?: boolean;
      /** Why it is not available, shown as the item's tooltip. */
      readonly reason?: string;
    };

/** What a menu needs. */
export interface MenuOptions {
  /** Class of the menu element: `new-menu`, `item-menu` or `sort-menu`. */
  readonly className: string;
  /** The accessible name of the menu. */
  readonly label: string;
  /** The entries, in order. */
  readonly entries: readonly MenuEntry[];
  /** Called with the id of the chosen item; the menu has closed. */
  readonly onChoose: (id: string) => void;
}

/** An open menu. */
export interface OpenMenu {
  /** The menu element. */
  readonly element: HTMLElement;
  /** Close the menu; `restoreFocus` returns focus to the opener. Idempotent. */
  close(restoreFocus: boolean): void;
}

/** Where a menu opens: below an element, or at a pointer position. */
export type MenuAnchor =
  { readonly element: HTMLElement } | { readonly x: number; readonly y: number };

/** The stylesheet of a menu. */
export const STYLE = `
  .menu { position: fixed; z-index: 4; box-sizing: border-box; display: flex; flex-direction: column; min-inline-size: 14.5rem; max-inline-size: calc(100vw - 2 * var(--fs-space-2)); max-block-size: calc(100vh - 2 * var(--fs-space-2)); overflow: auto; padding: var(--fs-space-1); border: 1px solid var(--fs-outline-strong); border-radius: var(--fs-radius); background: var(--fs-surface-overlay); color: var(--fs-on-surface); font-family: var(--fs-font-sans); font-size: var(--fs-font-size-small); }
  .menu.new-menu { inline-size: 14.5rem; }
  .menu.item-menu { inline-size: 15rem; }
  .menu.sort-menu { inline-size: 15rem; }
  .menu .eyebrow { padding: var(--fs-space-2) var(--fs-space-2) var(--fs-space-1); }
  .menu hr { inline-size: 100%; margin: var(--fs-space-1) 0; border: 0; border-block-start: 1px solid var(--fs-outline); }
  .menu-item { display: flex; align-items: center; gap: var(--fs-space-2); box-sizing: border-box; inline-size: 100%; min-block-size: 2.125rem; padding: 0 var(--fs-space-2); border: 0; border-radius: var(--fs-radius-item); background: none; color: inherit; font: inherit; text-align: start; cursor: pointer; }
  .menu-item:hover, .menu-item:focus-visible { background: var(--fs-surface-raised); }
  .menu-item .label { flex: 1 1 auto; min-width: 0; }
  .menu-item .shortcut { color: var(--fs-on-surface-muted); }
  .menu-item .tick { inline-size: 1rem; flex: 0 0 auto; color: var(--fs-primary); }
  .menu-item.remove, .menu-item.remove .icon { color: var(--fs-error); }
  .menu-item[aria-disabled="true"] { color: var(--fs-on-surface-placeholder); cursor: default; }
  .menu-item[aria-disabled="true"]:hover { background: none; }
`;

/** The role of an item entry. */
function roleOf(kind: "item" | "radio"): string {
  return kind === "radio" ? "menuitemradio" : "menuitem";
}

/** Build the element of one item entry. */
function itemElement(entry: Extract<MenuEntry, { id: string }>): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "menu-item";
  button.dataset.id = entry.id;
  button.setAttribute("role", roleOf(entry.kind));
  button.tabIndex = -1;
  if (entry.destructive) button.classList.add("remove");
  if (entry.kind === "radio") {
    button.setAttribute("aria-checked", String(entry.checked === true));
    const tick = document.createElement("span");
    tick.className = "tick";
    if (entry.checked) tick.append(icon("check"));
    button.append(tick);
  }
  const label = document.createElement("span");
  label.className = "label";
  label.textContent = entry.label;
  button.append(label);
  if (entry.shortcut) {
    const shortcut = document.createElement("span");
    shortcut.className = "shortcut";
    shortcut.textContent = entry.shortcut;
    button.append(shortcut);
  }
  if (entry.disabled) {
    button.setAttribute("aria-disabled", "true");
    if (entry.reason) button.title = entry.reason;
  }
  return button;
}

/** Build the element of one entry. */
function entryElement(entry: MenuEntry): HTMLElement {
  if (entry.kind === "divider") {
    const rule = document.createElement("hr");
    rule.setAttribute("role", "separator");
    return rule;
  }
  if (entry.kind === "eyebrow") {
    const eyebrow = document.createElement("p");
    eyebrow.className = "eyebrow";
    eyebrow.textContent = entry.text;
    return eyebrow;
  }
  return itemElement(entry);
}

/** The items of a menu element, in order. */
function itemsOf(menu: HTMLElement): HTMLElement[] {
  return [...menu.querySelectorAll<HTMLElement>(".menu-item")];
}

/** Move focus by `delta` among the items, wrapping; `to` jumps to the first or last. */
function moveFocus(
  menu: HTMLElement,
  current: Element | null,
  delta: number,
  to?: "first" | "last",
) {
  const items = itemsOf(menu);
  if (items.length === 0) return;
  const here = current ? items.indexOf(current as HTMLElement) : -1;
  let next = (here + delta + items.length) % items.length;
  if (to === "first") next = 0;
  if (to === "last") next = items.length - 1;
  items[next]?.focus();
}

/** Position a menu below its anchor, or at the pointer, inside the viewport. */
function place(menu: HTMLElement, anchor: MenuAnchor): void {
  const gap = 4;
  const rect = "element" in anchor ? anchor.element.getBoundingClientRect() : undefined;
  const wantX = rect ? rect.left : (anchor as { x: number }).x;
  const wantY = rect ? rect.bottom + gap : (anchor as { y: number }).y;
  const box = menu.getBoundingClientRect();
  const width = box.width || 240;
  const height = box.height || 200;
  const left = Math.max(gap, Math.min(wantX, window.innerWidth - width - gap));
  const top = Math.max(gap, Math.min(wantY, window.innerHeight - height - gap));
  menu.style.left = `${left}px`;
  menu.style.top = `${top}px`;
}

/**
 * Open a menu in `layer`. Side effects: adds the menu and a document listener, and moves focus
 * to the first item (or the chosen radio item).
 *
 * @param layer The element the menu is appended to (inside the view's shadow root).
 * @param anchor Where it opens.
 * @param opener The control that opened it, which gets focus back on close.
 * @param options Entries, class, name and the choose callback.
 * @returns The open menu.
 */
export function openMenu(
  layer: HTMLElement,
  anchor: MenuAnchor,
  opener: HTMLElement | null,
  options: MenuOptions,
): OpenMenu {
  const menu = document.createElement("div");
  menu.className = `menu ${options.className}`;
  menu.setAttribute("role", "menu");
  menu.setAttribute("aria-label", options.label);
  menu.append(...options.entries.map(entryElement));
  layer.append(menu);
  place(menu, anchor);
  let closed = false;
  const outside = (event: Event) => {
    const path = event.composedPath();
    // A press on the opener is left to its own click, which toggles the menu.
    if (!path.includes(menu) && !(opener && path.includes(opener))) close(false);
  };
  const close = (restoreFocus: boolean) => {
    if (closed) return;
    closed = true;
    document.removeEventListener("pointerdown", outside, true);
    menu.remove();
    if (restoreFocus) opener?.focus();
  };
  menu.addEventListener("click", (event) => {
    const item = (event.target as Element).closest<HTMLElement>(".menu-item");
    if (!item || item.getAttribute("aria-disabled") === "true") return;
    close(true);
    options.onChoose(item.dataset.id ?? "");
  });
  menu.addEventListener("keydown", (event) => onKey(event, menu, close));
  document.addEventListener("pointerdown", outside, true);
  const items = itemsOf(menu);
  (items.find((el) => el.getAttribute("aria-checked") === "true") ?? items[0])?.focus();
  return { element: menu, close };
}

/** The keys of an open menu. */
function onKey(event: KeyboardEvent, menu: HTMLElement, close: (restore: boolean) => void): void {
  const current = (menu.getRootNode() as ShadowRoot).activeElement;
  const handled = ["ArrowDown", "ArrowUp", "Home", "End", "Escape", "Tab"];
  if (!handled.includes(event.key)) return;
  if (event.key === "Tab") return close(false);
  event.preventDefault();
  event.stopPropagation();
  if (event.key === "Escape") return close(true);
  if (event.key === "Home") return moveFocus(menu, current, 0, "first");
  if (event.key === "End") return moveFocus(menu, current, 0, "last");
  moveFocus(menu, current, event.key === "ArrowDown" ? 1 : -1);
}
