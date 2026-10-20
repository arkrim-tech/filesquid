//! Minimal XMP interchange for `dcterms:` and registered extension
//! statements: sidecar read/write (MD-S-006, MD-S-012) and the reader
//! [`extract`](crate::extract) applies to an embedded XMP packet (MD-S-007).
//!
//! An extension namespace is written with its registered IRI and read back
//! by that IRI, whatever prefix a packet binds it to; it is never read as
//! `dcterms:`.

use crate::namespace::NamespaceRegistry;
use crate::record::Record;
use crate::{MetadataError, MetadataNamespace};

/// The `/terms/` namespace IRI.
const DCTERMS_IRI: &str = "http://purl.org/dc/terms/";

/// Most `xmlns:` declarations of one packet that are read.
const MAX_BINDINGS: usize = 1024;

/// Serialize populated `dcterms:` statements as a small XMP packet.
pub fn to_xmp(record: &Record) -> String {
    render(record, &[])
}

/// Serialize populated `dcterms:` and extension statements as a small XMP
/// packet, each extension prefix bound to its registered IRI.
///
/// # Errors
///
/// [`MetadataError::UnregisteredNamespace`] naming an extension prefix of
/// `record` that `registry` does not hold, or
/// [`MetadataError::UnknownExtensionTerm`] for a term the registration
/// refuses (every written term is a valid element name).
pub fn to_xmp_with(record: &Record, registry: &NamespaceRegistry) -> Result<String, MetadataError> {
    let mut bound: Vec<(&str, &str)> = Vec::new();
    for statement in &record.statements {
        let MetadataNamespace::Extension(prefix) = &statement.namespace else {
            continue;
        };
        registry.check(prefix, &statement.term)?;
        if bound.iter().any(|(p, _)| p == prefix) {
            continue;
        }
        let registration = registry
            .get(prefix)
            .ok_or_else(|| MetadataError::UnregisteredNamespace(prefix.clone()))?;
        bound.push((prefix, registration.iri()));
    }
    Ok(render(record, &bound))
}

/// The packet of `record`'s `dcterms:` statements and the statements of
/// each `(prefix, IRI)` of `bound`.
fn render(record: &Record, bound: &[(&str, &str)]) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n\
 <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n",
    );
    out.push_str("  <rdf:Description xmlns:dcterms=\"");
    out.push_str(DCTERMS_IRI);
    out.push('"');
    for (prefix, iri) in bound {
        out.push_str(&format!(" xmlns:{prefix}=\"{}\"", xml_escape(iri)));
    }
    out.push_str(">\n");
    append_elements(&mut out, record, &MetadataNamespace::Dcterms);
    for (prefix, _) in bound {
        append_elements(
            &mut out,
            record,
            &MetadataNamespace::Extension(prefix.to_string()),
        );
    }
    out.push_str("  </rdf:Description>\n </rdf:RDF>\n</x:xmpmeta>\n");
    out
}

/// Append one element per statement of `record` in `namespace`.
fn append_elements(out: &mut String, record: &Record, namespace: &MetadataNamespace) {
    let prefix = namespace.prefix();
    for s in record
        .statements
        .iter()
        .filter(|s| &s.namespace == namespace)
    {
        out.push_str(&format!(
            "   <{prefix}:{term}>{value}</{prefix}:{term}>\n",
            term = s.term,
            value = xml_escape(&s.value)
        ));
    }
}

/// Read `dcterms:` and `dc:` elements from a sidecar produced by [`to_xmp`]
/// or another XMP packet. Terms the `/terms/` register refuses are left out;
/// [`dcterms_pairs`] lists them all.
pub fn from_xmp(text: &str) -> Record {
    let mut record = Record::default();
    for (term, value) in dcterms_pairs(text) {
        // A refused term is not part of the sidecar's record.
        let _ = record.set(MetadataNamespace::Dcterms, &term, value);
    }
    record
}

/// Read `dcterms:`, `dc:` and registered extension elements from an XMP
/// packet. An extension namespace is found by its registered IRI under any
/// prefix the packet binds to it; elements of unregistered namespaces and
/// terms a registration or the register refuses are left out.
pub fn from_xmp_with(text: &str, registry: &NamespaceRegistry) -> Record {
    let mut record = from_xmp(text);
    let bindings = xmlns_bindings(text);
    for registration in registry.registrations() {
        let namespace = MetadataNamespace::Extension(registration.prefix().to_string());
        let packet_prefixes = bindings.iter().filter(|(name, iri)| {
            iri == registration.iri() && !matches!(name.as_str(), "dc" | "dcterms")
        });
        for (name, _) in packet_prefixes {
            let mut pairs = Vec::new();
            pull_tags(text, &format!("{name}:"), &mut pairs);
            for (term, value) in pairs {
                // A term the registration refuses is not part of the record.
                let _ = record.set_registered(registry, namespace.clone(), &term, value);
            }
        }
    }
    record
}

