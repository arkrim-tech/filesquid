/**
 * The per-user UI preferences (UI-F2.8, UI-S-006, UI-S-017): the contract type, the defaults,
 * and the check every answer passes before the UI uses it. Choices only, never a name, title,
 * logical path, token or content handle. Pure: no I/O, no DOM.
 *
 * Every list is bounded (AGENTS.md section 22) by the limits of the API contract and
 * UI-S-017: 100 pinned destinations, 16 section ids, 1,000 collapsed node handles.
 */
import type { components } from "../api/schema";
import {
  MAX_COLLAPSED,
  MAX_PINNED,
  MAX_SECTIONS,
  NAVIGATION_KINDS,
  isHandle,
  type NavigationPreferences,
} from "../navigation/model";

/** One user's saved choices, generated from the API contract. */
export type UserPreferences = components["schemas"]["Preferences"];
/** What a listing is sorted by. */
export type SortKey = components["schemas"]["SortKey"];
/** The sort direction. */
export type SortDirection = components["schemas"]["SortDirection"];
/** Where folder rows go. */
export type FolderPlacement = components["schemas"]["FolderPlacement"];
/** How listings are laid out. */
export type Layout = components["schemas"]["Layout"];
/** Where the UI opens. */
export type StartView = components["schemas"]["StartView"];
/** Which files a view shows. */
export type ViewKind = components["schemas"]["ViewKind"];

/** Every sort key, in the order the sort menu lists them (UI-S-026). */
export const SORT_KEYS: readonly SortKey[] = [
  "name",
  "size",
  "modified",
  "modified_by_me",
  "opened_by_me",
];
/** Every sort direction. */
export const SORT_DIRECTIONS: readonly SortDirection[] = ["ascending", "descending"];
/** Every folder placement. */
export const FOLDER_PLACEMENTS: readonly FolderPlacement[] = ["on_top", "mixed"];
/** Every layout. */
export const LAYOUTS: readonly Layout[] = ["list", "grid"];
/** Every start view. */
export const START_VIEWS: readonly StartView[] = ["home", "personal_root"];

/** The choices of a user who saved none (the API's defaults). */
export const DEFAULT_PREFERENCES: UserPreferences = Object.freeze({
  schema: 1,
  start_view: "home",
  sort_key: "name",
  sort_direction: "ascending",
  folder_placement: "on_top",
  layout: "list",
  hover_card: true,
  navigation: Object.freeze({ version: 1, pinned: [], order: [], collapsed: [] }),
}) as UserPreferences;

/** True for a plain object. */
function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** True when `value` is one of `allowed`. */
function oneOf<T extends string>(allowed: readonly T[], value: unknown): value is T {
  return typeof value === "string" && (allowed as readonly string[]).includes(value);
}

/** True for a list of at most `max` distinct strings that pass `valid`. */
function boundedStrings(value: unknown, max: number, valid: (text: string) => boolean): boolean {
  if (!Array.isArray(value) || value.length > max) return false;
  const seen = new Set<unknown>();
  for (const item of value) {
    if (typeof item !== "string" || !valid(item) || seen.has(item)) return false;
    seen.add(item);
  }
  return true;
}

/** True for a pinned destination list: handles of known kinds, each node once, at most 100. */
function validPins(value: unknown): boolean {
  if (!Array.isArray(value) || value.length > MAX_PINNED) return false;
  const nodes = new Set<unknown>();
  for (const pin of value) {
    if (!isRecord(pin) || !isHandle(pin.node) || nodes.has(pin.node)) return false;
    if (!oneOf(NAVIGATION_KINDS, pin.kind)) return false;
    nodes.add(pin.node);
  }
  return true;
}

/** Check the navigation member; undefined when it breaks a bound or has another version. */
function parseNavigation(value: unknown): NavigationPreferences | undefined {
  if (!isRecord(value) || value.version !== 1) return undefined;
  const sectionId = (text: string) => text.length >= 1 && text.length <= 64;
  const valid =
    validPins(value.pinned) &&
    boundedStrings(value.order, MAX_SECTIONS, sectionId) &&
    boundedStrings(value.collapsed, MAX_COLLAPSED, isHandle);
  if (!valid) return undefined;
  const pins = value.pinned as { node: string; kind: NavigationPreferences["pinned"][0]["kind"] }[];
  return {
    pinned: pins.map(({ node, kind }) => ({ node, kind })),
    order: [...(value.order as string[])],
    collapsed: [...(value.collapsed as string[])],
  };
}

/**
 * Check an answer of `GET /api/v1/preferences` against the contract and keep only its fields,
 * so a host that breaks the contract is refused instead of guessed at. Pure.
 *
 * @param value The parsed answer.
 * @returns The preferences, or undefined when any member is missing, unknown or out of bound.
 */
export function parsePreferences(value: unknown): UserPreferences | undefined {
  if (!isRecord(value) || value.schema !== 1) return undefined;
  const navigation = parseNavigation(value.navigation);
  const ok =
    oneOf(START_VIEWS, value.start_view) &&
    oneOf(SORT_KEYS, value.sort_key) &&
    oneOf(SORT_DIRECTIONS, value.sort_direction) &&
    oneOf(FOLDER_PLACEMENTS, value.folder_placement) &&
    oneOf(LAYOUTS, value.layout) &&
    typeof value.hover_card === "boolean";
  if (!ok || !navigation) return undefined;
  return {
    schema: 1,
    start_view: value.start_view as StartView,
    sort_key: value.sort_key as SortKey,
    sort_direction: value.sort_direction as SortDirection,
    folder_placement: value.folder_placement as FolderPlacement,
    layout: value.layout as Layout,
    hover_card: value.hover_card as boolean,
    navigation: mutableNavigation(navigation),
  };
}

/**
 * The navigation choices inside preferences. Pure.
 *
 * @param prefs The preferences.
 * @returns Pins, section order and collapsed nodes.
 */
export function navigationOf(prefs: UserPreferences): NavigationPreferences {
  const { pinned, order, collapsed } = prefs.navigation;
  return { pinned, order, collapsed };
}

/**
 * The same preferences with other navigation choices. Pure.
 *
 * @param prefs The preferences.
 * @param navigation The new choices.
 * @returns A new value.
 */
export function withNavigation(
  prefs: UserPreferences,
  navigation: NavigationPreferences,
): UserPreferences {
  return { ...prefs, navigation: mutableNavigation(navigation) };
}

/** A copy of the navigation choices in the mutable shape of the generated contract type. */
function mutableNavigation(navigation: NavigationPreferences): UserPreferences["navigation"] {
  return {
    version: 1,
    pinned: navigation.pinned.map((pin) => ({ node: pin.node, kind: pin.kind })),
    order: [...navigation.order],
    collapsed: [...navigation.collapsed],
  };
}

/**
 * Whether no navigation choice is saved.
 *
 * @param navigation The choices.
 * @returns True for the defaults.
 */
export function isEmptyNavigation(navigation: NavigationPreferences): boolean {
  return navigation.pinned.length + navigation.order.length + navigation.collapsed.length === 0;
}
