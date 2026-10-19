//! Minimal XMP interchange for dcterms statements: sidecar read/write
//! (MD-S-006) and the reader [`extract`](crate::extract) applies to an
//! embedded XMP packet (MD-S-007).

use crate::record::Record;
use crate::MetadataNamespace;

/// Serialize populated `dcterms:` statements as a small XMP packet.
pub fn to_xmp(record: &Record) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n\
 <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\
  <rdf:Description xmlns:dcterms=\"http://purl.org/dc/terms/\">\n",
    );
    for s in record
        .statements
        .iter()
        .filter(|s| s.namespace == MetadataNamespace::Dcterms)
    {
        out.push_str("   <dcterms:");
        out.push_str(&s.term);
        out.push('>');
        out.push_str(&xml_escape(&s.value));
        out.push_str("</dcterms:");
        out.push_str(&s.term);
        out.push_str(">\n");
    }
    out.push_str("  </rdf:Description>\n </rdf:RDF>\n</x:xmpmeta>\n");
    out
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

/// Every `dcterms:` and `dc:` element of an XMP packet as `(term, value)`,
/// with `dc:` names mapped onto `dcterms:` and unchecked against the
/// register, so a caller can count the terms it refuses. An `rdf:Alt`,
/// `rdf:Bag` or `rdf:Seq` value is its `rdf:li` items joined by `"; "`.
pub fn dcterms_pairs(text: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    pull_tags(text, "dcterms:", &mut pairs);
    pull_tags(text, "dc:", &mut pairs);
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
        let skip = raw.ends_with('/') || term.contains(' ') || term.is_empty();
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
        let term = crate::map_dc_to_dcterms(term).unwrap_or(term);
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
/// return what follows its `]]>`; an unterminated section is kept whole.
fn push_cdata<'a>(cdata: &'a str, out: &mut String) -> &'a str {
    match cdata.split_once("]]>") {
        Some((text, after)) => {
            out.push_str(text);
            after
        }
        None => {
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
