/**
 * UI tests for `<fs-import-dialog>` (UI-1.2, UI-S-002 **Destination**, UI-S-028, API-S-036,
 * mockups 52, 18 and 48): the dialog opens on the New menu's upload items, needs a destination
 * before its start control works and sends nothing until then, shows the planning state without
 * a percentage, shows the review before any job starts, starts only on Upload, discards the plan
 * on Cancel, Escape and Change…, and words each refusal as an error card with Dismiss.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  IMPORT_STARTED_EVENT,
  MemoryImportSource,
  MemoryNavigationSource,
  NamedDestinations,
  accessibilityViolations,
  defineElements,
  type FixtureNode,
  type FsImportDialog,
} from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import { COLLECTION_ID, LIBRARY_ID, PLAN, PLAN_ITEMS, refusal } from "../support/import-fakes";

const tree: FixtureNode = {
  node: "r",
  kind: "repository",
  name: null,
  children: [
    {
      node: "l1",
      kind: "library",
      name: "Family archive",
      children: [
        {
          node: "c1",
          kind: "collection",
          name: "Harbour project",
          children: [{ node: "f1", kind: "folder", name: "Site photos" }],
        },
        { node: "v1", kind: "virtual_collection", name: "Best of 2026" },
      ],
    },
  ],
};

beforeEach(() => defineElements());
afterEach(() => {
  document.body.replaceChildren();
  vi.restoreAllMocks();
});

/** Let pending promises and renders finish. */
const settle = async () => {
  for (let i = 0; i < 12; i += 1) await new Promise((resolve) => setTimeout(resolve, 0));
};

/** Mount a dialog over fakes; the directory resolves the nodes the tree holds. */
async function mount(source = new MemoryImportSource({ plan: PLAN, items: PLAN_ITEMS })) {
  const directory = new NamedDestinations(async (node) =>
    node === "c1" || node === "f1"
      ? { libraryId: LIBRARY_ID, collectionId: COLLECTION_ID }
      : undefined,
  );
  const dialog = document.createElement("fs-import-dialog") as FsImportDialog;
  dialog.configure({ labels, source, navigation: new MemoryNavigationSource(tree), directory });
  document.body.append(dialog);
  return { dialog, source, directory };
}

/** The shadow root of the dialog. */
const rootOf = (dialog: FsImportDialog) => dialog.shadowRoot as ShadowRoot;

/** The open dialog element. */
const shown = (dialog: FsImportDialog) =>
  rootOf(dialog).querySelector<HTMLElement>("[role=dialog]");

/** One of the open dialog's controls by `data-action`. */
const control = (dialog: FsImportDialog, action: string) =>
  rootOf(dialog).querySelector<HTMLButtonElement>(
    `button[data-action="${action}"]`,
  ) as HTMLButtonElement;

/** The picker's tree row of a node. */
function row(dialog: FsImportDialog, node: string): HTMLElement {
  const picker = rootOf(dialog).querySelector("fs-destination-picker") as HTMLElement;
  return picker.shadowRoot?.querySelector(`li[data-node="${node}"]`) as HTMLElement;
}

/** Open the dialog and choose the collection, or the folder inside it. */
async function openAndChoose(dialog: FsImportDialog, node = "c1") {
  dialog.open();
  await settle();
  if (node === "f1") {
    row(dialog, "c1").querySelector<HTMLElement>(".chevron")?.click();
    await settle();
  }
  row(dialog, node).click();
  await settle();
}

