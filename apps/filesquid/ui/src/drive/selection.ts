/**
 * The selection state of a listing (UI-F2.8, UI-S-006): which rows are ticked, the header
 * checkbox state, and range selection. It is independent of any element, so the Drive view, the
 * search results of UI-F2.9 and the destination picker of UI-F2.17 share one model. It extends
 * `EventTarget` (ADR 031 Decision 6) and dispatches `change` after each change that changes
 * something. Bounded by {@link MAX_SELECTED}: the largest view holds 10,000 files and 1,000
 * folder rows (API-S-011).
 */

/** Most rows one selection holds. */
export const MAX_SELECTED = 11_000;

/** The header checkbox state: nothing, some rows or every row ticked. */
export type SelectionState = "none" | "some" | "all";

/** A set of row keys with the rules of a listing's checkboxes. */
export class Selection extends EventTarget {
  private readonly chosen = new Set<string>();

  /** How many rows are selected. */
  get size(): number {
    return this.chosen.size;
  }

  /** The selected keys, in the order they were selected. */
  keys(): string[] {
    return [...this.chosen];
  }

  /**
   * Whether a row is selected.
   *
   * @param key The row's key.
   * @returns True when selected.
   */
  has(key: string): boolean {
    return this.chosen.has(key);
  }

  /**
   * Tick or untick one row. A row beyond {@link MAX_SELECTED} is not added.
   *
   * @param key The row's key.
   * @param on True to select, false to unselect; the opposite of now when omitted.
   */
  toggle(key: string, on: boolean = !this.chosen.has(key)): void {
    if (on === this.chosen.has(key)) return;
    if (on && this.chosen.size >= MAX_SELECTED) return;
    if (on) this.chosen.add(key);
    else this.chosen.delete(key);
    this.announce();
  }

  /**
   * Select every row of `keys` (the header checkbox), up to {@link MAX_SELECTED}.
   *
   * @param keys The keys of every row in the view.
   */
  selectAll(keys: readonly string[]): void {
    const before = this.chosen.size;
    for (const key of keys) {
      if (this.chosen.size >= MAX_SELECTED) break;
      this.chosen.add(key);
    }
    if (this.chosen.size !== before) this.announce();
  }

  /**
   * Select the rows between two keys, both included, in `order` (a Shift-click).
   *
   * @param order The keys of the rows in display order.
   * @param from The row last ticked.
   * @param to The row clicked.
   */
  selectRange(order: readonly string[], from: string, to: string): void {
    const a = order.indexOf(from);
    const b = order.indexOf(to);
    if (a < 0 || b < 0) return this.toggle(to, true);
    this.selectAll(order.slice(Math.min(a, b), Math.max(a, b) + 1));
  }

  /** Unselect everything. */
  clear(): void {
    if (this.chosen.size === 0) return;
    this.chosen.clear();
    this.announce();
  }

  /**
   * Drop every selected key that is not in `present` (after a reload removed rows).
   *
   * @param present The keys of the rows now in the view.
   */
  retain(present: ReadonlySet<string>): void {
    let changed = false;
    for (const key of [...this.chosen]) {
      if (present.has(key)) continue;
      this.chosen.delete(key);
      changed = true;
    }
    if (changed) this.announce();
  }

  /**
   * The header checkbox state for a view of `total` rows.
   *
   * @param total How many rows the view shows.
   * @returns None, some (the partial state) or all.
   */
  state(total: number): SelectionState {
    if (this.chosen.size === 0) return "none";
    return total > 0 && this.chosen.size >= total ? "all" : "some";
  }

  /** Dispatch `change`. */
  private announce(): void {
    this.dispatchEvent(new Event("change"));
  }
}
