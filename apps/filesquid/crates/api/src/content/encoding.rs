//! The text form of a content handle: base64url without padding (RFC 4648 section 5). Pure.

use super::{HANDLE_BYTES, HANDLE_CHARS};

/// The base64url alphabet.
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// `bytes` in base64url without padding: 43 characters for 32 bytes.
pub(super) fn encode(bytes: &[u8; HANDLE_BYTES]) -> String {
    let mut out = String::with_capacity(HANDLE_CHARS);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
        let chars = chunk.len() + 1;
        for i in 0..chars {
            let index = (n >> (18 - 6 * i)) & 0x3f;
            out.push(char::from(ALPHABET[index as usize]));
        }
    }
    out
}

/// True when `text` has a handle's shape: exactly [`HANDLE_CHARS`] base64url characters.
/// Says nothing about whether the handle exists.
pub(super) fn well_formed(text: &str) -> bool {
    text.len() == HANDLE_CHARS
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thirty_two_bytes_become_43_characters_matching_rfc_4648() {
        let zeros = encode(&[0u8; HANDLE_BYTES]);
        assert_eq!(zeros, "A".repeat(HANDLE_CHARS));
        let ones = encode(&[0xff; HANDLE_BYTES]);
        assert_eq!(ones, format!("{}8", "_".repeat(HANDLE_CHARS - 1)));
        let mut bytes = [0u8; HANDLE_BYTES];
        bytes[..6].copy_from_slice(b"foobar");
        // RFC 4648 section 10: BASE64("foobar") = "Zm9vYmFy".
        assert!(encode(&bytes).starts_with("Zm9vYmFy"));
        assert!(well_formed(&encode(&bytes)));
    }

    #[test]
    fn only_43_base64url_characters_are_well_formed() {
        assert!(well_formed(&"a".repeat(43)));
        for text in ["", "abc", &"a".repeat(44), &format!("{}+", "a".repeat(42))] {
            assert!(!well_formed(text), "{text:?}");
        }
    }
}
