/**
 * The File info rows of the Details panel (UI-F2.3, UI-S-003, UI-S-031 mockup 35): the storage
 * facts of `GET /api/v1/files/details` with the version total of `GET /api/v1/files/versions`,
 * worded from the label table as property rows of a label over a value. Storage facts only,
 * each owned by the OCFL module (UI-S-003); the stored name and the logical path are never
 * shown (UI-S-019), and the API never answers a content location (API-S-005). UI-F2.15 adds
 * the facts projection of the file's record (`GET /api/v1/files/facts`: original name,
 * extension, format and media type, SHA-256 and the import facts, ADR 038) and the technical
 * rows of the file's `exif:` statements. Pure.
 */
import { formatLabel, type LabelKey, type LabelTable } from "../theme/labels";
import type { FileFacts, ImportSummary, RawName } from "./facts-source";
import type { FileHistorySize, ObjectVersions } from "./history-source";
import { TECHNICAL_PREFIXES, type MetadataStatement } from "./metadata";
import { formatSize, type FileDetails } from "./model";
import { shortDigest, shortIdentifier, versionCountText } from "./versions";

/** One property row of a file panel table. */
export interface PropertyRow {
  /** Stable key of the row, for tests and `data-key`. */
  readonly key: string;
  /** The property, as the user sees it. */
  readonly label: string;
  /** The value shown. */
  readonly value: string;
  /** The text a copy puts on the clipboard; the value when absent. */
  readonly copy?: string;
  /** True for identifiers, codes, digests and extensions, set in the mono face. */
  readonly mono?: boolean;
  /**
   * A label that is a property key (a CURIE), set in the mono face: `owner` in `--fs-primary`
   * for an owner-asserted value, `extracted` in the secondary colour for a value read from
   * the file (UI-S-031 mockup 28). A plain label when absent.
   */
  readonly labelStyle?: "owner" | "extracted";
  /**
   * The eyebrow of the group the row belongs to (UI-F2.15: "Technical · read from the file").
   * Rows of one group follow each other under one eyebrow, after the ungrouped rows, and are
   * not counted as properties.
   */
  readonly group?: string;
  /**
   * `note` draws the value alone, as a line of secondary text with no label and no copy
   * (the "Nothing could be read from this format." line).
   */
  readonly display?: "note";
  /**
   * The text the in-tab filter matches when it is not the label, value and copy text (a
   * Provenance activity matches its operation ID, times, agents and entities).
   */
  readonly searchText?: string;
}

/** Most fixity algorithms shown besides the content digest. */
export const MAX_FIXITY_ROWS = 8;

/** The algorithm the mockup shows first. */
const PREFERRED = "sha256";

/** An algorithm name as people write it: `sha256` is `SHA-256`, `md5` is `MD5`. Pure. */
export function algorithmName(algorithm: string): string {
  return algorithm.toUpperCase().replace(/^SHA(\d)/, "SHA-$1");
}

/**
 * The digest rows of one file: SHA-256 first when known, then the content digest, then the
 * other fixity values the inventory records, each algorithm once. Pure.
 *
 * @param details The storage facts.
 * @param labels The active label table.
 * @param sha256 The SHA-256 of the file's record (`GET /files/facts`), which takes the SHA-256
 *   row when given; the inventory's fixity block otherwise.
 * @returns One row per algorithm, at most {@link MAX_FIXITY_ROWS} plus the content digest.
 */
export function digestRows(
  details: FileDetails,
  labels: LabelTable,
  sha256?: string,
): PropertyRow[] {
  const fixity = new Map<string, string>();
  const preferred = sha256 ?? details.fixity[PREFERRED]?.[0];
  if (preferred && details.digest_algorithm !== PREFERRED) fixity.set(PREFERRED, preferred);
  for (const [algorithm, digests] of Object.entries(details.fixity)) {
    const first = digests[0];
    const known = algorithm === details.digest_algorithm || fixity.has(algorithm);
    if (first && !known && fixity.size < MAX_FIXITY_ROWS) fixity.set(algorithm, first);
  }
  const values = new Map<string, string>();
  if (fixity.has(PREFERRED)) values.set(PREFERRED, fixity.get(PREFERRED) as string);
  values.set(details.digest_algorithm, details.digest);
  for (const [algorithm, digest] of fixity) values.set(algorithm, digest);
  return [...values].map(([algorithm, digest]) => ({
    key: `digest-${algorithm}`,
    label: formatLabel(labels.filePanelDigest, { algorithm: algorithmName(algorithm) }),
    value: shortDigest(digest),
    copy: digest,
    mono: true,
  }));
}

/**
 * A time in UTC as a short date and a 24-hour clock, or the text unchanged. Pure.
 *
 * @param iso An RFC 3339 time.
 * @param locale BCP 47 tag for the format.
 * @returns For example `Oct 16, 2026, 11:05`.
 */
