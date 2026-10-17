//! Bearer secrets: random generation, constant-time comparison and the desktop per-launch
//! token. Nothing here performs I/O beyond reading the operating system's random source.

/// Random bytes in every generated token (256 bits).
pub const TOKEN_BYTES: usize = 32;

/// Generate a random 256-bit token as 64 lowercase hex characters.
///
/// Returns an error message when the operating system's random source fails.
pub fn generate_token() -> Result<String, String> {
    let mut bytes = [0u8; TOKEN_BYTES];
    getrandom::fill(&mut bytes).map_err(|e| format!("random source failed: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Compare without an early exit, so timing does not reveal how much of a token matched.
/// Different lengths compare unequal (lengths are not secret: every token is 64 characters).
pub(crate) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// The desktop host's per-launch token (ADR 030 Decision 5).
///
/// The host generates one at every launch, keeps it in memory only, installs it with
/// [`crate::auth::Authoriser::desktop_owner`] and hands it only to its own webviews. A token
/// from an earlier launch is worthless. `Debug` output never shows the secret.
#[derive(Clone)]
pub struct LaunchToken {
    secret: String,
}

impl LaunchToken {
    /// Generate a fresh 256-bit token. Fails only when the random source fails.
    pub fn generate() -> Result<Self, String> {
        Ok(Self {
            secret: generate_token()?,
        })
    }

    /// The secret, for the host to hand to its own webviews and nobody else.
    pub fn secret(&self) -> &str {
        &self.secret
    }
}

impl std::fmt::Debug for LaunchToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LaunchToken([redacted])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_slices_compare_equal_and_every_difference_is_found() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(constant_time_eq(b"", b""));
        for other in [&b"abd"[..], b"xbc", b"ab", b"abcd", b""] {
            assert!(!constant_time_eq(b"abc", other), "{other:?}");
        }
    }
}
