//! Display names (owner requirement, 2026-10-19): users always see a file's original name,
//! never the sanitized name it is stored under.
//!
//! The desktop stores each original under a sanitized, universally unique name and keeps the
//! owner's original file name beside it, in the file record (DE-0.3, API-S-026,
//! [ADR 038](../../../../../../resources/architecture/038-import-file-record.md)). The host
//! that holds those records injects them as [`DisplayNames`]; without one, a file's display
//! name is its stored name. Every listing and details answer carries the display name, and
//! every content answer names the file by it in `Content-Disposition` (RFC 6266), with an
//! ASCII `filename` fallback and the exact name as RFC 5987 `filename*` UTF-8. Pure, except
//! for the injected lookup.

use std::future::Future;
use std::pin::Pin;

/// One stored file whose display name is asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoredFile<'a> {
    /// The library.
    pub library_id: &'a str,
    /// The OCFL object.
    pub object_id: &'a str,
    /// The version (`v1`, `v2`, ...).
    pub version: &'a str,
    /// The logical path the file is stored under.
    pub path: &'a str,
}

/// The future of one display-name lookup.
pub type NameFuture<'a> = Pin<Box<dyn Future<Output = Option<String>> + Send + 'a>>;

/// Where the owner's original file names come from: the file records the host keeps.
pub trait DisplayNames: Send + Sync {
    /// The owner's original name of `file`, or `None` when no record holds one (the stored
    /// name is shown then). A record that cannot be read is answered as `None` and logged by
    /// the implementation; the answer never fails because of it.
    fn original_name<'a>(&'a self, file: StoredFile<'a>) -> NameFuture<'a>;
}

/// Longest display name kept, in bytes; a longer one is cut at a character boundary.
pub const MAX_NAME_BYTES: usize = 1_024;

/// `name` safe to show and to send: path separators become `_`, control characters are
/// dropped, surrounding spaces are trimmed, and at most [`MAX_NAME_BYTES`] bytes are kept.
/// `None` when nothing is left.
pub fn safe_name(name: &str) -> Option<String> {
    let cleaned: String = name
        .chars()
        .filter(|c| !c.is_control())
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim();
    let mut end = trimmed.len().min(MAX_NAME_BYTES);
    while !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    let kept = &trimmed[..end];
    (!kept.is_empty()).then(|| kept.to_string())
}

/// The last segment of a logical path: the stored name.
pub fn stored_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// How a content answer offers the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disposition {
    /// Shown in place (previews, media elements).
    Inline,
    /// Saved as a download.
    Attachment,
}

/// The `Content-Disposition` value naming the file `name` (RFC 6266): an ASCII `filename`
/// fallback in which every other character, `"`, `\` and `%` become `_`, and the exact name
/// as `filename*=UTF-8''<percent-encoded>` (RFC 5987). `name` is passed through
/// [`safe_name`] first.
pub fn content_disposition(kind: Disposition, name: &str) -> String {
    let name = safe_name(name).unwrap_or_else(|| "file".to_string());
    let kind = match kind {
        Disposition::Inline => "inline",
        Disposition::Attachment => "attachment",
    };
    format!(
        "{kind}; filename=\"{}\"; filename*=UTF-8''{}",
        ascii_fallback(&name),
        percent_encode(&name)
    )
}

/// `name` in printable ASCII for the quoted `filename` parameter.
fn ascii_fallback(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '"' | '\\' | '%' => '_',
            c if c.is_ascii() && !c.is_ascii_control() => c,
            _ => '_',
        })
        .collect()
}

/// `name` as RFC 5987 `value-chars`: every byte outside `attr-char` percent-encoded.
fn percent_encode(name: &str) -> String {
    let mut out = String::with_capacity(name.len() * 3);
    for byte in name.bytes() {
        let attr_char = byte.is_ascii_alphanumeric() || b"!#$&+-.^_`|~".contains(&byte);
        if attr_char {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_non_ascii_name_with_spaces_gets_a_fallback_and_an_exact_utf8_form() {
        let header = content_disposition(Disposition::Attachment, "Été à Nice 2023.jpg");
        assert_eq!(
            header,
            "attachment; filename=\"_t_ _ Nice 2023.jpg\"; \
             filename*=UTF-8''%C3%89t%C3%A9%20%C3%A0%20Nice%202023.jpg"
        );
    }

    #[test]
    fn separators_controls_and_quotes_never_reach_the_header() {
        let header = content_disposition(Disposition::Inline, " ../evil\n\"x\"\\%.jpg ");
        assert!(!header.contains('/') && !header.contains('\n'));
        assert!(header.starts_with("inline; filename=\".._evil_x___.jpg\"; filename*=UTF-8''"));
        assert_eq!(safe_name("\u{7}\t "), None);
        assert_eq!(
            content_disposition(Disposition::Inline, "\n"),
            "inline; filename=\"file\"; filename*=UTF-8''file"
        );
    }

    #[test]
    fn long_names_are_cut_at_a_character_boundary() {
        let long = "é".repeat(MAX_NAME_BYTES);
        let kept = safe_name(&long).expect("kept");
        assert!(kept.len() <= MAX_NAME_BYTES);
        assert!(kept.chars().all(|c| c == 'é'));
    }
}
