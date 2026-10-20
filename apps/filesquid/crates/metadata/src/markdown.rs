//! Canonical `metadata.md`: YAML front matter plus optional Markdown body.

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
    append_ns(&mut out, "schema", MetadataNamespace::SchemaOrg, record);
    out.push_str(tail);
    out.push_str("---\n");
    if !record.body.trim().is_empty() {
        out.push('\n');
        out.push_str(record.body.trim_end());
        out.push('\n');
    }
    out
}

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
            ns = match name.trim() {
                "dcterms" => Some(MetadataNamespace::Dcterms),
                "exif" => Some(MetadataNamespace::Exif),
                "schema" => Some(MetadataNamespace::SchemaOrg),
                _ => None,
            };
            continue;
        }
        let Some(namespace) = ns else {
            continue;
        };
        let trimmed = line.trim();
        let Some((term, raw)) = trimmed.split_once(':') else {
            continue;
        };
        let value = unquote(raw.trim());
        let _ = record.set(namespace, term.trim(), value);
    }
    record
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
