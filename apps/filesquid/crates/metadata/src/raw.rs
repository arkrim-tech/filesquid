//! Exact bytes of a name or value as the source reported them.
//!
//! The import record stores names and attribute values that are valid UTF-8
//! as plain YAML/JSON strings and any other bytes as base64 with
//! `encoding: base64` beside them, so binary data round-trips exactly
//! ([ADR 038](../../../../resources/architecture/038-import-file-record.md)
//! Decision 3). The base64 here is RFC 4648 standard, padded; it is written
//! by hand so the crate takes no dependency for 40 lines.

use std::fmt;

use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};

/// The base64 alphabet of RFC 4648 section 4.
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// The `encoding` value that marks base64 bytes.
const BASE64: &str = "base64";

/// Bytes exactly as reported: text when they are UTF-8, base64 otherwise.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Raw(Vec<u8>);

impl Raw {
    /// Wrap `bytes` as reported.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(bytes.to_vec())
    }

    /// The reported bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// The bytes as text, when they are valid UTF-8.
    pub fn as_text(&self) -> Option<&str> {
        std::str::from_utf8(&self.0).ok()
    }

    /// The bytes as text for display, replacing invalid sequences with
    /// U+FFFD. The stored bytes stay exact; this is a view, never an identity.
    pub fn display_lossy(&self) -> String {
        String::from_utf8_lossy(&self.0).into_owned()
    }
}

impl Serialize for Raw {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if let Some(text) = self.as_text() {
            return serializer.serialize_str(text);
        }
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("encoding", BASE64)?;
        map.serialize_entry("value", &encode(&self.0))?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for Raw {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(RawVisitor)
    }
}

/// Reads a string or an `{encoding, value}` map.
struct RawVisitor;

impl<'de> Visitor<'de> for RawVisitor {
    type Value = Raw;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a string or an {encoding: base64, value} map")
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<Raw, E> {
        Ok(Raw(text.as_bytes().to_vec()))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Raw, A::Error> {
        let mut encoding: Option<String> = None;
        let mut value: Option<String> = None;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "encoding" if encoding.is_none() => encoding = Some(map.next_value()?),
                "value" if value.is_none() => value = Some(map.next_value()?),
                "encoding" => return Err(de::Error::duplicate_field("encoding")),
                "value" => return Err(de::Error::duplicate_field("value")),
                _ => return Err(de::Error::unknown_field(&key, &["encoding", "value"])),
            }
        }
        if encoding.as_deref() != Some(BASE64) {
            return Err(de::Error::custom("encoding must be base64"));
        }
        let value = value.ok_or_else(|| de::Error::missing_field("value"))?;
        decode(&value)
            .map(Raw)
            .ok_or_else(|| de::Error::custom("invalid base64"))
    }
}

/// Encode `bytes` as padded standard base64.
fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The value of one base64 character, or `None` outside the alphabet.
fn sextet(c: u8) -> Option<u32> {
    ALPHABET.iter().position(|a| *a == c).map(|p| p as u32)
}

/// Decode padded standard base64; `None` for any other input.
fn decode(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    let last = bytes.len() / 4;
    for (index, quad) in bytes.chunks(4).enumerate() {
        let pad = quad.iter().rev().take_while(|c| **c == b'=').count();
        if pad > 2 || (pad > 0 && index + 1 != last) {
            return None;
        }
        let mut n = 0u32;
        for (i, c) in quad[..4 - pad].iter().enumerate() {
            n |= sextet(*c)? << (18 - 6 * i);
        }
        let all = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&all[..3 - pad]);
    }
    Some(out)
}
