/**
 * `<fs-first-use>`: the dialog "Create your first library" over the welcome screen (UI-1.3,
 * UI-S-034, mockup `resources/ui-mockups/v3/57-first-use-library.svg`). With no library known to
 * the host it asks for the library's name and folder, creates the library and then its first
 * collection ("untitled collection", ADR 041), each step a journaled API command. The folder is
 * chosen in the host's own dialog through the location-choices route (purpose `create`) and
 * reaches this element only as an opaque location ID: no path, folder name or volume is shown,
 * stored or logged (API-S-034). The dialog is 600 x 470 px on the overlay surface with one
 * primary button, Create library.
 *
 * A failure after the library exists keeps the library: the error is shown with Try again for
 * the collection step alone, so the folder is never asked for again and the library is never
 * created twice. A failure before it leaves the folder chosen, so Create runs again without
 * asking. Cancel closes the dialog (and, once the library exists, goes to its page).
 *
 * The dialog announces `fs-first-use-done` (bubbles, composed; detail `{ libraryId,
 * collectionId }`) when both steps are done, before it goes to the library's page: that event
 * is the hook the import dialog of UI-1.2 listens to. This element builds no import entry point.
 * It announces `fs-first-use-cancel` when the owner closes it.
 */
import { focusElement } from "../a11y/focus";
import { libraryFailure, type Failure, type LibraryAction } from "../libraries/errors";
import { libraryHref, type CollectionView, type LibraryView } from "../libraries/model";
import { RETRY_EVENT } from "./fs-states";
import { icon } from "./icons";
import {
  Feedback,
  StepFailure,
  actionButton,
  browserOnline,
  goTo,
  node,
  pageStyle,
  step,
  type LibraryPageOptions,
} from "./library-parts";

/** The event announced when the first library and its first collection exist. */
export const FIRST_USE_DONE_EVENT = "fs-first-use-done";
/** The event announced when the owner closes the dialog. */
export const FIRST_USE_CANCEL_EVENT = "fs-first-use-cancel";

/** What the done event carries. */
export interface FirstUseDone {
  /** The new library's ID. */
  readonly libraryId: string;
  /** Its first collection's ID. */
  readonly collectionId: string;
}

/** How far the dialog got, so Try again resumes from the step that failed. */
interface Progress {
  locationId?: string;
  library?: LibraryView;
  collection?: CollectionView;
}

/** The state that offers Try again for the collection step when the answer was a sentence. */
const RETRY_FAILURE: Failure = { kind: "state", state: { kind: "error" } };

/** Longest name the field accepts, in characters (the API allows 1,024 bytes). */
const MAX_NAME_CHARS = 256;

/**
 * The dialog's rules (UI-S-034, UI-S-024): 600 x 470 px, radius 10 px, 28 px side padding, the
 * name input 544 x 36 px, the location box 400 x 36 px beside Choose folder (136 x 36 px), the
 * note box 544 x 64 px on the raised surface with radius 8 px, and 38 px footer buttons. It sits
 * 162 px below the top of the window, as drawn.
 */
