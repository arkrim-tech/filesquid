/**
 * The model behind `<fs-provenance-panel>`, the Provenance tab (UI-F2.15; UI-S-020, UI-S-022,
 * UI-S-031 mockup 36; API-S-029; ADR 040): the history types from the generated API contract
 * and the pure rules over them: the activities newest first, the agents classified by their
 * identifiers (the installation agent `urn:uuid:<installation>`, the host software agent with
 * its product and version, a tool, an unrecorded agent), the `filesquid-prov/1` JSON-LD record
 * of one version parsed with bounds, the `prov:` attribute rows of one activity, the words of
 * every operation kind, and the bounded paging of the history route. Nothing here fetches or
 * touches the DOM, and nothing is invented: a relation the record does not carry has no row.
 *
 * Every list is bounded (AGENTS.md section 22) by the `MAX_*` constants.
 */
import type { components } from "../api/schema";
import { formatLabel, type LabelKey, type LabelTable } from "../theme/labels";
import { utcTime, type PropertyRow } from "./details";
import type { FileRef } from "./model";
import { shortDigest, shortIdentifier } from "./versions";

/** One activity of a file's history, generated from the API contract. */
export type HistoryEntry = components["schemas"]["HistoryEntry"];
/** One page of history entries, with the next cursor and the index's truncation flag. */
export type HistoryPage = components["schemas"]["HistoryPage"];
/** The kind of a committed operation. */
export type OperationKind = components["schemas"]["OperationKind"];

/** Most history pages read for one file (200 entries each): 5,000 activities. */
export const MAX_HISTORY_PAGES = 25;
/** Most nodes of one record's graph the parser reads. */
export const MAX_RECORD_NODES = 4096;
/** The profile this model reads (ADR 040 Decision 4). */
const PROFILE_V1 = "filesquid-prov/1";

/** Every operation kind of the contract, each with a word in the label table. */
export const OPERATION_KINDS: readonly OperationKind[] = [
  "import",
  "upload",
  "copy",
  "rename",
  "move",
  "hide",
  "unhide",
  "delete",
  "restore",
  "purge",
  "metadata.edit",
  "label",
  "revert",
  "reinstate",
  "snapshot",
  "catalog.organisation",
  "repair",
  "export",
  "scrub",
  "publish",
  "backfill",
  "tombstone",
  "unknown",
];

/** The label key of each operation kind's word. */
const KIND_KEYS: Readonly<Record<OperationKind, LabelKey>> = {
  import: "provenanceKindImport",
  upload: "provenanceKindUpload",
  copy: "provenanceKindCopy",
  rename: "provenanceKindRename",
  move: "provenanceKindMove",
  hide: "provenanceKindHide",
  unhide: "provenanceKindUnhide",
  delete: "provenanceKindDelete",
  restore: "provenanceKindRestore",
  purge: "provenanceKindPurge",
  "metadata.edit": "provenanceKindMetadataEdit",
  label: "provenanceKindLabel",
  revert: "provenanceKindRevert",
  reinstate: "provenanceKindReinstate",
  snapshot: "provenanceKindSnapshot",
  "catalog.organisation": "provenanceKindCatalogOrganisation",
  repair: "provenanceKindRepair",
  export: "provenanceKindExport",
  scrub: "provenanceKindScrub",
  publish: "provenanceKindPublish",
  backfill: "provenanceKindBackfill",
  tombstone: "provenanceKindTombstone",
  unknown: "provenanceKindUnknown",
};

/**
 * The words of an operation kind (`metadata edit`, `remove`, `restore`). Pure.
 *
 * @param kind The activity's operation kind.
 * @param labels The active label table.
 * @returns The word from the label table.
 */
export function kindWords(kind: OperationKind, labels: LabelTable): string {
  return labels[KIND_KEYS[kind] ?? KIND_KEYS.unknown];
}