export function utcTime(iso: string, locale?: string): string {
  const time = new Date(iso);
  if (Number.isNaN(time.getTime())) return iso;
  return new Intl.DateTimeFormat(locale, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
    timeZone: "UTC",
  }).format(time);
}

/** The OCFL version row: "v4 of 4 · 16 Oct 2026, 11:05 UTC". */
function versionRow(
  details: FileDetails,
  versions: ObjectVersions,
  labels: LabelTable,
  locale?: string,
): PropertyRow {
  const record = versions.versions.find((v) => v.version === details.version);
  const created = record?.created ?? details.last_change.created;
  const value = formatLabel(labels.filePanelVersionValue, {
    version: details.version,
    total: new Intl.NumberFormat(locale).format(versions.versions.length),
    time: utcTime(created, locale),
  });
  return { key: "version", label: labels.filePanelVersion, value };
}

/** The size row: "6.2 MB · 6,481,920 bytes". */
function sizeRow(details: FileDetails, labels: LabelTable, locale?: string): PropertyRow {
  const value = formatLabel(labels.filePanelSizeValue, {
    size: formatSize(details.size, labels, locale),
    bytes: new Intl.NumberFormat(locale).format(details.size),
  });
  return { key: "size", label: labels.filePanelSize, value, copy: String(details.size) };
}

/**
 * The history size row (UI-F2.11, PRD F-065: storage used by history is reported per file):
 * "{size} · {count} versions", the bytes the file's history keeps, each content once. Pure.
 *
 * @param history The history size of the file.
 * @param labels The active label table.
 * @param locale BCP 47 tag for numbers.
 * @returns The row.
 */
function historySizeRow(
  history: Pick<FileHistorySize, "versions" | "bytes">,
  labels: LabelTable,
  locale?: string,
): PropertyRow {
  const value = formatLabel(labels.filePanelHistorySizeValue, {
    size: formatSize(history.bytes, labels, locale),
    versions: versionCountText(history.versions, labels, locale),
  });
  return { key: "history-size", label: labels.filePanelHistorySize, value };
}

/** What a File info tab reads besides the storage facts; each is optional. */
export interface DetailExtras {
  /** The facts projection of the file's record (`GET /files/facts`). */
  readonly facts?: FileFacts;
  /** The file's statements (`GET /files/metadata`); the `exif:` ones are the technical rows. */
  readonly technical?: readonly MetadataStatement[];
}

/** Most acquisitions the File info tab draws rows for. */
export const MAX_IMPORT_ROWS = 10;
/** Most technical rows the File info tab draws. */
export const MAX_TECHNICAL_ROWS = 50;

/** The label key of each way a file reached FileSquid. */
const METHOD_KEYS: Readonly<Record<ImportSummary["method"], LabelKey>> = {
  file_picker: "filePanelMethodFilePicker",
  folder: "filePanelMethodFolder",
  drag_and_drop: "filePanelMethodDragAndDrop",
  other_application: "filePanelMethodOtherApplication",
  api_upload: "filePanelMethodApiUpload",
};

/** A name of the record as text: the text, or the base64 bytes of a name that is not UTF-8. */
function rawNameText(name: RawName, labels: LabelTable): { text: string; mono: boolean } {
  if (typeof name === "string") return { text: name, mono: false };
  return { text: formatLabel(labels.filePanelRawName, { value: name.value }), mono: true };
}

/** A time in the viewer's time zone as a short date and a 24-hour clock, or the text unchanged. */
function localTime(iso: string, locale?: string): string {
  const time = new Date(iso);
  if (Number.isNaN(time.getTime())) return iso;
  return new Intl.DateTimeFormat(locale, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
  }).format(time);
}

/** The 24-hour clock of a time in UTC, or the text unchanged. */
function utcClock(iso: string, locale?: string): string {
  const time = new Date(iso);
  if (Number.isNaN(time.getTime())) return iso;
  return new Intl.DateTimeFormat(locale, {
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
    timeZone: "UTC",
  }).format(time);
}

/** The key of the `index`th row of a kind: the first is the plain key. */
function numberedKey(key: string, index: number): string {
  return index === 0 ? key : `${key}-${index + 1}`;
}

/**
 * The rows of one acquisition (ADR 038): Uploaded `{local} ({utc} UTC)`, Source `{method} ·
 * volume {volume}` (the method alone when the volume is unknown) and, only when the API knows
 * it, the path relative to the import root. The absolute path, the import root, the host and
 * the account have no row: the API never sends them (API-S-027). Pure.
 */