const FIRST_USE_STYLE = `
  :host { display: contents; }
  .scrim { align-items: start; justify-items: center; padding-block-start: max(var(--fs-space-3), min(10.125rem, calc(100vh - 29.375rem - var(--fs-space-3)))); overflow: auto; }
  .first-use { display: flex; flex-direction: column; gap: 0; inline-size: min(37.5rem, 100%); min-block-size: 29.375rem; padding: var(--fs-space-4) calc(var(--fs-space-4) + var(--fs-space-1) - 1px); }
  .first-use .dialog-title { font-size: var(--fs-font-size-heading); line-height: 1.2; }
  .lead { display: grid; gap: 0; margin-block-start: var(--fs-space-2); color: var(--fs-on-surface); font-size: var(--fs-font-size-ui); line-height: 1.5714; }
  .field { display: grid; gap: var(--fs-space-1); margin-block-start: var(--fs-space-4); }
  .field + .field { margin-block-start: calc(var(--fs-space-3) + var(--fs-space-1)); }
  .field-label { margin-block-end: var(--fs-space-1); color: var(--fs-on-surface); font-size: var(--fs-font-size-small); font-weight: 600; line-height: 1.3; }
  .first-use .input { inline-size: 100%; }
  .location { display: grid; grid-template-columns: minmax(0, 25rem) 8.5rem; gap: var(--fs-space-2); }
  .location-box { display: flex; align-items: center; gap: var(--fs-space-2); box-sizing: border-box; min-block-size: 2.25rem; padding: 0 0.75rem; border: 1.5px solid var(--fs-outline-control); border-radius: var(--fs-radius-input); background: var(--fs-surface-card); color: var(--fs-on-surface); font-size: var(--fs-font-size-ui); }
  .location-value { min-inline-size: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .location-box[data-chosen="false"] .location-value { color: var(--fs-on-surface-placeholder); }
  .location .button { inline-size: 100%; padding-inline: 0; }
  .rule { margin-block-start: 0.3125rem; color: var(--fs-on-surface-variant); font-size: var(--fs-font-size-small); line-height: 1.5385; }
  .collection-note { grid-template-columns: auto minmax(0, 1fr); align-items: start; gap: 0 var(--fs-space-2); margin-block-start: 0.625rem; padding: 0.75rem 0.875rem; font-size: var(--fs-font-size-small); line-height: 1.5385; }
  .collection-note .icon { margin-block-start: 0.125rem; }
  .collection-note div { display: grid; }
  .collection-note strong { font-weight: 600; color: var(--fs-on-surface); }
  .collection-note span { color: var(--fs-on-surface-variant); }
  .kept { margin-block-start: var(--fs-space-2); color: var(--fs-on-surface); font-size: var(--fs-font-size-small); font-weight: 600; }
  .kept:empty { display: none; }
  .first-use .feedback { margin-block-start: var(--fs-space-2); }
  .first-use .dialog-actions { margin-block-start: auto; padding-block-start: var(--fs-space-4); border-block-start: none; }
  .first-use .dialog-actions .button[data-action="cancel"] { min-inline-size: 5.125rem; }
  .first-use .dialog-actions .button[data-action="create"] { min-inline-size: 8.125rem; }
`;

/** Custom element that shows the first-use dialog. */
export class FsFirstUse extends HTMLElement {
  private options?: LibraryPageOptions;
  private progress: Progress = {};
  private busy = false;
  private generation = 0;
  private feedback?: Feedback;
  private nameField?: HTMLInputElement;
  private chosenBox?: HTMLElement;
  private keptLine?: HTMLElement;

  constructor() {
    super();
    const root = this.attachShadow({ mode: "open" });
    root.addEventListener(RETRY_EVENT, (event) => {
      event.stopPropagation();
      void this.retry();
    });
  }

  /**
   * Set the labels, the library source and the registry, then render. Call before or after
   * connecting.
   *
   * @param options Labels, source, registry, and the optional navigation and online check.
   */
  configure(options: LibraryPageOptions): void {
    this.options = options;
    if (this.isConnected) this.render();
  }

  /** Render when attached and focus the name field, as drawn. */
  connectedCallback(): void {
    this.render();
  }

  /** Stop a flow still in flight from touching the dialog once it is gone. */
  disconnectedCallback(): void {
    this.generation += 1;
  }

  /** Build the dialog and focus the name field. */
  private render(): void {
    const options = this.options;
    if (!this.shadowRoot || !options) return;
    this.generation += 1;
    this.busy = false;
    this.progress = {};
    const { labels } = options;
    this.feedback = new Feedback(labels, "first-use");
    this.keptLine = node("p", "kept");
    this.keptLine.setAttribute("role", "status");
    const dialog = node("div", "dialog first-use");
    dialog.setAttribute("role", "dialog");
    dialog.setAttribute("aria-modal", "true");
    dialog.setAttribute("aria-labelledby", "first-use-title");
    const title = node("h3", "dialog-title", labels.libraryFirstUseTitle);
    title.id = "first-use-title";
    dialog.append(
      title,
      this.lead(),
      this.nameSection(),
      this.locationSection(),
      this.collectionNote(),
      this.keptLine,
      this.feedback.root,
      this.footer(),
    );
    dialog.addEventListener("keydown", (event) => this.keepInside(event, dialog));
    const scrim = node("div", "scrim");
    scrim.append(dialog);
    this.shadowRoot.replaceChildren(pageStyle(FIRST_USE_STYLE), scrim);
    this.syncCreate();
    focusElement(this.nameField as HTMLElement);
  }

  /** The two lead sentences. */
  private lead(): HTMLElement {
    const { labels } = this.options as LibraryPageOptions;
    const lead = node("div", "lead");
    lead.append(
      node("p", undefined, labels.libraryFirstUseLead),
      node("p", undefined, labels.libraryFirstUseCollectionLead),
    );
    return lead;
  }

  /** The Name label and its input. */
  private nameSection(): HTMLElement {
    const { labels } = this.options as LibraryPageOptions;
    const label = node("label", "field-label", labels.libraryFirstUseName);
    label.htmlFor = "first-use-name";
    const input = node("input", "input");
    input.id = "first-use-name";
    input.type = "text";
    input.maxLength = MAX_NAME_CHARS;
    input.autocomplete = "off";
    input.spellcheck = false;
    this.nameField = input;
    const field = node("div", "field");
    field.append(label, input);
    return field;
  }

