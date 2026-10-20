/**
 * `<fs-library-repair>`: the version-inventory repair offer of one library,
 * `#/library/<id>/repair` (UI-1.1, API-S-023, API-S-025, OCFL-S-020; owner decision
 * 2026-10-18). It lists the objects whose files can be read but not changed because the head
 * has no version inventory, and offers the repair only through a consent dialog that names
 * those objects and says that one copy of the root inventory and its sidecar will be added to
 * each object's latest version directory and that nothing else changes. Only Confirm calls the
 * repair route, with the owner's consent for exactly the named objects; Cancel and Escape send
 * nothing and change nothing. The result is shown per object, and a refused repair says nothing
 * was changed. The Monitor's W010 note links here through `bindMonitorRoute`'s `repairHref`.
 */
import { RETRY_EVENT } from "./fs-states";
import { focusElement } from "../a11y/focus";
import { libraryFailure, type LibraryAction } from "../libraries/errors";
import {
  MAX_REPAIR_OBJECTS,
  consentRequest,
  libraryHref,
  type ObjectRepair,
  type RepairAnswer,
  type RepairCandidate,
} from "../libraries/model";
import { formatLabel, type LabelTable } from "../theme/labels";
import {
  Feedback,
  StepFailure,
  actionButton,
  browserOnline,
  link,
  node,
  pageStyle,
  step,
  type LibraryPageOptions,
} from "./library-parts";

/** What the repair page needs: no registry and no navigation. */
export type RepairPageOptions = Pick<LibraryPageOptions, "labels" | "source" | "online">;

/** One object's line in the offer. */
function candidateText(candidate: RepairCandidate, labels: LabelTable): string {
  const head = formatLabel(labels.repairObject, {
    object: candidate.object_id,
    head: candidate.head,
    algorithm: candidate.digest_algorithm,
  });
  const notes = [
    candidate.interrupted ? labels.repairInterrupted : "",
    candidate.committable_after_repair ? "" : labels.repairStaysReadOnly,
  ];
  return [head, ...notes].filter((part) => part !== "").join(" ");
}

/** One object's line in the result. */
function outcomeText(outcome: ObjectRepair, labels: LabelTable): string {
  const object = outcome.object_id;
  if (outcome.result === "refused") {
    return formatLabel(labels.repairRefused, { object, code: outcome.error_code ?? "unknown" });
  }
  const version = outcome.version ?? "";
  if (outcome.provenance_error_code) {
    const code = outcome.provenance_error_code;
    return formatLabel(labels.repairRepairedNoProvenance, { object, version, code });
  }
  return formatLabel(labels.repairRepaired, { object, version });
}

/** The result section: one line per object. */
function resultsSection(answer: RepairAnswer, labels: LabelTable): HTMLElement {
  const section = node("section");
  section.dataset.section = "results";
  section.setAttribute("role", "status");
  const list = node("ul");
  list.append(...answer.objects.map((item) => node("li", undefined, outcomeText(item, labels))));
  const title = node("h3", undefined, labels.repairResultsTitle);
  section.append(title, list);
  return section;
}

/** Custom element that shows one library's repair offer. */
export class FsLibraryRepair extends HTMLElement {
  private options?: RepairPageOptions;
  private library?: string;
  private generation = 0;
  private busy = false;
  private feedback?: Feedback;
  private listBox?: HTMLElement;
  private resultsBox?: HTMLElement;
  private candidates: readonly RepairCandidate[] = [];

  constructor() {
    super();
    const root = this.attachShadow({ mode: "open" });
    root.addEventListener(RETRY_EVENT, (event) => {
      event.stopPropagation();
      void this.load();
    });
  }

  /**
   * Set the labels and the library source, then render. Call before or after connecting.
   *
   * @param options Labels, source and the optional online check.
   */
  configure(options: RepairPageOptions): void {
    this.options = options;
    if (this.isConnected) this.render();
  }

  /** The ID of the library whose repair is offered. */
  get libraryId(): string | undefined {
    return this.library;
  }

  /** Offer the repair of another library; its candidates are read again. */
  set libraryId(id: string | undefined) {
    this.library = id;
    if (this.isConnected) this.render();
  }

  /** Render when attached. */
  connectedCallback(): void {
    this.render();
  }

  /** Stop answers still in flight from touching the page once it is detached. */
  disconnectedCallback(): void {
    this.generation += 1;
  }

  /** Build the page and read the candidates. */
  private render(): void {
    const options = this.options;
    const id = this.library;
    if (!this.shadowRoot || !options || id === undefined) return;
    this.generation += 1;
    const { labels } = options;
    this.feedback = new Feedback(labels);
    this.listBox = node("div", "offer");
    this.resultsBox = node("div", "results");
    this.candidates = [];
    const page = node("div", "page");
    page.append(
      link("back", labels.libraryBack, libraryHref(id)),
      node("h2", undefined, labels.repairTitle),
      node("p", "intro", labels.repairIntro),
      this.feedback.root,
      this.listBox,
      this.resultsBox,
    );
    this.shadowRoot.replaceChildren(pageStyle(), page);
    void this.load();
  }

  /** Read the candidates and show the offer. Network I/O through the source. */
  private async load(): Promise<void> {
    const options = this.options;
    const feedback = this.feedback;
    const id = this.library;
    if (!options || !feedback || id === undefined) return;
    const generation = this.generation;
    feedback.loading();
    try {
      const found = await step("candidates", () => options.source.repairCandidates(id));
      if (generation !== this.generation) return;
      this.candidates = found.candidates;
      this.showOffer(found.truncated);
    } catch (error) {
      if (generation !== this.generation) return;
      this.candidates = [];
      this.listBox?.replaceChildren();
      this.fail(error);
    }
  }

