//! Canonical `metadata.md`: YAML front matter plus optional Markdown body.

use crate::namespace::is_extension_prefix;
use crate::record::{Record, Statement};
use crate::MetadataNamespace;

/// Render a record as Obsidian-style Markdown with YAML front matter.
pub fn to_markdown(record: &Record) -> String {
    render(record, "", "")
}

/// Render `record` with `head` before and `tail` after its namespace maps in
/// the front matter. The per-file record puts its header and `import` list
/// there ([`FileRecord`](crate::file_record::FileRecord)); both are whole
/// lines ending in a line feed, or empty.
pub(crate) fn render(record: &Record, head: &str, tail: &str) -> String {
    let mut out = String::from("---\n");
    out.push_str(head);
    append_ns(&mut out, "dcterms", MetadataNamespace::Dcterms, record);
    append_ns(&mut out, "exif", MetadataNamespace::Exif, record);
    // A file record's own `schema:` identifier takes the key, so its
    // Schema.org statements move to `schemaorg:`.
    let schema_key = if head.is_empty() {
        "schema"
    } else {
        "schemaorg"
    };
    append_ns(&mut out, schema_key, MetadataNamespace::SchemaOrg, record);
    for prefix in extension_prefixes(record) {
        let namespace = MetadataNamespace::Extension(prefix.to_string());
        append_ns(&mut out, prefix, namespace, record);
    }
    out.push_str(tail);
    out.push_str("---\n");
    if !record.body.trim().is_empty() {
        out.push('\n');
        out.push_str(record.body.trim_end());
        out.push('\n');
    }
    out
}

/// The extension prefixes of `record`'s statements, each once, in the order
/// they first appear (MD-S-012: one YAML map per extension namespace).
fn extension_prefixes(record: &Record) -> Vec<&str> {
    let mut prefixes: Vec<&str> = Vec::new();
    for statement in &record.statements {
        if let MetadataNamespace::Extension(prefix) = &statement.namespace {
            if !prefixes.contains(&prefix.as_str()) {
                prefixes.push(prefix);
            }
        }
    }
    prefixes
}

/// Append the YAML map `key:` of the statements in namespace `ns`, if any.
fn append_ns(out: &mut String, key: &str, ns: MetadataNamespace, record: &Record) {
    let rows: Vec<&Statement> = record
        .statements
        .iter()
        .filter(|s| s.namespace == ns)
        .collect();
    if rows.is_empty() {
        return;
    }
    out.push_str(key);
    out.push_str(":\n");
    for row in rows {
        out.push_str("  ");
        out.push_str(&row.term);
        out.push_str(": ");
        out.push_str(&yaml_quote(&row.value));
        out.push('\n');
    }
}

/// Quote a YAML scalar when plain style would lose or misread it.
///
/// Quoted values escape `\`, `"`, line feeds, and carriage returns as `\\`,
/// `\"`, `\n`, and `\r`, so each statement stays on one line of `metadata.md`.
fn yaml_quote(value: &str) -> String {
    let needs_quotes = value.is_empty()
        || value.starts_with(' ')
        || value.contains([':', '#', '"', '\\', '\n', '\r']);
    if !needs_quotes {
        return value.to_string();
    }
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r");
    format!("\"{escaped}\"")
}

/// Parse YAML front matter produced by [`to_markdown`].
pub fn from_markdown(text: &str) -> Record {
    let text = text.trim_start();
    if !text.starts_with("---") {
        return Record {
            body: text.to_string(),
            ..Record::default()
        };
    }
    let rest = text.trim_start_matches("---").trim_start_matches('\n');
    let Some((yaml, body)) = rest.split_once("\n---") else {
        return Record::default();
    };
    let mut record = Record {
        body: body.trim().to_string(),
        statements: Vec::new(),
    };
    let mut ns = None;
    for line in yaml.lines() {
        if let Some(name) = line.strip_suffix(':').filter(|l| !l.starts_with(' ')) {
            ns = map_namespace(name.trim());
            continue;
        }
        // A top-level line that opens no map (a file-record header line)
        // ends the map before it; only indented lines are its statements.
        if !line.starts_with(' ') {
            ns = None;
            continue;
        }
        let Some(namespace) = &ns else {
            continue;
        };
        let trimmed = line.trim();
        let Some((term, raw)) = trimmed.split_once(':') else {
            continue;
        };
        read_statement(&mut record, namespace, term.trim(), unquote(raw.trim()));
    }
    record
}

/// The namespace of the top-level map `key`: a built-in map, or an extension
/// map for any key that could be a registered prefix. The record is read
/// without the registry, so a map is kept as it was written; registration
/// is checked when a statement is authored.
fn map_namespace(key: &str) -> Option<MetadataNamespace> {
    match key {
        "dcterms" => Some(MetadataNamespace::Dcterms),
        "exif" => Some(MetadataNamespace::Exif),
        "schema" | "schemaorg" => Some(MetadataNamespace::SchemaOrg),
        other if is_extension_prefix(other) => {
            Some(MetadataNamespace::Extension(other.to_string()))
        }
        _ => None,
    }
}

/// Add one statement read from a record: built-in statements pass the
/// `/terms/` register as before (a refused term is not part of the record);
/// extension statements are kept as written.
fn read_statement(record: &mut Record, namespace: &MetadataNamespace, term: &str, value: String) {
    match namespace {
        MetadataNamespace::Extension(_) => record.insert(namespace.clone(), term, value),
        _ => {
            // A refused built-in term is not part of the record.
            let _ = record.set(namespace.clone(), term, value);
        }
    }
}

/// Strip YAML double quotes and decode the escapes written by [`yaml_quote`].
fn unquote(raw: &str) -> String {
    let quoted = raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"');
    if !quoted {
        return raw.to_string();
    }
    unescape(&raw[1..raw.len() - 1])
}

/// Decode `\n`, `\r`, `\"`, and `\\` in a single left-to-right pass.
///
/// Unknown escapes are kept verbatim. The loop is bounded by the input length.
fn unescape(inner: &str) -> String {
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}