  /** The Location label, the chosen-folder box, Choose folder and the rule below them. */
  private locationSection(): HTMLElement {
    const { labels } = this.options as LibraryPageOptions;
    const label = node("span", "field-label location-label", labels.libraryFirstUseLocation);
    label.id = "first-use-location";
    const box = node("div", "location-box");
    box.setAttribute("role", "group");
    box.setAttribute("aria-labelledby", "first-use-location");
    this.chosenBox = box;
    const choose = actionButton("choose", labels.libraryChooseFolder);
    choose.addEventListener("click", () => void this.choose());
    const row = node("div", "location");
    row.append(box, choose);
    const field = node("div", "field");
    field.append(label, row, node("p", "rule", labels.libraryFolderRule));
    this.showLocation();
    return field;
  }

  /** The note that the first collection is created with the library. */
  private collectionNote(): HTMLElement {
    const { labels } = this.options as LibraryPageOptions;
    const text = node("div");
    text.append(
      node("strong", undefined, labels.libraryFirstCollection),
      node("span", undefined, labels.libraryFirstCollectionNote),
    );
    const box = node("div", "note-box raised collection-note");
    box.append(icon("info", 16), text);
    return box;
  }

  /** Cancel (secondary) and Create library (the one primary button), both 38 px. */
  private footer(): HTMLElement {
    const { labels } = this.options as LibraryPageOptions;
    const cancel = actionButton("cancel", labels.libraryFirstUseCancel);
    const create = actionButton("create", labels.libraryCreate, "primary");
    cancel.classList.add("large");
    create.classList.add("large");
    cancel.addEventListener("click", () => this.cancel());
    create.addEventListener("click", () => void this.create());
    const actions = node("div", "dialog-actions");
    actions.append(cancel, create);
    return actions;
  }

  /** Show whether a folder is chosen: its state only, never a path or a folder name. */
  private showLocation(): void {
    const { labels } = this.options as LibraryPageOptions;
    const chosen = this.progress.locationId !== undefined;
    const text = chosen ? labels.libraryFolderChosen : labels.libraryFolderNone;
    this.chosenBox?.replaceChildren(
      icon(chosen ? "check" : "folder", 16),
      node("span", "location-value", text),
    );
    if (this.chosenBox) this.chosenBox.dataset.chosen = String(chosen);
  }

  /** Create is available once a folder is chosen and nothing is running. */
  private syncCreate(): void {
    const create = this.button("create");
    const ready = this.progress.locationId !== undefined && !this.busy;
    if (ready) create?.removeAttribute("aria-disabled");
    else create?.setAttribute("aria-disabled", "true");
  }

  /** One of the dialog's buttons. */
  private button(action: string): HTMLButtonElement | null {
    return this.shadowRoot?.querySelector(`button[data-action="${action}"]`) ?? null;
  }

  /** Mark the dialog busy, so only one step runs at a time. */
  private setBusy(busy: boolean): void {
    this.busy = busy;
    for (const control of this.shadowRoot?.querySelectorAll<HTMLElement>("button, input") ?? []) {
      if (busy) control.setAttribute("aria-disabled", "true");
      else if (!this.locked(control)) control.removeAttribute("aria-disabled");
    }
    this.syncCreate();
  }

  /** True for a control that stays unavailable once the library exists. */
  private locked(control: HTMLElement): boolean {
    if (this.progress.library === undefined) return false;
    return control.tagName === "INPUT" || control.dataset.action === "choose";
  }

  /** Keep the library as it is: the name and the folder cannot change any more. */
  private lockFields(): void {
    this.nameField?.setAttribute("aria-disabled", "true");
    this.nameField?.setAttribute("readonly", "");
    this.button("choose")?.setAttribute("aria-disabled", "true");
  }

  /** Escape closes the dialog; Tab and Shift+Tab stay on its controls. */
  private keepInside(event: KeyboardEvent, dialog: HTMLElement): void {
    if (event.key === "Escape") {
      event.preventDefault();
      this.cancel();
      return;
    }
    if (event.key !== "Tab") return;
    const controls = [...dialog.querySelectorAll<HTMLElement>("input, button")];
    const first = controls[0];
    const last = controls[controls.length - 1];
    const active = this.shadowRoot?.activeElement;
    if (event.shiftKey && active === first && last) {
      event.preventDefault();
      focusElement(last);
    } else if (!event.shiftKey && active === last && first) {
      event.preventDefault();
      focusElement(first);
    }
  }