/// The `xmlns:<name>="<iri>"` declarations of a packet (single or double
/// quotes), at most [`MAX_BINDINGS`].
fn xmlns_bindings(text: &str) -> Vec<(String, String)> {
    let mut bindings = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("xmlns:") {
        if bindings.len() >= MAX_BINDINGS {
            break;
        }
        rest = &rest[start + "xmlns:".len()..];
        let Some((name, after)) = rest.split_once('=') else {
            break;
        };
        let Some(quote) = after.chars().next().filter(|c| matches!(c, '"' | '\'')) else {
            continue;
        };
        let Some((iri, _)) = after[1..].split_once(quote) else {
            break;
        };
        bindings.push((name.trim().to_string(), xml_unescape(iri)));
    }
    bindings
}

/// Every `dcterms:` and `dc:` element of an XMP packet as `(term, value)`,
/// with `dc:` names mapped onto `dcterms:` and unchecked against the
/// register, so a caller can count the terms it refuses. An `rdf:Alt`,
/// `rdf:Bag` or `rdf:Seq` value is its `rdf:li` items joined by `"; "`.
pub fn dcterms_pairs(text: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    pull_tags(text, "dcterms:", &mut pairs);
    let mut dc = Vec::new();
    pull_tags(text, "dc:", &mut dc);
    pairs.extend(dc.into_iter().map(|(term, value)| {
        let mapped = crate::map_dc_to_dcterms(&term).map_or(term, str::to_string);
        (mapped, value)
    }));
    pairs
}

/// Collect the `<prefix term>value</prefix term>` elements of `text`.
fn pull_tags(text: &str, prefix: &str, pairs: &mut Vec<(String, String)>) {
    let open = format!("<{prefix}");
    let mut rest = text;
    while let Some(start) = rest.find(&open) {
        rest = &rest[start + open.len()..];
        let Some(gt) = rest.find('>') else {
            break;
        };
        let raw = rest[..gt].trim();
        let term = raw.trim_end_matches('/');
        let skip = raw.ends_with('/') || term.contains(char::is_whitespace) || term.is_empty();
        rest = &rest[gt + 1..];
        if skip {
            continue;
        }
        let close = format!("</{prefix}{term}>");
        // An element without its close ends the scan, so a malformed packet
        // costs one pass rather than one rescan per unmatched opening.
        let Some(end) = rest.find(&close) else {
            break;
        };
        let value = element_text(&rest[..end]);
        pairs.push((term.to_string(), value));
        rest = &rest[end + close.len()..];
    }
}

/// The text of an element body: its `rdf:li` items joined by `"; "` when it
/// holds an RDF container, else the body itself, unescaped.
fn element_text(body: &str) -> String {
    let items: Vec<String> = body
        .split("<rdf:li")
        .skip(1)
        .filter_map(|item| {
            let (_, after) = item.split_once('>')?;
            let (text, _) = after.split_once("</rdf:li>")?;
            Some(xml_unescape(text.trim()))
        })
        .collect();
    if items.is_empty() {
        return xml_unescape(body);
    }
    items.join("; ")
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Decode XML character data: `<![CDATA[…]]>` sections are kept verbatim,
/// and the predefined entities (`&lt;`, `&gt;`, `&amp;`, `&quot;`, `&apos;`)
/// and decimal or hex character references are decoded. An unknown or
/// malformed reference is kept as written.
fn xml_unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(at) = rest.find(['&', '<']) {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        rest = match rest.strip_prefix("<![CDATA[") {
            Some(cdata) => push_cdata(cdata, &mut out),
            None if rest.starts_with('&') => push_reference(rest, &mut out),
            None => {
                out.push('<');
                &rest[1..]
            }
        };
    }
    out.push_str(rest);
    out
}

/// Append a CDATA section's text (`cdata` starts after `<![CDATA[`) and
/// return what follows its `]]>`; an unterminated section is kept as
/// written, opener included.
fn push_cdata<'a>(cdata: &'a str, out: &mut String) -> &'a str {
    match cdata.split_once("]]>") {
        Some((text, after)) => {
            out.push_str(text);
            after
        }
        None => {
            out.push_str("<![CDATA[");
            out.push_str(cdata);
            ""
        }
    }
}

/// Append the character a reference at the start of `text` (`&…;`) stands
/// for and return what follows it; an unknown one is appended as written.
fn push_reference<'a>(text: &'a str, out: &mut String) -> &'a str {
    let decoded = text
        .find(';')
        .filter(|end| *end <= 10)
        .and_then(|end| decode_reference(&text[1..end]).map(|c| (c, end)));
    match decoded {
        Some((c, end)) => {
            out.push(c);
            &text[end + 1..]
        }
        None => {
            out.push('&');
            &text[1..]
        }
    }
}

/// The character the reference name `name` (between `&` and `;`) stands for.
fn decode_reference(name: &str) -> Option<char> {
    let code = match name {
        "lt" => return Some('<'),
        "gt" => return Some('>'),
        "amp" => return Some('&'),
        "quot" => return Some('"'),
        "apos" => return Some('\''),
        _ => match name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => name.strip_prefix('#')?.parse().ok()?,
        },
    };
    char::from_u32(code)
}