describe("<fs-import-dialog> destination step (mockup 52)", () => {
  it("is a defined element that opens from the New menu's upload items and says Upload", async () => {
    expect(customElements.get("fs-import-dialog")).toBeDefined();
    const { dialog } = await mount();
    document.dispatchEvent(
      new CustomEvent("fs-new-action", { detail: { action: "upload-folder", destination: "c1" } }),
    );
    await settle();
    const frame = shown(dialog);
    expect(frame?.getAttribute("aria-modal")).toBe("true");
    expect(frame?.querySelector("h2")?.textContent).toBe(labels.importTitle);
    expect(frame?.querySelector(".intro")?.textContent).toBe(labels.importIntro);
    expect(frame?.querySelector("legend")?.textContent).toBe(labels.importSourceLegend);
    const radios = [...(frame?.querySelectorAll<HTMLInputElement>("input[type=radio]") ?? [])];
    expect(radios.map((radio) => [radio.value, radio.checked])).toEqual([
      ["files", false],
      ["folder", true],
    ]);
    expect(frame?.querySelector(".field-head")?.textContent).toBe(
      `${labels.importDestination}${labels.importRequired}`,
    );
    expect(frame?.querySelector<HTMLInputElement>(".destination-field")?.placeholder).toBe(
      labels.importDestinationPlaceholder,
    );
    expect(frame?.querySelector(".help")?.textContent).toBe(labels.importDestinationNote);
    expect(frame?.textContent).not.toMatch(/\bImport\b/);
  });

  it("starts with the destination of the place shown, chosen in the picker", async () => {
    const { dialog } = await mount();
    dialog.open({ destination: "c1" });
    await settle();
    expect(shown(dialog)?.querySelector<HTMLInputElement>(".destination-field")?.value).toBe(
      "Harbour project",
    );
    expect(control(dialog, "start").disabled).toBe(false);
  });

  it("keeps the start control off with its reason until a destination is chosen, and sends nothing", async () => {
    const { dialog, source } = await mount();
    dialog.open();
    await settle();
    const start = control(dialog, "start");
    expect(start.textContent).toBe(labels.importChooseFiles);
    expect(start.disabled).toBe(true);
    expect(shown(dialog)?.querySelector(".foot .reason")?.textContent).toBe(
      labels.importChooseDestinationFirst,
    );
    start.click();
    await settle();
    expect(source.requests).toEqual([]);
  });

  it("offers only collections and folders: a library row says it is not a destination and a virtual collection is absent", async () => {
    const { dialog } = await mount();
    dialog.open();
    await settle();
    const library = row(dialog, "l1");
    expect(library.dataset.selectable).toBe("false");
    expect(library.querySelector(".not-destination")?.textContent).toBe(
      labels.importNotDestination,
    );
    expect(row(dialog, "v1")).toBeNull();
    library.click();
    await settle();
    expect(control(dialog, "start").disabled).toBe(true);
  });

  it("words the start control for the chosen source and enables it once a destination is chosen", async () => {
    const { dialog } = await mount();
    await openAndChoose(dialog, "f1");
    const frame = shown(dialog) as HTMLElement;
    expect(frame.querySelector<HTMLInputElement>(".destination-field")?.value).toBe(
      "Harbour project › Site photos",
    );
    expect(control(dialog, "start").disabled).toBe(false);
    expect(control(dialog, "start").textContent).toBe(labels.importChooseFiles);
    const folder = frame.querySelector<HTMLInputElement>('input[value="folder"]');
    folder?.click();
    expect(control(dialog, "start").textContent).toBe(labels.importChooseFolder);
  });

  it("meets the accessibility contracts", async () => {
    const { dialog } = await mount();
    await openAndChoose(dialog);
    expect(accessibilityViolations(rootOf(dialog))).toEqual([]);
  });
});

