/**
 * The model behind `<fs-metadata-panel>`, the Description tab (UI-F2.4; UI-S-022, UI-S-031
 * mockups 28 and 29; API-S-035; MD-S-002, MD-S-011): the metadata-route types from the
 * generated API contract, and the pure rules over them: which statements the tab lists, their
 * rows keyed by CURIE with the owner or extracted tone, the file-scoped `/terms/` names the
 * Add property dialog searches, the candidate groups in the API's add order, and the value,
 * term-name and custom-key checks the dialog applies before it sends anything. Nothing here
 * fetches or touches the DOM.
 *
 * Every list is bounded (AGENTS.md section 22) by the `MAX_*` constants.
 */
import type { components } from "../api/schema";
import { formatLabel, type LabelKey, type LabelTable } from "../theme/labels";
import type { PropertyRow } from "./details";

/** The statements of one file at one version, generated from the API contract. */
export type FileStatements = components["schemas"]["FileStatements"];
/** One populated statement of a file's metadata record. */
export type MetadataStatement = components["schemas"]["MetadataStatement"];
/** Where a statement's value came from: extracted from the file, or asserted by the owner. */
export type StatementSource = components["schemas"]["StatementSource"];
/** The one OCFL version a metadata edit published. */
export type MetadataCommit = components["schemas"]["MetadataCommit"];
/** The add order of one term name. */
export type TermCandidates = components["schemas"]["TermCandidates"];
/** One place a term name can go. */
export type TermCandidate = components["schemas"]["TermCandidate"];
/** A library's namespace registry. */
export type Namespaces = components["schemas"]["Namespaces"];
/** One registered namespace. */
export type NamespaceRegistration = components["schemas"]["NamespaceRegistration"];

/**
 * Prefixes of technical statements, read from the file: they are the File info tab's
 * "TECHNICAL · READ FROM THE FILE" rows (UI-S-031, UI-F2.15), not Description rows.
 */
export const TECHNICAL_PREFIXES: readonly string[] = ["exif"];
/** Most rows the tab lists. */
export const MAX_STATEMENT_ROWS = 500;
/** Longest statement value the set route takes, in UTF-8 bytes (API-S-035). */
export const MAX_STATEMENT_VALUE_BYTES = 65_536;
/** Longest term name the terms route takes, in UTF-8 bytes. */
export const MAX_TERM_NAME_BYTES = 128;
/** Most `/terms/` names the dialog's search lists. */
export const MAX_TERM_MATCHES = 8;

/**
 * The ISO 15836-2 `/terms/` properties a file may carry: the register's 55 names less the
 * three `accrual*` terms, which are refused on a file (MD-S-002). The register itself lives in
 * the metadata module; the API answers the add order of one exact name, so this list lets the
 * dialog match as the owner types.
 */
export const DCTERMS_FILE_TERMS: readonly string[] = [
  "abstract",
  "accessRights",
  "alternative",
  "audience",
  "available",
  "bibliographicCitation",
  "conformsTo",
  "contributor",
  "coverage",
  "created",
  "creator",
  "date",
  "dateAccepted",
  "dateCopyrighted",
  "dateSubmitted",
  "description",
  "educationLevel",
  "extent",
  "format",
  "hasFormat",
  "hasPart",
  "hasVersion",
  "identifier",
  "instructionalMethod",
  "isFormatOf",
  "isPartOf",
  "isReferencedBy",
  "isReplacedBy",
  "isRequiredBy",
  "issued",
  "isVersionOf",
  "language",
  "license",
  "mediator",
  "medium",
  "modified",
  "provenance",
  "publisher",
  "references",
  "relation",
  "replaces",
  "requires",
  "rights",
  "rightsHolder",
  "source",
  "spatial",
  "subject",
  "tableOfContents",
  "temporal",
  "title",
  "type",
  "valid",
];

/** A term name as the set route accepts it: letters, digits, `_`, `-` and `.`, not starting with a digit, `-` or `.`. */
const TERM_NAME = /^[A-Za-z_][A-Za-z0-9_.-]*$/;

/** UTF-8 length of a string. */
function byteLength(text: string): number {
  return new TextEncoder().encode(text).byteLength;
}