  /**
   * Ask the host to let the owner choose the empty folder (purpose `create`). Network I/O
   * through the source; the answer is an opaque location ID.
   */
  private async choose(): Promise<void> {
    const options = this.options;
    const feedback = this.feedback;
    if (!options || !feedback || this.busy || this.progress.library) return;
    const generation = this.generation;
    this.setBusy(true);
    feedback.note(options.labels.libraryChoosing);
    try {
      const choice = await step("choose", () => options.source.chooseLocation("create"));
      if (generation !== this.generation) return;
      if (choice.outcome === "chosen" && choice.location_id) {
        this.progress.locationId = choice.location_id;
        this.showLocation();
        feedback.clear();
      } else {
        feedback.note(options.labels.libraryCancelled);
      }
    } catch (error) {
      if (generation === this.generation) this.fail(error);
    } finally {
      if (generation === this.generation) this.setBusy(false);
    }
  }

  /** Create the library and its first collection from where the last attempt stopped. */
  private async create(): Promise<void> {
    const options = this.options;
    const feedback = this.feedback;
    if (!options || !feedback || this.busy || this.progress.locationId === undefined) return;
    const generation = this.generation;
    this.setBusy(true);
    feedback.note(options.labels.libraryWorking);
    try {
      this.progress.library ??= await this.createLibrary(this.progress.locationId);
      if (generation !== this.generation) return;
      this.lockFields();
      this.progress.collection ??= await this.createCollection(this.progress.library);
      if (generation !== this.generation) return;
      // Creating the collection may have advanced the library's revision.
      options.registry.invalidateRevision(this.progress.library.library_id);
      this.finish(this.progress.library, this.progress.collection);
    } catch (error) {
      if (generation === this.generation) this.fail(error);
    } finally {
      if (generation === this.generation) this.setBusy(false);
    }
  }

  /** Create the library on the chosen folder with the name given, and remember it. Network I/O. */
  private async createLibrary(locationId: string): Promise<LibraryView> {
    const { source, registry } = this.options as LibraryPageOptions;
    const name = this.nameField?.value.trim();
    const view = await step("create", () =>
      source.create(locationId, name === "" ? undefined : name),
    );
    registry.remember(view);
    return view;
  }

  /** Create the library's first collection, expecting the revision the create answered. */
  private async createCollection(library: LibraryView): Promise<CollectionView> {
    const { labels, source } = this.options as LibraryPageOptions;
    const title = labels.libraryFirstCollectionTitle;
    return step("collection", () =>
      source.createCollection(library.library_id, title, library.revision),
    );
  }

  /** Try again: choose the folder if none is chosen, else resume creating. */
  private retry(): Promise<void> {
    return this.progress.locationId === undefined ? this.choose() : this.create();
  }

  /** Both steps are done: announce it for the import dialog, then go to the library's page. */
  private finish(library: LibraryView, collection: CollectionView): void {
    const options = this.options as LibraryPageOptions;
    this.feedback?.clear();
    const detail: FirstUseDone = {
      libraryId: library.library_id,
      collectionId: collection.collection_id,
    };
    this.dispatchEvent(
      new CustomEvent(FIRST_USE_DONE_EVENT, { bubbles: true, composed: true, detail }),
    );
    (options.navigate ?? goTo)(libraryHref(library.library_id));
  }

  /** Close the dialog; once the library exists, go to its page, which is kept. */
  private cancel(): void {
    const options = this.options;
    if (this.busy || !options) return;
    const library = this.progress.library;
    this.dispatchEvent(new CustomEvent(FIRST_USE_CANCEL_EVENT, { bubbles: true, composed: true }));
    this.remove();
    if (library) (options.navigate ?? goTo)(libraryHref(library.library_id));
  }

  /** Show a failed step in the dialog and log it to the console. */
  private fail(error: unknown): void {
    const options = this.options as LibraryPageOptions;
    const tagged = error instanceof StepFailure;
    const action: LibraryAction = tagged ? error.action : "create";
    const reason = tagged ? error.reason : error;
    console.warn(`FileSquid first-use step ${action} did not finish`, reason);
    const online = (options.online ?? browserOnline)();
    const kept = this.progress.library !== undefined && action === "collection";
    if (this.keptLine) {
      this.keptLine.textContent = kept ? options.labels.libraryFirstUseCollectionFailed : "";
    }
    const failure = libraryFailure(reason, action, options.labels, online);
    // The library is kept, so the collection step always offers Try again, whatever the answer.
    const offered: Failure = kept && failure.kind === "message" ? RETRY_FAILURE : failure;
    this.feedback?.fail(offered);
  }
}