describe("<fs-import-dialog> planning and review (mockup 18)", () => {
  it("sends the library, collection, folder and source, shows planning with no percentage, then the review", async () => {
    const source = new MemoryImportSource({ plan: PLAN, items: PLAN_ITEMS });
    let release: () => void = () => undefined;
    source.gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const { dialog } = await mount(source);
    await openAndChoose(dialog, "f1");
    control(dialog, "start").click();
    await settle();
    const planning = rootOf(dialog).querySelector(".planning");
    expect(planning?.textContent).toBe(labels.importPlanning);
    expect(planning?.textContent).not.toMatch(/%/);
    expect(planning?.getAttribute("role")).toBe("status");
    expect(source.requests).toHaveLength(1);
    expect(source.requests[0]).toMatchObject({
      libraryId: LIBRARY_ID,
      collectionId: COLLECTION_ID,
      folder: "Site photos",
      source: "files",
    });
    expect(source.requests[0]?.operationId).toMatch(/^[0-9a-f-]{36}$/);
    expect(source.calls).toEqual([]);
    release();
    await settle();
    expect(shown(dialog)?.classList.contains("review")).toBe(true);
  });

  it("shows the plan: title, volume, four counts, every file with its status, the destination, space and the integrity note, and no source path", async () => {
    const { dialog } = await mount();
    await openAndChoose(dialog, "f1");
    control(dialog, "start").click();
    await settle();
    const frame = shown(dialog) as HTMLElement;
    expect(frame.querySelector("h2")?.textContent).toBe("Upload 214 files");
    expect(frame.querySelector(".volume")?.textContent).toBe("From the volume EOS_DIGITAL");
    const cards = [...frame.querySelectorAll(".card")].map((card) => card.textContent);
    expect(cards).toEqual([
      "214files found",
      "196new",
      "18already in this library",
      "3.1 GBto copy",
    ]);
    const rows = [...frame.querySelectorAll(".files li")];
    expect(rows).toHaveLength(6);
    expect(rows[0]?.querySelector(".name")?.textContent).toBe("IMG_4102.CR3");
    expect(rows[0]?.querySelector(".status")?.textContent).toBe(labels.importItemNew);
    expect(rows[5]?.querySelector(".status")?.textContent).toBe(labels.importItemSkipped);
    expect(rows[5]?.getAttribute("data-present")).toBe("true");
    expect(frame.querySelector<HTMLInputElement>(".destination-field")?.value).toBe(
      "Harbour project › Site photos",
    );
    expect(frame.querySelector(".note")?.textContent).toBe(
      "Names taken in Site photos get the first free number, for example IMG_4102-1.JPG.",
    );
    expect(frame.querySelector(".free")?.textContent).toBe(
      "After upload: 409 GB free on this computer",
    );
    expect(frame.querySelector(".staging")?.textContent).toBe("Staging uses at most 10 GB.");
    expect(frame.querySelector(".integrity .title")?.textContent).toBe(labels.importIntegrityTitle);
    expect(frame.querySelector(".foot [data-action=start]")?.textContent).toBe("Upload 196 files");
    expect(frame.querySelector(".files-note")?.textContent).toBe(
      "Showing the first 6 of 214 files.",
    );
    expect(frame.textContent).not.toMatch(/\/Users|[A-Z]:\\|file:\/\//);
  });

  it("starts nothing before Upload: the review calls only the plan routes, and Upload starts the one plan", async () => {
    const { dialog, source } = await mount();
    const started = vi.fn();
    dialog.addEventListener(IMPORT_STARTED_EVENT, (event) =>
      started((event as CustomEvent).detail),
    );
    await openAndChoose(dialog);
    control(dialog, "start").click();
    await settle();
    expect(source.calls).toEqual(["items plan-1"]);
    expect(started).not.toHaveBeenCalled();
    control(dialog, "start").click();
    await settle();
    expect(source.calls).toEqual(["items plan-1", "start plan-1"]);
    expect(started).toHaveBeenCalledWith({ jobId: "job-1" });
    expect(shown(dialog)).toBeNull();
    expect(dialog.isOpen).toBe(false);
  });

  it("discards the plan and starts nothing when the owner cancels, closes or presses Escape", async () => {
    for (const how of ["cancel", "close", "escape"] as const) {
      const { dialog, source } = await mount();
      await openAndChoose(dialog);
      control(dialog, "start").click();
      await settle();
      if (how === "escape") {
        shown(dialog)?.dispatchEvent(
          new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
        );
      } else {
        control(dialog, how).click();
      }
      await settle();
      expect(source.calls).toEqual(["items plan-1", "discard plan-1"]);
      expect(shown(dialog)).toBeNull();
      document.body.replaceChildren();
    }
  });

  it("changes the destination by discarding the plan and going back with the choice kept", async () => {
    const { dialog, source } = await mount();
    await openAndChoose(dialog, "f1");
    control(dialog, "start").click();
    await settle();
    control(dialog, "change").click();
    await settle();
    expect(source.calls).toEqual(["items plan-1", "discard plan-1"]);
    const frame = shown(dialog) as HTMLElement;
    expect(frame.classList.contains("import")).toBe(true);
    expect(frame.querySelector<HTMLInputElement>(".destination-field")?.value).toBe(
      "Harbour project › Site photos",
    );
    expect(control(dialog, "start").disabled).toBe(false);
  });

  it("disables Upload and says why when every file is already in the library", async () => {
    const plan = { ...PLAN, files_new: 0, files_present: 214, bytes_to_copy: 0 };
    const { dialog } = await mount(new MemoryImportSource({ plan, items: PLAN_ITEMS }));
    await openAndChoose(dialog);
    control(dialog, "start").click();
    await settle();
    expect(control(dialog, "start").disabled).toBe(true);
    expect(shown(dialog)?.querySelector(".foot .reason")?.textContent).toBe(
      labels.importNothingNew,
    );
  });

  it("goes back to the destination step when the owner closes the host's file dialog", async () => {
    const { dialog, source } = await mount(new MemoryImportSource());
    await openAndChoose(dialog);
    control(dialog, "start").click();
    await settle();
    expect(shown(dialog)?.classList.contains("import")).toBe(true);
    expect(rootOf(dialog).querySelector(".refusal")).toBeNull();
    expect(source.calls).toEqual([]);
  });

  it("discards a plan that arrives after the owner cancelled", async () => {
    const source = new MemoryImportSource({ plan: PLAN, items: PLAN_ITEMS });
    let release: () => void = () => undefined;
    source.gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const { dialog } = await mount(source);
    await openAndChoose(dialog);
    control(dialog, "start").click();
    await settle();
    control(dialog, "cancel").click();
    await settle();
    release();
    await settle();
    expect(source.calls).toEqual(["discard plan-1"]);
    expect(shown(dialog)).toBeNull();
  });

  it("shows a job a host without the review started at once", async () => {
    const source = new MemoryImportSource();
    source.plan = async () => ({ kind: "started", jobId: "job-9" });
    const { dialog } = await mount(source);
    const started = vi.fn();
    dialog.addEventListener(IMPORT_STARTED_EVENT, (event) =>
      started((event as CustomEvent).detail),
    );
    await openAndChoose(dialog);
    control(dialog, "start").click();
    await settle();
    expect(started).toHaveBeenCalledWith({ jobId: "job-9" });
  });

  it("keeps the plan open and shows the card when the start is refused", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new MemoryImportSource({ plan: PLAN, items: PLAN_ITEMS });
    source.startError = refusal(409, "conflict", "/api/v1/imports/plan-1/start");
    const { dialog } = await mount(source);
    await openAndChoose(dialog);
    control(dialog, "start").click();
    await settle();
    control(dialog, "start").click();
    await settle();
    expect(shown(dialog)?.classList.contains("review")).toBe(true);
    expect(rootOf(dialog).querySelector(".refusal .reason")?.textContent).toBe(
      labels.importSourceChanged,
    );
    expect(control(dialog, "start").disabled).toBe(false);
  });
});

