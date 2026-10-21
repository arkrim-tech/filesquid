/**
 * `<fs-library-start>`: the start page of the library flow (UI-1.1, SRS-F-001, API-S-034;
 * mockups `resources/ui-mockups/v2/01-welcome.svg` to `04-recover-library.svg`). Three
 * choices: create a library, open one, or recover from a copy. Each one asks the host to let
 * the owner choose a folder (the desktop shows its native dialog), receives only an opaque
 * location ID, passes that ID to the library route, and then goes to the library's page
 * `#/library/<id>`. A cancelled choice changes nothing. A 503 from the chooser is the
 * unavailable state with its reason and a Retry; the three choices stay on the page. No path is
 * ever shown. Words come only from the label table, styling only from tokens.
 *
 * Recover opens the chosen copy and then rebuilds its catalog from the OCFL files alone; the
 * report waits in the registry for the library page. Nothing is repaired or adopted. Retry
 * resumes from the step that failed: a folder already chosen is not asked for again, and a
 * library already opened is not opened again.
 */
import { RETRY_EVENT } from "./fs-states";
import { libraryFailure, type LibraryAction } from "../libraries/errors";
import { libraryHref, type LibraryView } from "../libraries/model";
import {
  Feedback,
  StepFailure,
  actionButton,
  browserOnline,
  node,
  pageStyle,
  step,
  type LibraryPageOptions,
} from "./library-parts";

/** The three ways into a library. */
type Flow = "create" | "open" | "recover";

/** The order and wording keys of the flows' cards. */
const FLOWS = [
  ["create", "libraryCreateTitle", "libraryCreateHelp", "libraryCreate"],
  ["open", "libraryOpenTitle", "libraryOpenHelp", "libraryOpen"],
  ["recover", "libraryRecoverTitle", "libraryRecoverHelp", "libraryRecover"],
] as const;

/**
 * How far a flow got: the folder chosen and the library created or opened, so Retry resumes
 * from the step that failed instead of asking for the folder again.
 */
interface Progress {
  readonly flow: Flow;
  locationId?: string;
  library?: LibraryView;
}

/** Custom element that shows the library start page. */
export class FsLibraryStart extends HTMLElement {
  private options?: LibraryPageOptions;
  private feedback?: Feedback;
  private busy = false;
  private progress?: Progress;
  private generation = 0;

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

  /** Build the heading, the intro, the feedback block and the three cards. */
  private render(): void {
    const options = this.options;
    if (!this.shadowRoot || !options) return;
    const { labels } = options;
    this.generation += 1;
    this.busy = false;
    const feedback = new Feedback(labels);
    this.feedback = feedback;
    const cards = node("div", "cards");
    cards.append(...FLOWS.map((flow) => this.card(flow, labels)));
    const page = node("div", "page");
    page.append(
      node("h2", undefined, labels.libraries),
      node("p", "intro", labels.libraryIntro),
      feedback.root,
      cards,
    );
    this.shadowRoot.replaceChildren(pageStyle(), page);
  }

  /** One card: title, help and the button that starts its flow. */
  private card(flow: (typeof FLOWS)[number], labels: LibraryPageOptions["labels"]): HTMLElement {
    const [name, title, help, action] = flow;
    const section = node("section");
    section.dataset.flow = name;
    const button = actionButton(name, labels[action]);
    button.addEventListener("click", () => void this.run(name));
    section.append(node("h3", undefined, labels[title]), node("p", "help", labels[help]), button);
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
      if (progress.flow === "recover") await this.rebuild(progress.library);
      if (generation !== this.generation) return;
      feedback.clear();
      (options.navigate ?? goTo)(libraryHref(progress.library.library_id));
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

  /** Rebuild a recovered copy's catalog from its files and keep the report. Network I/O. */
  private async rebuild(view: LibraryView): Promise<void> {
    const { source, registry } = this.options as LibraryPageOptions;
    const report = await step("rebuild", () => source.rebuild(view.library_id));
    registry.rememberRebuild(view.library_id, report);
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

/** Go to a hash address in the current window. Side effect: changes the location. */
function goTo(hash: string): void {
  window.location.hash = hash;
}
