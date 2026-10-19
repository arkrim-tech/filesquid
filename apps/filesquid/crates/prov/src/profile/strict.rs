//! A bounded, strict JSON reader (PROV-S-004).
//!
//! `serde_json` accepts repeated object keys (the last one wins) and nests up
//! to 128 levels. This reader drives `serde_json`'s tokenizer with its own
//! visitor, so it refuses a repeated key in any object and any nesting
//! deeper than [`MAX_DEPTH`] as soon as it sees them, before any deeper
//! recursion. The size bound is checked before any byte is parsed.

use std::cell::Cell;
use std::fmt;

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

use super::{MAX_DEPTH, MAX_RECORD_BYTES};
use crate::error::ProvError;

/// Read `bytes` as one JSON value under the size, depth and key rules.
///
/// # Errors
/// [`ProvError::TooLarge`], [`ProvError::InvalidJson`],
/// [`ProvError::TooDeep`] or [`ProvError::DuplicateKey`].
pub(super) fn read(bytes: &[u8]) -> Result<Value, ProvError> {
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(ProvError::TooLarge);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| ProvError::InvalidJson)?;
    let failure = Cell::new(None);
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let seed = Strict {
        depth: 0,
        failure: &failure,
    };
    // A failure this reader detected wins over serde_json's own error text,
    // which may quote input and is therefore never passed on.
    let value = seed
        .deserialize(&mut deserializer)
        .map_err(|_| failure.get().unwrap_or(ProvError::InvalidJson))?;
    deserializer.end().map_err(|_| ProvError::InvalidJson)?;
    Ok(value)
}

/// Visitor state: the depth of the value being read and where to note a
/// refusal this reader detected.
#[derive(Clone, Copy)]
struct Strict<'a> {
    depth: usize,
    failure: &'a Cell<Option<ProvError>>,
}

impl Strict<'_> {
    /// Enter one array or object level, refusing levels beyond [`MAX_DEPTH`].
    fn enter<E: de::Error>(self) -> Result<Self, E> {
        let depth = self.depth + 1;
        if depth > MAX_DEPTH {
            self.failure.set(Some(ProvError::TooDeep));
            return Err(E::custom("nesting limit"));
        }
        Ok(Self { depth, ..self })
    }
}

impl<'de> DeserializeSeed<'de> for Strict<'_> {
    type Value = Value;

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Strict<'_> {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON value")
    }

    fn visit_bool<E>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }

    fn visit_i64<E>(self, v: i64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }

    fn visit_u64<E>(self, v: u64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> {
        Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| E::custom("number out of range"))
    }

    fn visit_str<E>(self, v: &str) -> Result<Value, E> {
        Ok(Value::String(v.to_string()))
    }

    fn visit_string<E>(self, v: String) -> Result<Value, E> {
        Ok(Value::String(v))
    }

    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let inner = self.enter()?;
        let mut items = Vec::new();
        // Bounded by the input: at most MAX_RECORD_BYTES elements exist.
        while let Some(item) = seq.next_element_seed(inner)? {
            items.push(item);
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let inner = self.enter()?;
        let mut members = Map::new();
        // Bounded by the input: at most MAX_RECORD_BYTES members exist.
        while let Some(key) = map.next_key::<String>()? {
            if members.contains_key(&key) {
                self.failure.set(Some(ProvError::DuplicateKey));
                return Err(de::Error::custom("repeated key"));
            }
            let value = map.next_value_seed(inner)?;
            members.insert(key, value);
        }
        Ok(Value::Object(members))
    }
}
