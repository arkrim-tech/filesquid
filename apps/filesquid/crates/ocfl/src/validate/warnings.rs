//! What the OCFL 1.1 inventory SHOULD do (W001, W004, W005, W007, W008,
//! W009). Warnings never make an object invalid.

use super::report::ValidationReport;
use crate::digest::DigestAlgorithm;
use crate::inventory::{Inventory, Version};

/// Report every inventory-level warning of the root inventory, located at
/// `at` (its storage key).
pub(crate) fn inventory_warnings(inventory: &Inventory, at: &str, report: &mut ValidationReport) {
    if inventory.head.is_zero_padded() {
        let message = "version directory names should not be zero-padded";
        report.add("W001", format!("{at}/head"), message);
    }
    algorithm_warning(inventory.digest_algorithm, at, report);
    if !is_uri(&inventory.id) {
        report.add("W005", format!("{at}/id"), "the object id should be a URI");
    }
    for (version, block) in &inventory.versions {
        version_warnings(block, &format!("{at}/versions/{version}"), report);
    }
}

/// W004: objects should use sha512 for content addressing. Reported for
/// the root inventory and for every version inventory.
pub(crate) fn algorithm_warning(
    algorithm: DigestAlgorithm,
    at: &str,
    report: &mut ValidationReport,
) {
    if algorithm != DigestAlgorithm::Sha512 {
        let message = "objects should use sha512 for content addressing";
        report.add("W004", format!("{at}/digestAlgorithm"), message);
    }
}

/// W007, W008 and W009 for one version block at `at`.
fn version_warnings(block: &Version, at: &str, report: &mut ValidationReport) {
    if block.message.is_none() || block.user.is_none() {
        let message = "a version should include message and user";
        report.add("W007", at.to_string(), message);
    }
    let Some(user) = &block.user else {
        return;
    };
    match &user.address {
        None => report.add(
            "W008",
            format!("{at}/user"),
            "a user should have an address",
        ),
        Some(address) if !is_uri(address) => {
            let message = "a user address should be a URI, such as mailto: or https:";
            report.add("W009", format!("{at}/user/address"), message);
        }
        Some(_) => {}
    }
}

/// Whether `text` is a URI by RFC 3986 section 3: a scheme (a letter, then
/// letters, digits, `+`, `-` or `.`), a colon, and a non-empty rest made of
/// URI characters and well-formed percent escapes. `[` and `]` are allowed
/// only around an IP-literal host in an authority (`//host`).
pub(crate) fn is_uri(text: &str) -> bool {
    let Some((scheme, rest)) = text.split_once(':') else {
        return false;
    };
    is_scheme(scheme) && !rest.is_empty() && is_hier_part(rest)
}

/// RFC 3986 `scheme`: a letter, then letters, digits, `+`, `-` or `.`.
fn is_scheme(scheme: &str) -> bool {
    let mut chars = scheme.chars();
    let starts_with_letter = chars.next().is_some_and(|c| c.is_ascii_alphabetic());
    starts_with_letter && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// Everything after the scheme's colon: an optional `//authority`, then a
/// path, query and fragment of URI characters.
fn is_hier_part(rest: &str) -> bool {
    let Some(after) = rest.strip_prefix("//") else {
        return is_uri_text(rest);
    };
    let end = after.find(['/', '?', '#']).unwrap_or(after.len());
    let (authority, tail) = after.split_at(end);
    is_authority(authority) && is_uri_text(tail)
}

/// RFC 3986 `authority`: optional `userinfo@`, then a host that is either
/// URI text or a bracketed IP literal, then an optional `:port`.
fn is_authority(authority: &str) -> bool {
    let (userinfo, host_port) = match authority.rsplit_once('@') {
        Some((userinfo, host_port)) => (userinfo, host_port),
        None => ("", authority),
    };
    let userinfo_ok = !userinfo.contains('@') && is_uri_text(userinfo);
    let Some(literal) = host_port.strip_prefix('[') else {
        return userinfo_ok && is_uri_text(host_port);
    };
    let Some((inside, port)) = literal.split_once(']') else {
        return false;
    };
    let inside_ok = !inside.is_empty()
        && inside
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".:-_~!$&'()*+,;=".contains(&b));
    let port_ok = port.is_empty()
        || port
            .strip_prefix(':')
            .is_some_and(|digits| digits.bytes().all(|b| b.is_ascii_digit()));
    userinfo_ok && inside_ok && port_ok
}

/// Whether `text` holds only RFC 3986 unreserved, `:/?#@` and sub-delim
/// characters, and `%` only in a two-hex-digit escape. `[` and `]` are
/// refused. The loop visits each byte at most once.
fn is_uri_text(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        let byte = bytes[at];
        if byte == b'%' {
            let escape = bytes.get(at + 1..at + 3);
            if !escape.is_some_and(|hex| hex.iter().all(u8::is_ascii_hexdigit)) {
                return false;
            }
            at += 3;
            continue;
        }
        if !(byte.is_ascii_alphanumeric() || b"-._~:/?#@!$&'()*+,;=".contains(&byte)) {
            return false;
        }
        at += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::is_uri;

    #[test]
    fn uris_need_a_scheme_and_a_rest() {
        for uri in [
            "ark:123/abc",
            "ark:/12345/bcd987",
            "urn:example:x",
            "https://orcid.org/0000-0000-0000-0000",
            "info:a/b",
            "mailto:a_person@example.org",
            "http://[2001:db8::1]:8080/a%20b?q=1#f",
            "https://user@example.org/",
        ] {
            assert!(is_uri(uri), "{uri}");
        }
        for text in ["not_a_uri", "1 Wonky Way, WW", ":x", "a:", "1a:b", "a b:c"] {
            assert!(!is_uri(text), "{text}");
        }
    }

    #[test]
    fn malformed_characters_and_escapes_are_not_uris() {
        for text in [
            "urn:^",
            "https://exa[mple",
            "https://example.org/a]b",
            "urn:a%2",
            "urn:a%zz",
            "http://[::1",
            "http://[::1]x",
            "urn:a\u{e9}",
        ] {
            assert!(!is_uri(text), "{text}");
        }
    }
}
