//! Display names (owner requirement, 2026-10-19): users always see a file's original name,
//! never the sanitized name it is stored under.
//!
//! The desktop stores each original under a sanitized, universally unique name and keeps the
//! owner's original file name beside it, in the file record (DE-0.3, API-S-026,
//! [ADR 038](../../../../../../resources/architecture/038-import-file-record.md)). The host
//! that holds those records injects them as [`DisplayNames`]; without one, a file's display
//! name is its stored name. Every listing and details answer carries the display name, and
//! every content answer names the file by it in `Content-Disposition` (RFC 6266), with an
//! ASCII `filename` fallback and the display name as RFC 5987 `filename*` UTF-8. The
//! display name is the original (or stored) name after [`safe_name`], the one safe
//! transformation API-S-005 defines. Pure, except
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

/// `name` safe to show and to send, by the one safe transformation of API-S-005 that
/// `filesquid-metadata`'s `safe_display_name` also applies: control characters are dropped,
/// `/` and `\` become `_`, surrounding spaces are trimmed, and at most [`MAX_NAME_BYTES`]
/// bytes are kept. `None` when nothing is left.
pub fn safe_name(name: &str) -> Option<String> {
    let cleaned: String = name
        .chars()
        .filter(|c| !c.is_control())
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect();
    let mut kept = String::new();
    for c in cleaned.trim_matches(' ').chars() {
        if kept.len() + c.len_utf8() > MAX_NAME_BYTES {
            break;
        }
        kept.push(c);
    }
    let kept = kept.trim_matches(' ');
    (!kept.is_empty()).then(|| kept.to_string())
}

/// The last segment of a logical path: the stored name, as stored; pass it through
/// [`safe_name`] before showing or sending it.
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

/// The `Content-Disposition` value naming the file `name` (RFC 6266, API-S-027): the display
/// name as `filename*=UTF-8''<percent-encoded>` (RFC 5987), and an ASCII `filename` fallback
/// that keeps printable ASCII and replaces every other character and every `"`, `\`, `/`,
/// `;` and control character with `_`. When the display name holds no ASCII letter or digit,
/// the fallback is `id_name` instead: the FileId plus extension (the stored name). `name` is
/// passed through [`safe_name`] first.
pub fn content_disposition(kind: Disposition, name: &str, id_name: &str) -> String {
    let name = safe_name(name).unwrap_or_else(|| "file".to_string());
    let kind = match kind {
        Disposition::Inline => "inline",
        Disposition::Attachment => "attachment",
    };
    let readable = name.chars().any(|c| c.is_ascii_alphanumeric());
    let fallback = match readable {
        true => ascii_fallback(&name),
        false => ascii_fallback(id_name),
    };
    format!(
        "{kind}; filename=\"{fallback}\"; filename*=UTF-8''{}",
        percent_encode(&name)
    )
}

/// `name` in printable ASCII for the quoted `filename` parameter; `file` when nothing is
/// left. Pure.
fn ascii_fallback(name: &str) -> String {
    let fallback: String = name
        .chars()
        .map(|c| match c {
            '"' | '\\' | '/' | ';' => '_',
            c if c.is_ascii() && !c.is_ascii_control() => c,
            _ => '_',
        })
        .collect();
    match fallback.is_empty() {
        true => "file".to_string(),
        false => fallback,
    }
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

    /// The FileId-plus-extension fallback the tests pass.
    const ID: &str = "7d1c2b9e-4f3a-4e61-8a0b-2c9d5e6f7a80.jpg";

    #[test]
    fn a_non_ascii_name_with_spaces_gets_a_fallback_and_an_exact_utf8_form() {
        let header = content_disposition(Disposition::Attachment, "Été à Nice 2023.jpg", ID);
        assert_eq!(
            header,
            "attachment; filename=\"_t_ _ Nice 2023.jpg\"; \
             filename*=UTF-8''%C3%89t%C3%A9%20%C3%A0%20Nice%202023.jpg"
        );
    }

    #[test]
    fn quotes_backslashes_slashes_semicolons_and_controls_never_reach_the_fallback() {
        let header = content_disposition(Disposition::Inline, " ../evil\n\"x\"\\;%.jpg ", ID);
        assert!(!header.contains('/') && !header.contains('\n'));
        assert!(
            header.starts_with("inline; filename=\".._evil_x___%.jpg\"; filename*=UTF-8''"),
            "{header}"
        );
        assert!(
            header.ends_with("filename*=UTF-8''.._evil%22x%22_%3B%25.jpg"),
            "{header}"
        );
        assert_eq!(safe_name("\u{7}\t "), None);
        assert_eq!(
            content_disposition(Disposition::Inline, "\n", ID),
            "inline; filename=\"file\"; filename*=UTF-8''file"
        );
    }

    #[test]
    fn a_name_without_an_ascii_letter_or_digit_falls_back_to_the_file_id() {
        let header = content_disposition(Disposition::Attachment, "写真 — 夏", ID);
        assert_eq!(
            header,
            format!(
                "attachment; filename=\"{ID}\"; \
                 filename*=UTF-8''%E5%86%99%E7%9C%9F%20%E2%80%94%20%E5%A4%8F"
            )
        );
        let ascii = content_disposition(Disposition::Attachment, "写真 2.jpg", ID);
        assert!(
            ascii.starts_with("attachment; filename=\"__ 2.jpg\""),
            "{ascii}"
        );
    }

    #[test]
    fn the_display_name_matches_the_metadata_modules_safe_display_name() {
        let long = "é".repeat(MAX_NAME_BYTES);
        let names = ["a\u{a0}", " spaced ", "tab\tname", "a/b\\c", long.as_str()];
        for name in names {
            let ours = safe_name(name).unwrap_or_else(|| "file".to_string());
            let theirs = filesquid_metadata::file_record::safe_display_name(name);
            assert_eq!(ours, theirs, "{name:?}");
        }
    }

    #[test]
    fn long_names_are_cut_at_a_character_boundary() {
        let long = "é".repeat(MAX_NAME_BYTES);
        let kept = safe_name(&long).expect("kept");
        assert!(kept.len() <= MAX_NAME_BYTES);
        assert!(kept.chars().all(|c| c == 'é'));
    }
}