/**
 * Whether `name` is a bare term name the routes accept (API-S-035): letters, digits, `_`,
 * `-` and `.`, starting with a letter or `_`, 1 to 128 UTF-8 bytes. Pure.
 *
 * @param name The candidate.
 * @returns True when the API would take it.
 */
export function validTermName(name: string): boolean {
  return TERM_NAME.test(name) && byteLength(name) <= MAX_TERM_NAME_BYTES;
}

/** One row of the Description tab: a property row that knows its statement. */
export interface MetadataRow extends PropertyRow {
  /** `prefix:term`. */
  readonly curie: string;
  /** The namespace prefix. */
  readonly prefix: string;
  /** Where the value came from. */
  readonly source: StatementSource;
  /** True for an owner-asserted value, which the owner may edit or remove. */
  readonly editable: boolean;
}

/**
 * The statements the Description tab lists: every statement whose prefix is not technical,
 * in record order, at most {@link MAX_STATEMENT_ROWS}. Pure.
 *
 * @param statements The record's populated statements.
 * @returns The descriptive statements.
 */
export function descriptiveStatements(
  statements: readonly MetadataStatement[],
): MetadataStatement[] {
  return statements
    .filter((statement) => !TECHNICAL_PREFIXES.includes(statement.prefix))
    .slice(0, MAX_STATEMENT_ROWS);
}

/**
 * The rows of the Description tab: one per descriptive statement, keyed and labelled by its
 * CURIE (so the prefix is always shown), in the owner tone for an asserted value and the
 * extracted tone for a value read from the file, editable only when asserted. Pure.
 *
 * @param statements The record's populated statements.
 * @returns The rows, in record order.
 */
export function statementRows(statements: readonly MetadataStatement[]): MetadataRow[] {
  return descriptiveStatements(statements).map((statement) => ({
    key: statement.curie,
    label: statement.curie,
    value: statement.value,
    labelStyle: statement.source === "asserted" ? "owner" : "extracted",
    curie: statement.curie,
    prefix: statement.prefix,
    source: statement.source,
    editable: statement.source === "asserted",
  }));
}

/**
 * The `/terms/` names that match what the owner typed: names starting with the text first,
 * then names containing it, case-insensitive, at most {@link MAX_TERM_MATCHES}. Pure.
 *
 * @param typed The text typed in the dialog's search.
 * @param terms The names searched; the file-scoped `/terms/` names by default.
 * @returns The matching names, in that order.
 */
export function matchingTerms(
  typed: string,
  terms: readonly string[] = DCTERMS_FILE_TERMS,
): string[] {
  const needle = typed.trim().toLocaleLowerCase();
  if (needle === "") return [];
  const lower = terms.map((term) => [term, term.toLocaleLowerCase()] as const);
  const starts = lower.filter(([, l]) => l.startsWith(needle)).map(([term]) => term);
  const contains = lower
    .filter(([, l]) => !l.startsWith(needle) && l.includes(needle))
    .map(([term]) => term);
  return [...starts, ...contains].slice(0, MAX_TERM_MATCHES);
}

/** One registered namespace that accepts the typed name. */
export interface RegisteredCandidate {
  /** `prefix:name`. */
  readonly curie: string;
  /** The namespace prefix. */
  readonly prefix: string;
  /** An extension namespace or an owner-declared custom namespace. */
  readonly kind: "extension" | "custom";
}

/** The dialog's candidate groups, in the API's add order. */
export interface CandidateGroups {
  /** `dcterms:` CURIEs, the `/terms/` register first. */
  readonly dcterms: string[];
  /** The Schema.org suggestion, when the API offered one. */
  readonly schema: string | undefined;
  /** Each registered namespace that accepts the name. */
  readonly registered: RegisteredCandidate[];
}

/**
 * Group the candidates for what the owner typed, in the API's add order: the `/terms/`
 * names matched locally (plus any `dcterms:` candidate the API answered that the local list
 * lacks), then the Schema.org suggestion, then each registered namespace. An API answer for
 * another name is ignored, so a late answer never shows under the wrong text. Pure.
 *
 * @param typed The text typed in the dialog's search.
 * @param answer The API's add order for the typed name, when it has answered.
 * @param locals The locally matched `/terms/` names.
 * @returns The groups.
 */