/** An agent as the tab shows it, classified by its identifier (ADR 040 Decision 4). */
export type AgentView =
  | { readonly id: string; readonly kind: "installation" }
  | {
      readonly id: string;
      readonly kind: "software";
      readonly product: string;
      readonly version: string;
    }
  | { readonly id: string; readonly kind: "tool"; readonly name: string; readonly version: string }
  | { readonly id: string; readonly kind: "unrecorded-installation" | "unrecorded-host" }
  | { readonly id: string; readonly kind: "other" };

/** The identifier forms of `filesquid-prov` (its `ids` module). */
const SOFTWARE_PREFIX = "urn:filesquid:software:";
const TOOL_PREFIX = "urn:filesquid:tool:";
const UNRECORDED_PREFIX = "urn:filesquid:agent:unrecorded:";
const URN_UUID_ID = /^urn:uuid:[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** The percent-decoded parts of an identifier tail, or undefined when it has the wrong count or does not decode. */
function decodedParts(tail: string, count: number): string[] | undefined {
  const parts = tail.split(":");
  if (parts.length !== count) return undefined;
  try {
    return parts.map((part) => decodeURIComponent(part));
  } catch (error) {
    console.warn("FileSquid ignored an agent identifier that does not decode", errorName(error));
    return undefined;
  }
}

/**
 * Classify an agent identifier of a record: the installation agent `urn:uuid:<uuid>`, the host
 * software agent `urn:filesquid:software:<product>:<distribution>:<version>`, a tool
 * `urn:filesquid:tool:<name>:<version>`, an unrecorded agent
 * `urn:filesquid:agent:unrecorded:<role>`, or any other identifier. Pure; never throws.
 *
 * @param id The agent's identifier.
 * @returns The agent as the tab shows it.
 */
export function classifyAgent(id: string): AgentView {
  if (URN_UUID_ID.test(id)) return { id, kind: "installation" };
  if (id.startsWith(SOFTWARE_PREFIX)) {
    const parts = decodedParts(id.slice(SOFTWARE_PREFIX.length), 3);
    if (parts) return { id, kind: "software", product: parts[0] ?? "", version: parts[2] ?? "" };
  }
  if (id.startsWith(TOOL_PREFIX)) {
    const parts = decodedParts(id.slice(TOOL_PREFIX.length), 2);
    if (parts) return { id, kind: "tool", name: parts[0] ?? "", version: parts[1] ?? "" };
  }
  if (id.startsWith(UNRECORDED_PREFIX)) {
    const role = id.slice(UNRECORDED_PREFIX.length);
    if (role === "installation") return { id, kind: "unrecorded-installation" };
    if (role === "host") return { id, kind: "unrecorded-host" };
  }
  return { id, kind: "other" };
}

/** One activity as the tab lists it. */
export interface ActivityView {
  /** Stable key: the version and the operation (one operation commits one version per object). */
  readonly key: string;
  /** The operation kind. */
  readonly rawKind: OperationKind;
  /** The operation's ID, shared by every object version it committed. */
  readonly operationId: string;
  /** The OCFL object the activity committed a version of. */
  readonly objectId: string;
  /** The version it committed, `v4`. */
  readonly version: string;
  /** When the operation started; null for a reconstructed record. */
  readonly startedAt: string | null;
  /** When the version was committed. */
  readonly endedAt: string;
  /** True for a record reconstructed from the inventory afterwards. */
  readonly reconstructed: boolean;
  /** The agents the activity was associated with. */
  readonly agents: readonly AgentView[];
}

/**
 * The activities newest first. The route answers oldest first, by commit time and then
 * activity, so the list is reversed. Pure.
 *
 * @param entries The entries the history route answered, oldest first.
 * @returns One view per entry, newest first.
 */
export function activityViews(entries: readonly HistoryEntry[]): ActivityView[] {
  return entries
    .map((entry): ActivityView => ({
      key: `${entry.version.version}:${entry.operation_id}`,
      rawKind: entry.kind,
      operationId: entry.operation_id,
      objectId: entry.version.object_id,
      version: entry.version.version,
      startedAt: entry.started_at,
      endedAt: entry.ended_at,
      reconstructed: entry.reconstructed,
      agents: entry.agents.map(classifyAgent),
    }))
    .reverse();
}

/**
 * A time in UTC for an activity: a short date, a 24-hour clock and "UTC". Pure.
 *
 * @param iso An RFC 3339 time.
 * @param locale BCP 47 tag for the date format.
 * @returns For example `Oct 16, 2026, 11:05 UTC`; a time that does not parse comes back as it is.
 */
export function activityTime(iso: string, locale?: string): string {
  const shown = utcTime(iso, locale);
  return shown === iso ? iso : `${shown} UTC`;
}

/** An entity of a record: one state of one file or metadata record at one version. */
export interface EntityRef {
  /** The entity's identifier. */
  readonly id: string;
  /** The OCFL object. */
  readonly objectId: string;
  /** The OCFL version, `v3`. */
  readonly version: string;
  /** The logical path the record states. */
  readonly path: string;
  /** The SHA-256 the record states; null when the entity is named only by its identifier. */
  readonly sha256: string | null;
}

/** An entity the activity generated, with the entities it derives from. */
export interface GeneratedEntity {
  /** The generated entity. */
  readonly entity: EntityRef;
  /** The entities it was derived from. */
  readonly derivedFrom: readonly EntityRef[];
}

/** The relations of one activity's record. */
export interface ProvRecord {
  /** `prov:used`. */
  readonly used: readonly EntityRef[];
  /** `prov:wasGeneratedBy` and `prov:wasDerivedFrom`. */
  readonly generated: readonly GeneratedEntity[];
  /** `prov:wasInvalidatedBy`. */
  readonly invalidated: readonly EntityRef[];
}

/** A JSON object, or undefined. */
function asObject(value: unknown): Record<string, unknown> | undefined {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;
}

/** A string member, or undefined. */
function asString(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

/** The `@id`s of a node-reference member: one reference or a list of them. */
function referenceIds(value: unknown): string[] {
  const list = Array.isArray(value) ? value : value === undefined ? [] : [value];
  return list
    .map((item) => asString(asObject(item)?.["@id"]))
    .filter((id): id is string => id !== undefined);
}

/** The entity of a graph node, or undefined when it lacks the members the profile requires. */
function entityOfNode(node: Record<string, unknown>): EntityRef | undefined {
  const id = asString(node["@id"]);
  const objectId = asString(node["fsq:objectId"]);
  const version = asString(node["fsq:version"]);
  const path = asString(node["fsq:path"]);
  if (id === undefined || objectId === undefined || version === undefined || path === undefined) {
    return undefined;
  }
  return { id, objectId, version, path, sha256: asString(node["fsq:sha256"]) ?? null };
}

/** `urn:filesquid:entity:<object-id>:<version>:<percent-encoded path>`. */
const ENTITY_ID = /^urn:filesquid:entity:(.+?):(v\d+):([^:]*)$/;

/** An entity named only by its identifier, from the identifier's parts; undefined when it has another form. */
function entityOfId(id: string): EntityRef | undefined {
  const match = ENTITY_ID.exec(id);
  if (!match) return undefined;
  const [, objectId = "", version = "", encoded = ""] = match;
  let path = encoded;
  try {
    path = decodeURIComponent(encoded);
  } catch (error) {
    console.warn("FileSquid kept an entity path that does not decode", errorName(error));
  }
  return { id, objectId, version, path, sha256: null };
}

/** The kind of an error, without its message. */
function errorName(error: unknown): string {
  return error instanceof Error ? error.name : typeof error;
}

/**
 * Parse the `filesquid-prov/1` record of one version (the answer of `GET
 * /files/{id}/versions/{version}/prov`) into the relations the tab shows. The graph is bounded
 * by {@link MAX_RECORD_NODES}; a reference to an entity the graph does not hold is named from
 * its identifier. Pure apart from a console warning.
 *
 * @param json The record, parsed.
 * @returns The used, generated, derived and invalidated entities.
 * @throws TypeError when the record is not a JSON object of profile `filesquid-prov/1` with a
 *   bounded `@graph`.
 */
export function parseProvRecord(json: unknown): ProvRecord {
  const document = asObject(json);
  if (document?.["fsq:profile"] !== PROFILE_V1)
    throw new TypeError("not a filesquid-prov/1 record");
  const graph = document["@graph"];
  if (!Array.isArray(graph) || graph.length > MAX_RECORD_NODES) {
    throw new TypeError("the record's graph is missing or too large");
  }
  const nodes = graph
    .map(asObject)
    .filter((node): node is Record<string, unknown> => node !== undefined);
  const activity = nodes.find((node) => node["@type"] === "prov:Activity");
  const activityId = asString(activity?.["@id"]);
  const entities = new Map<string, EntityRef>();
  for (const node of nodes) {
    const entity = entityOfNode(node);
    if (entity) entities.set(entity.id, entity);
  }
  const resolve = (id: string): EntityRef | undefined => entities.get(id) ?? entityOfId(id);
  const resolveAll = (ids: readonly string[]) =>
    ids.map(resolve).filter((entity): entity is EntityRef => entity !== undefined);
  const byActivity = (member: string) =>
    nodes.filter((node) => referenceIds(node[member]).includes(activityId ?? "\0"));
  return {
    used: resolveAll(referenceIds(activity?.["prov:used"])),
    generated: byActivity("prov:wasGeneratedBy").flatMap((node) => {
      const entity = entityOfNode(node);
      return entity
        ? [{ entity, derivedFrom: resolveAll(referenceIds(node["prov:wasDerivedFrom"])) }]
        : [];
    }),
    invalidated: byActivity("prov:wasInvalidatedBy")
      .map(entityOfNode)
      .filter((entity): entity is EntityRef => entity !== undefined),
  };
}

/** One value of an attribute: its text and how it is drawn. */
export interface AttributeValue {
  /** The text shown. */
  readonly text: string;
  /** The whole text a copy puts on the clipboard; no copy button when absent. */
  readonly copy?: string;
  /** `mono` for identifiers and digests, `muted` for a note beside one, `plain` for words. */
  readonly tone: "mono" | "mono-muted" | "plain" | "muted";
}

/** One `prov:` attribute of an activity: its name over one or more values. */
export interface Attribute {
  /** The attribute's name, for example `prov:used`. */
  readonly name: string;
  /** Its values, in the order the mockup draws them. */
  readonly values: readonly AttributeValue[];
}

/** An entity as `<object>@<version>`; an entity of the activity's own object is `…<last six>@<version>`. */
function entityText(entity: EntityRef, objectId: string): string {
  if (entity.objectId === objectId) return `…${objectId.slice(-6)}@${entity.version}`;
  return `${shortIdentifier(entity.objectId)}@${entity.version}`;
}

/** An entity as `<short object>@<version>` always, for the entities an activity used or generated. */
function entityLine(entity: EntityRef): AttributeValue {
  return {
    text: `${shortIdentifier(entity.objectId)}@${entity.version}`,
    copy: entity.id,
    tone: "mono",
  };
}

/** The values of `prov:wasAssociatedWith` for one agent. */
function agentValues(agent: AgentView, labels: LabelTable): AttributeValue[] {
  switch (agent.kind) {
    case "installation":
      return [
        { text: shortIdentifier(agent.id), copy: agent.id, tone: "mono" },
        { text: labels.provenanceAgentInstallation, tone: "muted" },
      ];
    case "software":
      return [
        {
          text: formatLabel(labels.provenanceAgentSoftware, {
            product: agent.product,
            version: agent.version,
          }),
          copy: agent.id,
          tone: "plain",
        },
        { text: "prov:SoftwareAgent", tone: "mono-muted" },
      ];
    case "tool":
      return [
        {
          text: formatLabel(labels.provenanceAgentTool, {
            name: agent.name,
            version: agent.version,
          }),
          copy: agent.id,
          tone: "plain",
        },
        { text: "prov:SoftwareAgent", tone: "mono-muted" },
      ];
    case "unrecorded-installation":
      return [{ text: labels.provenanceAgentUnrecordedInstallation, tone: "muted" }];
    case "unrecorded-host":
      return [{ text: labels.provenanceAgentUnrecordedHost, tone: "muted" }];
    default:
      return [
        { text: shortIdentifier(agent.id), copy: agent.id, tone: "mono" },
        { text: labels.provenanceAgentOther, tone: "muted" },
      ];
  }
}

/** The values of `prov:used` or `prov:wasInvalidatedBy` for one entity: the id, the path and the digest. */
function entityValues(entity: EntityRef): AttributeValue[] {
  const values: AttributeValue[] = [entityLine(entity), { text: entity.path, tone: "plain" }];
  if (entity.sha256) {
    values.push({
      text: `sha256 ${shortDigest(entity.sha256)}`,
      copy: entity.sha256,
      tone: "mono-muted",
    });
  }
  return values;
}

/** The time attributes of an activity: `prov:startedAtTime` and `prov:endedAtTime`. */
function timeAttributes(view: ActivityView, labels: LabelTable): Attribute[] {
  const started: AttributeValue = view.startedAt
    ? { text: view.startedAt, copy: view.startedAt, tone: "mono" }
    : { text: labels.provenanceNotRecorded, tone: "muted" };
  const ended: AttributeValue = { text: view.endedAt, copy: view.endedAt, tone: "mono" };
  return [
    { name: "prov:startedAtTime", values: [started] },
    { name: "prov:endedAtTime", values: [ended] },
  ];
}

/** The relation attributes a record adds, each only when the record carries it. */
function relationAttributes(
  view: ActivityView,
  record: ProvRecord,
  labels: LabelTable,
): Attribute[] {
  const rows: Attribute[] = [];
  if (record.used.length > 0) {
    rows.push({ name: "prov:used", values: record.used.flatMap(entityValues) });
  }
  for (const { entity } of record.generated) {
    const name = formatLabel(labels.provenanceGeneratedBy, { version: entity.version });
    rows.push({ name, values: [entityLine(entity)] });
  }
  const derived = new Map<string, EntityRef>();
  for (const generated of record.generated) {
    for (const source of generated.derivedFrom) derived.set(source.id, source);
  }
  if (derived.size > 0) {
    const values = [...derived.values()].map((source): AttributeValue => ({
      text: entityText(source, view.objectId),
      copy: source.id,
      tone: "mono",
    }));
    rows.push({ name: "prov:wasDerivedFrom", values });
  }
  if (record.invalidated.length > 0) {
    rows.push({ name: "prov:wasInvalidatedBy", values: record.invalidated.flatMap(entityValues) });
  }
  return rows;
}

/**
 * The `prov:` attribute rows of one activity in the order of mockup 36: the times and the
 * agents from the history entry, then the relations of the version's record when it has been
 * read. A time that was not recorded and an agent that was not recorded are shown as such. Pure.
 *
 * @param view The activity.
 * @param record Its parsed record, or undefined when it has not been read.
 * @param labels The active label table.
 * @param locale BCP 47 tag (kept for the signature of the other rows; identifiers and RFC 3339
 *   times are not localised).
 * @returns The attributes, each a name over its values.
 */
export function activityAttributes(
  view: ActivityView,
  record: ProvRecord | undefined,
  labels: LabelTable,
  locale?: string,
): Attribute[] {
  void locale;
  const rows = timeAttributes(view, labels);
  if (view.agents.length > 0) {
    rows.push({
      name: "prov:wasAssociatedWith",
      values: view.agents.flatMap((a) => agentValues(a, labels)),
    });
  }
  return record ? [...rows, ...relationAttributes(view, record, labels)] : rows;
}

/** The words that describe an agent, for the in-tab filter. */
function agentWords(agent: AgentView): string {
  if (agent.kind === "software") return `${agent.product} ${agent.version}`;
  if (agent.kind === "tool") return `${agent.name} ${agent.version}`;
  return "";
}

/**
 * The row the shared table keeps for one activity: its kind and version, and the text the
 * in-tab filter matches (kind words and name, version, operation ID, times, agents, and the
 * paths and digests of its record once read). Pure.
 *
 * @param view The activity.
 * @param labels The active label table.
 * @param locale BCP 47 tag for the shown time.
 * @param record Its parsed record, when read.
 * @returns A property row whose `key` is the activity's key.
 */
export function activityRow(
  view: ActivityView,
  labels: LabelTable,
  locale?: string,
  record?: ProvRecord,
): PropertyRow {
  const entities = record
    ? [
        ...record.used,
        ...record.generated.flatMap((g) => [g.entity, ...g.derivedFrom]),
        ...record.invalidated,
      ]
    : [];
  const searchText = [
    kindWords(view.rawKind, labels),
    view.rawKind,
    view.version,
    view.operationId,
    view.objectId,
    view.startedAt ?? "",
    view.endedAt,
    activityTime(view.endedAt, locale),
    ...view.agents.flatMap((agent) => [agent.id, agentWords(agent)]),
    ...entities.flatMap((entity) => [entity.path, entity.sha256 ?? "", entity.id]),
  ].join("\n");
  return { key: view.key, label: kindWords(view.rawKind, labels), value: view.version, searchText };
}

/** What the paging of one file's history found. */
export interface AllHistory {
  /** Every entry read, oldest first. */
  readonly entries: HistoryEntry[];
  /** True when the API's index stopped at its own cap, so some activities may be missing. */
  readonly truncated: boolean;
  /** True when this reader stopped at {@link MAX_HISTORY_PAGES} (or a repeated cursor) with more to read. */
  readonly capped: boolean;
}

/** The part of a source the paging needs. */
export interface HistoryPager {
  /** One page of the file's activities, oldest first. */
  history(file: FileRef, options?: { cursor?: string; signal?: AbortSignal }): Promise<HistoryPage>;
}

/**
 * Read a file's history page by page, oldest first, to its last page. The loop stops at
 * {@link MAX_HISTORY_PAGES} pages or when a page repeats its cursor, and says so in `capped`;
 * it stops at once when the signal aborts. Network I/O through the source.
 *
 * @param source Reads one page.
 * @param file The file.
 * @param options Abort signal.
 * @returns The entries and whether the list is partial.
 * @throws DOMException `AbortError` when the signal aborts; whatever the source rejects with.
 */
export async function readAllHistory(
  source: HistoryPager,
  file: FileRef,
  options: { signal?: AbortSignal },
): Promise<AllHistory> {
  const entries: HistoryEntry[] = [];
  let cursor: string | undefined;
  let truncated = false;
  for (let page = 0; page < MAX_HISTORY_PAGES; page += 1) {
    const answer = await source.history(file, { cursor, signal: options.signal });
    if (options.signal?.aborted) throw new DOMException("aborted", "AbortError");
    entries.push(...answer.items);
    truncated = truncated || answer.truncated;
    const next = answer.next_cursor;
    if (next === null) return { entries, truncated, capped: false };
    if (next === cursor) return { entries, truncated, capped: true };
    cursor = next;
  }
  return { entries, truncated, capped: true };
}
