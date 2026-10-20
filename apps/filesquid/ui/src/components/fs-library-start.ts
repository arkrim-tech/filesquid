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
 * report waits in the registry for the library page. Nothing is repaired or adopted.
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

/** Custom element that shows the library start page. */
export class FsLibraryStart extends HTMLElement {
  private options?: LibraryPageOptions;
  private feedback?: Feedback;
  private busy = false;
  private last?: Flow;
  private generation = 0;

  constructor() {
    super();
    const root = this.attachShadow({ mode: "open" });
    root.addEventListener(RETRY_EVENT, (event) => {
      event.stopPropagation();
      if (this.last) void this.run(this.last);
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
   * Run one flow: choose a folder, then create, open or recover, then go to the library page.
   * Side effects: network I/O through the source, registry writes and a navigation.
   */
  private async run(flow: Flow): Promise<void> {
    const options = this.options;
    const feedback = this.feedback;
    if (!options || !feedback || this.busy) return;
    const generation = this.generation;
    this.last = flow;
    this.setBusy(true);
    feedback.note(options.labels.libraryChoosing);
    try {
      const choice = await step("choose", () =>
        options.source.chooseLocation(flow === "create" ? "create" : "open"),
      );
      if (generation !== this.generation) return;
      if (choice.outcome !== "chosen" || !choice.location_id) {
        feedback.note(options.labels.libraryCancelled);
        return;
      }
      feedback.note(options.labels.libraryWorking);
      const view = await this.complete(flow, choice.location_id);
      if (generation !== this.generation) return;
      feedback.clear();
      (options.navigate ?? goTo)(libraryHref(view.library_id));
    } catch (error) {
      if (generation === this.generation) this.fail(error);
    } finally {
      if (generation === this.generation) this.setBusy(false);
    }
  }

  /** Create, open or recover on the chosen folder and remember the result. */
  private async complete(flow: Flow, locationId: string): Promise<LibraryView> {
    const { source, registry } = this.options as LibraryPageOptions;
    const action = flow === "create" ? "create" : "open";
    const view = await step(action, () =>
      flow === "create" ? source.create(locationId) : source.open(locationId),
    );
    registry.remember(view);
    if (flow !== "recover") return view;
    const report = await step("rebuild", () => source.rebuild(view.library_id));
    registry.rememberRebuild(view.library_id, report);
    return view;
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