function importRows(
  entry: ImportSummary,
  index: number,
  labels: LabelTable,
  locale?: string,
): PropertyRow[] {
  const uploaded = formatLabel(labels.filePanelUploadedValue, {
    local: localTime(entry.imported_at, locale),
    utc: utcClock(entry.imported_at, locale),
  });
  const method = labels[METHOD_KEYS[entry.method]];
  // The volume and the path are names as the source reported them: text, or base64 bytes.
  const volume = entry.volume_name === null ? undefined : rawNameText(entry.volume_name, labels);
  const source = volume
    ? formatLabel(labels.filePanelSourceValue, { method, volume: volume.text })
    : method;
  const path = entry.relative_path === null ? undefined : rawNameText(entry.relative_path, labels);
  const rows: PropertyRow[] = [
    { key: numberedKey("uploaded", index), label: labels.filePanelUploaded, value: uploaded },
    { key: numberedKey("source", index), label: labels.filePanelSource, value: source },
  ];
  if (path) {
    const key = numberedKey("import-path", index);
    rows.push({ key, label: labels.filePanelImportPath, value: path.text, mono: true });
  }
  return rows;
}

/** The note that says how many acquisitions have no rows, or none when all are shown. */
function moreImportsRow(total: number, labels: LabelTable, locale?: string): PropertyRow[] {
  if (total <= MAX_IMPORT_ROWS) return [];
  const count = new Intl.NumberFormat(locale).format(total - MAX_IMPORT_ROWS);
  const value = formatLabel(labels.filePanelImportsMore, { count });
  return [{ key: "imports-more", label: value, value, display: "note" }];
}

/** A term name as words: `PixelXDimension` is `Pixel X Dimension`, `GPSLatitude` is `GPS Latitude`. */
function termWords(term: string): string {
  return term
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/([A-Z]+)([A-Z][a-z])/g, "$1 $2")
    .replace(/[_.-]+/g, " ");
}

/**
 * The technical rows of a file (UI-S-031: "TECHNICAL · READ FROM THE FILE"): its `exif:`
 * statements, read from the file, under their own group, or one note when the file has none.
 * Pure.
 *
 * @param statements The file's statements.
 * @param labels The active label table.
 * @returns The group's rows, at most {@link MAX_TECHNICAL_ROWS}; one note row when there is none.
 */
function technicalRows(
  statements: readonly MetadataStatement[],
  labels: LabelTable,
): PropertyRow[] {
  const group = labels.filePanelTechnicalGroup;
  const technical = statements.filter((s) => TECHNICAL_PREFIXES.includes(s.prefix));
  if (technical.length === 0) {
    const value = labels.filePanelTechnicalNone;
    return [{ key: "technical-none", label: value, value, display: "note", group }];
  }
  return technical.slice(0, MAX_TECHNICAL_ROWS).map((s) => ({
    key: `technical-${s.term}`,
    label: termWords(s.term),
    value: s.value,
    group,
  }));
}

/** The name, extension and format rows, from the facts when the file has them. */
function identityRows(
  details: FileDetails,
  labels: LabelTable,
  facts: FileFacts | undefined,
): PropertyRow[] {
  const name = facts
    ? rawNameText(facts.original_name, labels)
    : { text: details.display_name, mono: false };
  const extension = facts?.extension ?? details.type_hint;
  const media = facts?.media_type ?? details.media_type;
  const format = facts?.format;
  const formatValue = format
    ? formatLabel(labels.filePanelFormatValue, { format, mime: media })
    : media;
  const rows: PropertyRow[] = [
    { key: "name", label: labels.filePanelOriginalName, value: name.text, mono: name.mono },
  ];
  if (extension) {
    const value = `.${extension}`;
    rows.push({ key: "extension", label: labels.filePanelExtension, value, mono: true });
  }
  rows.push({ key: "format", label: labels.filePanelFormat, value: formatValue });
  return rows;
}

/**
 * The File info rows of one file version, in the order of mockup 35, with the history size
 * after the size, the import facts after the digests, and the technical group last. Pure.
 *
 * @param details The storage facts of the version.
 * @param versions Every version of its object (for the total and the version's time).
 * @param labels The active label table.
 * @param locale BCP 47 tag for numbers and dates.
 * @param history The file's history size; the row is left out when it could not be read.
 * @param extras The facts projection and the file's statements; each row group is left out
 *   when its read is absent or failed, and the technical group says so when the file has no
 *   `exif:` statement.
 * @returns The rows; the extension row only when the file has an extension.
 */
export function detailRows(
  details: FileDetails,
  versions: ObjectVersions,
  labels: LabelTable,
  locale?: string,
  history?: Pick<FileHistorySize, "versions" | "bytes">,
  extras: DetailExtras = {},
): PropertyRow[] {
  const { facts, technical } = extras;
  const all = facts?.imports ?? [];
  const imports = all.slice(0, MAX_IMPORT_ROWS);
  return [
    ...identityRows(details, labels, facts),
    sizeRow(details, labels, locale),
    ...(history ? [historySizeRow(history, labels, locale)] : []),
    ...digestRows(details, labels, facts?.sha256),
    ...imports.flatMap((entry, index) => importRows(entry, index, labels, locale)),
    {
      key: "object",
      label: labels.filePanelObject,
      value: shortIdentifier(details.object_id),
      copy: details.object_id,
      mono: true,
    },
    versionRow(details, versions, labels, locale),
    ...moreImportsRow(all.length, labels, locale),
    ...(technical ? technicalRows(technical, labels) : []),
  ];
}
