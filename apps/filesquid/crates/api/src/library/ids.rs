//! UUID version 4 identifiers for new libraries, collections and virtual collections
//! (CORE-S-001, [ADR 021](../../../../../../resources/architecture/021-uuid-v4-identifiers.md)).
//!
//! Identifiers are minted from the operating system's CSPRNG through `getrandom` and written
//! in the canonical lowercase 8-4-4-4-12 form. They encode no date and no path.

use super::LibraryError;

/// Length of a canonical UUID string.
const UUID_LEN: usize = 36;

/// Byte offsets of the hyphens in a canonical UUID string.
const HYPHENS: [usize; 4] = [8, 13, 18, 23];

/// Mint a new UUID version 4 from the operating system's CSPRNG (an I/O call to the OS
/// random source; no storage I/O).
///
/// # Errors
///
/// Returns [`LibraryError::Unavailable`] when the OS random source fails.
pub fn mint_uuid_v4() -> Result<String, LibraryError> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|err| {
        tracing::error!(error_code = "unavailable", detail = %err, "OS random source failed");
        LibraryError::Unavailable
    })?;
    Ok(format_uuid_v4(bytes))
}

/// Set the version (4) and variant (RFC 9562) bits of `bytes` and format them canonically.
fn format_uuid_v4(mut bytes: [u8; 16]) -> String {
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// True when `value` is a canonical lowercase UUID version 4 with the RFC 9562 variant.
pub fn is_uuid_v4(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == UUID_LEN
        && bytes.iter().enumerate().all(|(i, b)| uuid_char_ok(i, *b))
        && bytes[14] == b'4'
        && matches!(bytes[19], b'8' | b'9' | b'a' | b'b')
}

/// Whether byte `b` may stand at offset `i` of a canonical UUID.
fn uuid_char_ok(i: usize, b: u8) -> bool {
    if HYPHENS.contains(&i) {
        return b == b'-';
    }
    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
}