describe("<fs-import-dialog> refusals (mockup 48)", () => {
  it("shows collection_not_found's card for a 404 of the plan request and Dismiss removes it", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new MemoryImportSource({ planError: refusal(404, "not_found") });
    const { dialog } = await mount(source);
    await openAndChoose(dialog);
    control(dialog, "start").click();
    await settle();
    const card = rootOf(dialog).querySelector(".refusal") as HTMLElement;
    expect(card.getAttribute("role")).toBe("alert");
    expect(card.querySelector(".title")?.textContent).toBe(labels.importRefusedTitle);
    expect(card.querySelector(".reason")?.textContent).toBe(
      "The collection you chose is no longer in this library. Choose another and try again.",
    );
    expect(card.querySelector("button")?.textContent).toBe(labels.fileDismiss);
    expect(control(dialog, "start").disabled).toBe(false);
    card.querySelector("button")?.click();
    expect(rootOf(dialog).querySelector(".refusal")).toBeNull();
    expect(shown(dialog)?.classList.contains("import")).toBe(true);
  });

  it("says a destination the host cannot name yet before sending anything", async () => {
    const source = new MemoryImportSource({ plan: PLAN, items: PLAN_ITEMS });
    const dialog = document.createElement("fs-import-dialog") as FsImportDialog;
    dialog.configure({
      labels,
      source,
      navigation: new MemoryNavigationSource(tree),
      directory: new NamedDestinations(),
    });
    document.body.append(dialog);
    await openAndChoose(dialog);
    control(dialog, "start").click();
    await settle();
    expect(rootOf(dialog).querySelector(".refusal .reason")?.textContent).toBe(
      labels.importDestinationUnresolved,
    );
    expect(source.requests).toEqual([]);
  });

  it("keeps the operation ID of a request whose answer was lost, and renews it after an answer", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new MemoryImportSource({ plan: PLAN, items: PLAN_ITEMS });
    const real = source.plan.bind(source);
    let calls = 0;
    source.plan = async (request) => {
      calls += 1;
      if (calls === 1) {
        source.requests.push(request);
        throw new TypeError("network down");
      }
      return real(request);
    };
    const { dialog } = await mount(source);
    await openAndChoose(dialog);
    control(dialog, "start").click();
    await settle();
    control(dialog, "start").click();
    await settle();
    expect(source.requests).toHaveLength(2);
    expect(source.requests[1]?.operationId).toBe(source.requests[0]?.operationId);
  });
});