  /** Show the list of objects and the button that opens the consent dialog. */
  private showOffer(truncated: boolean): void {
    const { labels } = this.options as RepairPageOptions;
    const feedback = this.feedback as Feedback;
    this.listBox?.replaceChildren();
    feedback.clear();
    if (this.candidates.length === 0) {
      feedback.empty(labels.repairNothing);
      return;
    }
    const list = node("ul");
    list.dataset.list = "objects";
    list.append(
      ...this.candidates.map((item) => node("li", undefined, candidateText(item, labels))),
    );
    const section = node("section");
    section.append(node("h3", undefined, labels.repairObjectsTitle));
    if (truncated) {
      const limit = MAX_REPAIR_OBJECTS.toLocaleString();
      section.append(node("p", "truncated", formatLabel(labels.repairTruncated, { limit })));
    }
    section.append(list);
    if (consentRequest(this.library as string, this.candidates))
      section.append(this.reviewButton());
    else feedback.fail({ kind: "message", text: labels.repairRequestRefused });
    this.listBox?.append(section);
  }

  /** The button that opens the consent dialog. */
  private reviewButton(): HTMLButtonElement {
    const { labels } = this.options as RepairPageOptions;
    const button = actionButton("review", labels.repairReview);
    button.addEventListener("click", () => this.openDialog(button));
    return button;
  }

  /** Open the consent dialog over the page and move focus into it, to Cancel. */
  private openDialog(opener: HTMLButtonElement): void {
    const { labels } = this.options as RepairPageOptions;
    if (this.busy || this.shadowRoot?.querySelector('[role="dialog"]')) return;
    const count = String(this.candidates.length);
    const title = node("h3", undefined, formatLabel(labels.repairDialogTitle, { count }));
    title.id = "repair-title";
    const names = node("ul");
    names.append(...this.candidates.map((item) => node("li", undefined, item.object_id)));
    const cancel = actionButton("cancel", labels.repairCancel);
    const confirm = actionButton("confirm", formatLabel(labels.repairConfirm, { count }));
    confirm.classList.add("primary");
    const buttons = node("div", "actions");
    buttons.append(cancel, confirm);
    const dialog = node("div");
    dialog.setAttribute("role", "dialog");
    dialog.setAttribute("aria-modal", "true");
    dialog.setAttribute("aria-labelledby", "repair-title");
    dialog.append(title, node("p", "consent", labels.repairConsent), names, buttons);
    const scrim = node("div", "scrim");
    scrim.append(dialog);
    cancel.addEventListener("click", () => this.closeDialog(opener));
    confirm.addEventListener("click", () => void this.confirm());
    dialog.addEventListener("keydown", (event) => this.keepInside(event, dialog, opener));
    this.shadowRoot?.append(scrim);
    focusElement(cancel);
  }

  /** Escape cancels; Tab and Shift+Tab stay on the dialog's two buttons. */
  private keepInside(event: KeyboardEvent, dialog: HTMLElement, opener: HTMLButtonElement): void {
    if (event.key === "Escape") {
      event.preventDefault();
      this.closeDialog(opener);
      return;
    }
    if (event.key !== "Tab") return;
    const buttons = [...dialog.querySelectorAll<HTMLButtonElement>("button")];
    const first = buttons[0];
    const last = buttons[buttons.length - 1];
    const active = this.shadowRoot?.activeElement;
    if (event.shiftKey && active === first && last) {
      event.preventDefault();
      focusElement(last);
    } else if (!event.shiftKey && active === last && first) {
      event.preventDefault();
      focusElement(first);
    }
  }

  /** Close the dialog without sending anything and return focus to the opener. */
  private closeDialog(opener?: HTMLElement): void {
    this.shadowRoot?.querySelector(".scrim")?.remove();
    if (opener?.isConnected) focusElement(opener);
  }

  /**
   * The owner confirmed: close the dialog and send the consent for exactly the named objects.
   * Network I/O through the source that writes into each object's latest version directory.
   */
  private async confirm(): Promise<void> {
    const options = this.options as RepairPageOptions;
    const feedback = this.feedback as Feedback;
    const request = consentRequest(this.library as string, this.candidates);
    this.closeDialog();
    if (!request || this.busy) return;
    this.busy = true;
    const generation = this.generation;
    feedback.note(options.labels.libraryWorking);
    try {
      const answer = await step("repair", () => options.source.repair(request));
      if (generation !== this.generation) return;
      feedback.clear();
      this.resultsBox?.replaceChildren(resultsSection(answer, options.labels));
      await this.load();
      this.focusResults();
    } catch (error) {
      if (generation === this.generation) this.fail(error);
    } finally {
      this.busy = false;
    }
  }

  /** Move focus to the result so a keyboard or screen-reader user lands on it. */
  private focusResults(): void {
    const results = this.resultsBox?.querySelector<HTMLElement>("section");
    if (results) focusElement(results);
  }

  /** Show a failed step, and log it to the console. */
  private fail(error: unknown): void {
    const options = this.options as RepairPageOptions;
    const tagged = error instanceof StepFailure;
    const action: LibraryAction = tagged ? error.action : "repair";
    const reason = tagged ? error.reason : error;
    console.warn(`FileSquid library step ${action} did not finish`, reason);
    const online = (options.online ?? browserOnline)();
    this.feedback?.fail(libraryFailure(reason, action, options.labels, online));
  }
}
