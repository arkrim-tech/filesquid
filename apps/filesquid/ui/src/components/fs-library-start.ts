/**
 * `<fs-library-start>`: the start page of the library flow (UI-1.1, SRS-F-001, API-S-034;
 * UI-S-027, mockups `resources/ui-mockups/v3/01-welcome.svg` to `04-recover-library.svg`),
 * shown without the shell's frame: the 56 px title `libraryWelcomeTitle`, the 17 px lead, the
 * LIBRARIES eyebrow, three cards (Create primary, Open and Recover secondary), the raised privacy
 * box with the host statement read from the host descriptor when a host client is given, and the
 * footer line `appFooter` when a version is given. Three choices: create a library, open one, or
 * recover from a copy. Each one asks the host to let
 * the owner choose a folder (the desktop shows its native dialog), receives only an opaque
 * location ID, passes that ID to the library route, and then goes to the library's page
 * `#/library/<id>`. A cancelled choice changes nothing. A 503 from the chooser is the
 * unavailable state with its reason and a Retry; the three choices stay on the page. No path is
 * ever shown. Words come only from the label table, styling only from tokens.
 *
 * Recover opens the chosen copy and then starts the rebuild of its catalog from the OCFL files
 * alone as a durable job (API-F2.19), then goes to the page that follows the job
 * (`#/library/<id>/recover/<job>`, UI-1.3, mockup 04). Nothing is repaired or adopted. Retry
 * resumes from the step that failed: a folder already chosen is not asked for again, and a
 * library already opened is not opened again.
 *
 * UI-1.3 adds the libraries this host opened (API-S-038, mockup 01) as the RECENT LIBRARIES
 * column, and first use (UI-S-034, mockup 57): when the host knows no library the page opens
 * `<fs-first-use>` over itself, and the Create card opens it again after it was closed. A host
 * without a library registry (503) shows neither the column nor the dialog.
 */
import { RETRY_EVENT } from "./fs-states";
import { formatLabel } from "../theme/labels";
import { FIRST_USE_CANCEL_EVENT, type FsFirstUse } from "./fs-first-use";
import type { FsHostIndicator } from "./fs-host-indicator";
import { icon, type IconName } from "./icons";
import { ApiError } from "../api/client";
import { libraryFailure, type LibraryAction } from "../libraries/errors";
import {
  libraryHref,
  libraryRecoverHref,
  type LibraryView,
  type RecentLibrary,
} from "../libraries/model";
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
import { RECENT_STYLE, recentSection } from "./recent-libraries";

/** The three ways into a library. */
type Flow = "create" | "open" | "recover";

/** The order and wording keys of the flows' cards. */
const FLOWS = [
  ["create", "libraryCreateTitle", "libraryCreateHelp", "libraryCreate"],
  ["open", "libraryOpenTitle", "libraryOpenHelp", "libraryOpen"],
  ["recover", "libraryRecoverTitle", "libraryRecoverHelp", "libraryRecover"],
] as const;

/** The 20 px icon of each card (mockup 01). */
const FLOW_ICON: Readonly<Record<Flow, IconName>> = {
  create: "plus",
  open: "folder",
  recover: "layers",
};

/**
 * How far a flow got: the folder chosen and the library created or opened, so Retry resumes
 * from the step that failed instead of asking for the folder again.
 */
interface Progress {
  readonly flow: Flow;
  locationId?: string;
  library?: LibraryView;
  /** The recovered copy's rebuild job, once started. */
  jobId?: string;
}

/** HTTP status of a host that keeps no library registry. */
const HTTP_UNAVAILABLE = 503;

/** Custom element that shows the library start page. */
export class FsLibraryStart extends HTMLElement {
  private options?: LibraryPageOptions;
  private feedback?: Feedback;
  private busy = false;
  private progress?: Progress;
  private generation = 0;
  /** True once the host answered that it knows no library: the first-use dialog applies. */
  private noLibraryKnown = false;