export function groupCandidates(
  typed: string,
  answer: TermCandidates | undefined,
  locals: readonly string[],
): CandidateGroups {
  const candidates = answer && answer.name === typed.trim() ? answer.candidates : [];
  const dcterms = locals.map((name) => `dcterms:${name}`);
  for (const candidate of candidates) {
    if (candidate.kind === "dcterms" && !dcterms.includes(candidate.curie)) {
      dcterms.unshift(candidate.curie);
    }
  }
  const schema = candidates.find((candidate) => candidate.kind === "schema")?.curie;
  const registered = candidates
    .filter((candidate) => candidate.kind === "extension" || candidate.kind === "custom")
    .map((candidate) => ({
      curie: candidate.curie,
      prefix: candidate.curie.slice(0, candidate.curie.indexOf(":")),
      kind: candidate.kind as "extension" | "custom",
    }));
  return { dcterms, schema, registered };
}

/**
 * Check a typed value as the set route does: the trimmed text is sent, so it is 1 to 65,536
 * UTF-8 bytes after trimming. Pure.
 *
 * @param raw The value as typed.
 * @returns `empty` or `too-long` when the route would refuse it, otherwise undefined.
 */
export function valueProblem(raw: string): "empty" | "too-long" | undefined {
  const value = raw.trim();
  if (value === "") return "empty";
  return byteLength(value) > MAX_STATEMENT_VALUE_BYTES ? "too-long" : undefined;
}

/**
 * The owner-declared custom namespaces of a registry. Pure.
 *
 * @param namespaces The library's registrations.
 * @returns Those of kind `custom`, in registration order.
 */
export function customNamespaces(
  namespaces: readonly NamespaceRegistration[],
): NamespaceRegistration[] {
  return namespaces.filter((namespace) => namespace.kind === "custom");
}

/**
 * Check a key typed for a custom namespace: `prefix:name` with a valid name, where the
 * prefix is a declared custom namespace. Pure.
 *
 * @param key The key as typed.
 * @param namespaces The library's registrations.
 * @returns `format` for anything but `prefix:name`, `prefix` for an unknown or non-custom
 *   prefix, otherwise undefined.
 */
export function customKeyProblem(
  key: string,
  namespaces: readonly NamespaceRegistration[],
): "format" | "prefix" | undefined {
  const text = key.trim();
  const colon = text.indexOf(":");
  if (colon <= 0) return "format";
  const prefix = text.slice(0, colon);
  const name = text.slice(colon + 1);
  if (!validTermName(name)) return "format";
  const custom = customNamespaces(namespaces).some((namespace) => namespace.prefix === prefix);
  return custom ? undefined : "prefix";
}

/**
 * The description of a `/terms/` property from the label table (`dctermsTitle` for
 * `title`). Pure.
 *
 * @param term The bare term name.
 * @param labels The active label table.
 * @returns The description, or undefined for a name the table does not describe.
 */
export function dctermsDescription(term: string, labels: LabelTable): string | undefined {
  const key = `dcterms${term.charAt(0).toUpperCase()}${term.slice(1)}` as LabelKey;
  return Object.hasOwn(labels, key) ? labels[key] : undefined;
}

/**
 * The count eyebrow of the Description tab: "Dublin Core and Schema.org · {count}", "… ·
 * {shown} of {count}" under a filter, or "0 properties" for an empty record. Pure.
 *
 * @param labels The active label table.
 * @param shown The rows that match the filter.
 * @param total Every row.
 * @param locale BCP 47 tag for numbers.
 * @returns The eyebrow text.
 */
export function metadataCountText(
  labels: LabelTable,
  shown: number,
  total: number,
  locale?: string,
): string {
  if (total === 0) return labels.metadataCountEmpty;
  const number = (n: number) => new Intl.NumberFormat(locale).format(n);
  if (shown === total) return formatLabel(labels.metadataCount, { count: number(total) });
  return formatLabel(labels.metadataCountFiltered, {
    shown: number(shown),
    count: number(total),
  });
}