describe("<fs-import-dialog> races (UI-1.2)", () => {
  it("sends the folder source when the folder option is chosen", async () => {
    const { dialog, source } = await mount();
    await openAndChoose(dialog);
    shown(dialog)?.querySelector<HTMLInputElement>('input[value="folder"]')?.click();
    control(dialog, "start").click();
    await settle();
    expect(source.requests[0]?.source).toBe("folder");
  });

  it("sends nothing when the owner cancels while the destination is being named", async () => {
    const source = new MemoryImportSource({ plan: PLAN, items: PLAN_ITEMS });
    let release: () => void = () => undefined;
    const slow = new NamedDestinations(
      () =>
        new Promise((resolve) => {
          release = () => resolve({ libraryId: LIBRARY_ID, collectionId: COLLECTION_ID });
        }),
    );
    const dialog = document.createElement("fs-import-dialog") as FsImportDialog;
    dialog.configure({
      labels,
      source,
      navigation: new MemoryNavigationSource(tree),
      directory: slow,
    });
    document.body.append(dialog);
    await openAndChoose(dialog);
    control(dialog, "start").click();
    await settle();
    control(dialog, "cancel").click();
    release();
    await settle();
    expect(source.requests).toEqual([]);
    expect(shown(dialog)).toBeNull();
  });

  it("does not send a request for a place the owner left while the destination was being named", async () => {
    const source = new MemoryImportSource({ plan: PLAN, items: PLAN_ITEMS });
    const releases: Array<() => void> = [];
    const slow = new NamedDestinations(
      () =>
        new Promise((resolve) => {
          releases.push(() => resolve({ libraryId: LIBRARY_ID, collectionId: COLLECTION_ID }));
        }),
    );
    const dialog = document.createElement("fs-import-dialog") as FsImportDialog;
    dialog.configure({
      labels,
      source,
      navigation: new MemoryNavigationSource(tree),
      directory: slow,
    });
    document.body.append(dialog);
    await openAndChoose(dialog, "c1");
    control(dialog, "start").click();
    await settle();
    row(dialog, "c1").querySelector<HTMLElement>(".chevron")?.click();
    await settle();
    row(dialog, "f1").click();
    await settle();
    releases.forEach((release) => release());
    await settle();
    expect(source.requests).toEqual([]);
  });

  it("keeps a place the owner chose while the preset one was still being read", async () => {
    const navigation = new MemoryNavigationSource(tree);
    const real = navigation.list.bind(navigation);
    let release: () => void = () => undefined;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    navigation.list = async (parent, cursor, limit) => {
      if (parent === "f1") await gate;
      return real(parent, cursor, limit);
    };
    const dialog = document.createElement("fs-import-dialog") as FsImportDialog;
    dialog.configure({
      labels,
      source: new MemoryImportSource({ plan: PLAN, items: PLAN_ITEMS }),
      navigation,
      directory: new NamedDestinations(async () => ({
        libraryId: LIBRARY_ID,
        collectionId: COLLECTION_ID,
      })),
    });
    document.body.append(dialog);
    dialog.open({ destination: "f1" });
    await settle();
    row(dialog, "c1").click();
    await settle();
    release();
    await settle();
    expect(shown(dialog)?.querySelector<HTMLInputElement>(".destination-field")?.value).toBe(
      "Harbour project",
    );
  });

  it("locks the review while the host starts the job, so Change and Cancel cannot replace it", async () => {
    const source = new MemoryImportSource({ plan: PLAN, items: PLAN_ITEMS });
    let release: (id: string) => void = () => undefined;
    source.start = (planId) =>
      new Promise((resolve) => {
        source.calls.push(`start ${planId}`);
        release = resolve;
      });
    const { dialog } = await mount(source);
    const started = vi.fn();
    dialog.addEventListener(IMPORT_STARTED_EVENT, (event) =>
      started((event as CustomEvent).detail),
    );
    await openAndChoose(dialog);
    control(dialog, "start").click();
    await settle();
    control(dialog, "start").click();
    await settle();
    expect(control(dialog, "cancel").disabled).toBe(true);
    expect(control(dialog, "change").disabled).toBe(true);
    shown(dialog)?.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    await settle();
    expect(shown(dialog)).not.toBeNull();
    release("job-1");
    await settle();
    expect(started).toHaveBeenCalledWith({ jobId: "job-1" });
    expect(shown(dialog)).toBeNull();
  });

  it("announces a job the host started after the owner cancelled, without closing a newer dialog", async () => {
    const source = new MemoryImportSource();
    let release: () => void = () => undefined;
    source.plan = () =>
      new Promise((resolve) => {
        release = () => resolve({ kind: "started", jobId: "job-late" });
      });
    const { dialog } = await mount(source);
    const started = vi.fn();
    dialog.addEventListener(IMPORT_STARTED_EVENT, (event) =>
      started((event as CustomEvent).detail),
    );
    await openAndChoose(dialog);
    control(dialog, "start").click();
    await settle();
    control(dialog, "cancel").click();
    await settle();
    dialog.open();
    release();
    await settle();
    expect(started).toHaveBeenCalledWith({ jobId: "job-late" });
    expect(dialog.isOpen).toBe(true);
  });
});
