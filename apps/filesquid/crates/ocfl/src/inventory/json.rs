//! Strict reading of untrusted inventory JSON.
//!
//! [`read_json`] accepts only one RFC 8259 JSON value in UTF-8 with nothing
//! after it, refuses a JSON object that repeats a key (a plain map would keep
//! only the last value, so a forged second `head` or digest could hide), and
//! refuses nesting deeper than [`MAX_JSON_DEPTH`] while it parses, so hostile
//! nesting cannot exhaust the stack. The caller caps the input size first.

use std::fmt;

use serde::de::{self, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

use super::error::FormatError;

/// Deepest nesting of JSON objects and arrays the reader accepts. A valid
/// inventory needs five (root, `versions`, a version, its `state`, a path
/// list); anything deeper cannot be an inventory.
pub const MAX_JSON_DEPTH: usize = 8;

/// Parse `bytes` as one strict JSON value.
///
/// # Errors
///
/// [`FormatError::Invalid`] with E033 for invalid JSON or UTF-8, trailing
/// data, a repeated key or nesting deeper than [`MAX_JSON_DEPTH`].
pub(crate) fn read_json(bytes: &[u8]) -> Result<Value, FormatError> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = Strict { depth: 0 }
        .deserialize(&mut deserializer)
        .map_err(json_error)?;
    deserializer.end().map_err(json_error)?;
    Ok(value)
}

/// The E033 error for JSON the reader refuses.
fn json_error(err: serde_json::Error) -> FormatError {
    FormatError::invalid(
        "E033",
        "",
        format!("not a valid inventory JSON document: {err}"),
    )
}

/// A JSON value reader that tracks its nesting depth.
#[derive(Debug, Clone, Copy)]
struct Strict {
    depth: usize,
}

impl Strict {
    /// The reader for the values inside a container at this depth.
    fn enter<E: de::Error>(self) -> Result<Self, E> {
        if self.depth >= MAX_JSON_DEPTH {
            return Err(E::custom(format_args!(
                "JSON nesting deeper than {MAX_JSON_DEPTH} levels"
            )));
        }
        Ok(Self {
            depth: self.depth + 1,
        })
    }
}

impl<'de> DeserializeSeed<'de> for Strict {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Strict {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("not a finite number"))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    /// Collect an array. The loop ends with the input, whose size the caller
    /// caps.
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let inner = self.enter()?;
        let mut items = Vec::new();
        while let Some(item) = seq.next_element_seed(inner)? {
            items.push(item);
        }
        Ok(Value::Array(items))
    }

    /// Collect an object, refusing a repeated key. The loop ends with the
    /// input, whose size the caller caps.
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let inner = self.enter()?;
        let mut object = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if object.contains_key(&key) {
                // The key is not echoed: a hostile key may be very long.
                return Err(de::Error::custom("a JSON object repeats a key"));
            }
            let value = map.next_value_seed(inner)?;
            object.insert(key, value);
        }
        Ok(Value::Object(object))
    }
}