  constructor() {
    super();
    const root = this.attachShadow({ mode: "open" });
    root.addEventListener(RETRY_EVENT, (event) => {
      event.stopPropagation();
      if (this.progress) void this.resume(this.progress);
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

  /** Render when attached. */
  connectedCallback(): void {
    this.render();
  }

  /** Stop a flow still in flight from navigating once the page is gone. */
  disconnectedCallback(): void {
    this.generation += 1;
  }

  /**
   * Build the welcome title, the lead, the feedback block, the LIBRARIES eyebrow, the three
   * cards, the privacy box and the footer line.
   */
  private render(): void {
    const options = this.options;
    if (!this.shadowRoot || !options) return;
    const { labels } = options;
    this.generation += 1;
    this.busy = false;
    this.noLibraryKnown = false;
    const feedback = new Feedback(labels);
    this.feedback = feedback;
    const cards = node("div", "cards");
    cards.append(...FLOWS.map((flow) => this.card(flow, labels)));
    const page = node("div", "page start");
    page.append(
      node("h2", undefined, labels.libraryWelcomeTitle),
      node("p", "intro", labels.libraryIntro),
      feedback.root,
      node("p", "eyebrow", labels.libraries),
      cards,
      ...privacyBox(options),
      ...footerLine(options),
    );
    this.shadowRoot.replaceChildren(pageStyle(RECENT_STYLE), page);
    void this.loadRecent(this.generation);
  }

  /**
   * Read the libraries this host opened and show them, or, when it knows none, open the
   * first-use dialog. A host without a registry (503) shows neither; another failure is logged
   * and the three choices stay usable. Network I/O through the source.
   */
  private async loadRecent(generation: number): Promise<void> {
    const options = this.options as LibraryPageOptions;
    try {
      const { libraries } = await options.source.recent();
      if (generation !== this.generation) return;
      this.noLibraryKnown = libraries.length === 0;
      if (this.noLibraryKnown) this.openFirstUse();
      else this.showRecent(libraries);
    } catch (error) {
      if (generation !== this.generation) return;
      const quiet = error instanceof ApiError && error.status === HTTP_UNAVAILABLE;
      if (!quiet) console.warn("FileSquid could not read the recent libraries", error);
    }
  }

  /** Add the RECENT LIBRARIES column beside the title. */
  private showRecent(libraries: readonly RecentLibrary[]): void {
    const options = this.options as LibraryPageOptions;
    const page = this.shadowRoot?.querySelector(".page");
    if (!page) return;
    const now = (options.now ?? (() => new Date()))();
    const section = recentSection(
      libraries,
      options.labels,
      now,
      (library) => void this.openRecent(library),
    );
    page.classList.add("has-recent");
    page.querySelector(".intro")?.after(section);
  }

  /**
   * Open a recent library: its page when the host has it open now; otherwise the Open flow,
   * because the API reopens a library only from the folder the owner chooses.
   */
  private async openRecent(library: RecentLibrary): Promise<void> {
    if (library.open) {
      ((this.options as LibraryPageOptions).navigate ?? goTo)(libraryHref(library.library_id));
      return;
    }
    await this.run("open");
  }

  /** Open the first-use dialog over the page, unless it is open; the page waits behind it. */
  private openFirstUse(): void {
    const root = this.shadowRoot;
    if (!root || !this.options || root.querySelector("fs-first-use")) return;
    const dialog = document.createElement("fs-first-use") as FsFirstUse;
    dialog.configure(this.options);
    const page = root.querySelector(".page");
    dialog.addEventListener(FIRST_USE_CANCEL_EVENT, () => page?.removeAttribute("inert"));
    page?.setAttribute("inert", "");
    root.append(dialog);
  }

  /** One card: icon, title, help and the button that starts its flow (Create is primary). */
  private card(flow: (typeof FLOWS)[number], labels: LibraryPageOptions["labels"]): HTMLElement {
    const [name, title, help, action] = flow;
    const section = node("section");
    section.dataset.flow = name;
    const button = actionButton(name, labels[action], name === "create" ? "primary" : "secondary");
    button.addEventListener("click", () => void this.press(name));
    const glyph = icon(FLOW_ICON[name], 20);
    section.append(glyph, node("h3", undefined, labels[title]), node("p", "help", labels[help]));
    section.append(button);
    return section;
  }

  /** Mark the flow buttons busy, so only one folder dialog is open at a time. */
  private setBusy(busy: boolean): void {
    this.busy = busy;
    for (const button of this.shadowRoot?.querySelectorAll<HTMLButtonElement>("button") ?? []) {
      if (busy) button.setAttribute("aria-disabled", "true");
      else button.removeAttribute("aria-disabled");
    }
  }

  /**
   * A card was pressed. While the host knows no library, Create opens the first-use dialog,
   * which also names the library and makes its first collection (UI-S-034); any other press
   * starts its flow.
   */
  private async press(flow: Flow): Promise<void> {
    if (flow === "create" && this.noLibraryKnown) {
      this.openFirstUse();
      return;
    }
    await this.run(flow);
  }

  /**
   * Start a flow afresh from one of the three choices: nothing chosen or opened yet.
   * Side effects: those of {@link FsLibraryStart.resume}.
   */
  private run(flow: Flow): Promise<void> {
    return this.resume({ flow });
  }

  /**
   * Run a flow from where `progress` stopped: choose a folder unless one was chosen, create or
   * open the library unless that was done, rebuild a recovered copy, then go to the library
   * page. Retry resumes from the failed step, so a folder already chosen is never asked for
   * again (registering it twice would be refused as in use). Side effects: network I/O through
   * the source, registry writes and a navigation.
   */
  private async resume(progress: Progress): Promise<void> {
    const options = this.options;
    const feedback = this.feedback;
    if (!options || !feedback || this.busy) return;
    const generation = this.generation;
    this.progress = progress;
    this.setBusy(true);
    try {
      progress.locationId ??= await this.choose(progress.flow, feedback, generation);
      if (generation !== this.generation || progress.locationId === undefined) return;
      feedback.note(options.labels.libraryWorking);
      progress.library ??= await this.enter(progress.flow, progress.locationId);
      if (progress.flow === "recover") progress.jobId ??= await this.rebuild(progress.library);
      if (generation !== this.generation) return;
      feedback.clear();
      (options.navigate ?? goTo)(this.destination(progress, progress.library));
    } catch (error) {
      if (generation === this.generation) this.fail(error);
    } finally {
      if (generation === this.generation) this.setBusy(false);
    }
  }

  /**
   * Ask the host to let the owner choose a folder. Network I/O through the source.
   *
   * @returns The chosen folder's location ID, or undefined when the owner cancelled (said in
   *   the feedback) or the page moved on.
   */
  private async choose(
    flow: Flow,
    feedback: Feedback,
    generation: number,
  ): Promise<string | undefined> {
    const { labels, source } = this.options as LibraryPageOptions;
    feedback.note(labels.libraryChoosing);
    const purpose = flow === "create" ? "create" : "open";
    const choice = await step("choose", () => source.chooseLocation(purpose));
    if (generation !== this.generation) return undefined;
    if (choice.outcome === "chosen" && choice.location_id) return choice.location_id;
    feedback.note(labels.libraryCancelled);
    return undefined;
  }

  /** Create or open the library on the chosen folder and remember it. Network I/O. */
  private async enter(flow: Flow, locationId: string): Promise<LibraryView> {
    const { source, registry } = this.options as LibraryPageOptions;
    const action = flow === "create" ? "create" : "open";
    const view = await step(action, () =>
      flow === "create" ? source.create(locationId) : source.open(locationId),
    );
    registry.remember(view);
    return view;
  }

  /**
   * Start the rebuild of a recovered copy's catalog from its files as a durable job, expecting
   * the revision the open answered (API-F2.19). Network I/O.
   *
   * @returns The job's ID, which the recover page follows (UI-1.3, mockup 04).
   */
  private async rebuild(view: LibraryView): Promise<string> {
    const { source } = this.options as LibraryPageOptions;
    const answer = await step("rebuild", () => source.rebuild(view.library_id, view.revision));
    return answer.job_id;
  }

  /** Where a finished flow goes: the recover page of its job, else the library's page. */
  private destination(progress: Progress, library: LibraryView): string {
    if (progress.jobId !== undefined) return libraryRecoverHref(library.library_id, progress.jobId);
    return libraryHref(library.library_id);
  }

  /** Show a failed step, and log it to the console. */
  private fail(error: unknown): void {
    const options = this.options as LibraryPageOptions;
    const tagged = error instanceof StepFailure;
    const action: LibraryAction = tagged ? error.action : "open";
    const reason = tagged ? error.reason : error;
    console.warn(`FileSquid library step ${action} did not finish`, reason);
    const online = (options.online ?? browserOnline)();
    this.feedback?.fail(libraryFailure(reason, action, options.labels, online));
  }
}

/** The raised privacy box with the inline host statement, when a host client is given. */
function privacyBox(options: LibraryPageOptions): HTMLElement[] {
  if (!options.host) return [];
  const box = node("div", "privacy");
  const statement = document.createElement("fs-host-indicator") as FsHostIndicator;
  statement.configure({ labels: options.labels, client: options.host, inline: true });
  box.append(statement);
  return [box];
}

/** The footer line "Open source under the MIT licence · Version {version} · macOS". */
function footerLine(options: LibraryPageOptions): HTMLElement[] {
  if (!options.version) return [];
  const text = formatLabel(options.labels.appFooter, { version: options.version });
  return [node("p", "app-footer", text)];
}
