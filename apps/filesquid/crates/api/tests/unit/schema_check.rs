//! Strict structural checker for the subset of JSON Schema 2020-12 that the FileSquid
//! OpenAPI 3.1 contract uses (T-048 drift test support).
//!
//! The contract's convention is that documented object properties are exhaustive: a
//! serialized Rust value with a field the schema does not list fails, unless the schema
//! sets `additionalProperties`. [`unsupported_keywords`] makes sure the contract never uses
//! a keyword this checker would silently ignore.

use serde_json::Value;

/// Schema keywords the checker understands. Annotations (`description`, `format`) are
/// accepted and not validated.
pub const KEYWORDS: &[&str] = &[
    "$ref",
    "additionalProperties",
    "anyOf",
    "const",
    "description",
    "enum",
    "format",
    "items",
    "maximum",
    "minimum",
    "maxLength",
    "minLength",
    "properties",
    "required",
    "type",
];

/// Maximum `$ref` hops and nesting depth; contract schemas are far shallower.
const MAX_DEPTH: usize = 32;

/// Checks JSON values against schemas of one OpenAPI document.
pub struct Checker<'a> {
    doc: &'a Value,
}

impl<'a> Checker<'a> {
    /// Create a checker that resolves `$ref`s against `doc`.
    pub fn new(doc: &'a Value) -> Self {
        Self { doc }
    }

    /// Follow `#/...` references until a schema without `$ref` is reached.
    pub fn resolve(&self, schema: &'a Value) -> Result<&'a Value, String> {
        let mut current = schema;
        for _ in 0..MAX_DEPTH {
            let Some(reference) = current.get("$ref").and_then(Value::as_str) else {
                return Ok(current);
            };
            let pointer = reference
                .strip_prefix('#')
                .ok_or_else(|| format!("external $ref {reference}"))?;
            current = self
                .doc
                .pointer(pointer)
                .ok_or_else(|| format!("unresolved $ref {reference}"))?;
        }
        Err("$ref chain too long".into())
    }

    /// Check `value` against `schema`; the error names the JSON path that failed.
    pub fn check(&self, value: &Value, schema: &'a Value, at: &str) -> Result<(), String> {
        self.check_depth(value, schema, at, 0)
    }

    fn check_depth(
        &self,
        value: &Value,
        schema: &'a Value,
        at: &str,
        depth: usize,
    ) -> Result<(), String> {
        if depth > MAX_DEPTH {
            return Err(format!("{at}: nesting deeper than {MAX_DEPTH}"));
        }
        let schema = self.resolve(schema)?;
        if let Some(branches) = schema.get("anyOf").and_then(Value::as_array) {
            return self.check_any(value, branches, at, depth);
        }
        check_const_and_enum(value, schema, at)?;
        check_type(value, schema, at)?;
        check_bounds(value, schema, at)?;
        match value {
            Value::Object(_) => self.check_object(value, schema, at, depth),
            Value::Array(items) => self.check_items(items, schema, at, depth),
            _ => Ok(()),
        }
    }

    fn check_any(
        &self,
        value: &Value,
        branches: &'a [Value],
        at: &str,
        depth: usize,
    ) -> Result<(), String> {
        let passes = branches
            .iter()
            .any(|branch| self.check_depth(value, branch, at, depth + 1).is_ok());
        passes
            .then_some(())
            .ok_or_else(|| format!("{at}: {value} matches no anyOf branch"))
    }

    fn check_object(
        &self,
        value: &Value,
        schema: &'a Value,
        at: &str,
        depth: usize,
    ) -> Result<(), String> {
        let fields = value.as_object().expect("object");
        if is_unconstrained(schema) {
            return Ok(()); // An unconstrained schema (`{}`) accepts any object.
        }
        for name in required(schema) {
            if !fields.contains_key(name) {
                return Err(format!("{at}: required field {name} missing"));
            }
        }
        for (name, field) in fields {
            let path = format!("{at}.{name}");
            match schema.get("properties").and_then(|p| p.get(name)) {
                Some(prop) => self.check_depth(field, prop, &path, depth + 1)?,
                None => self.check_additional(field, schema, &path, depth)?,
            }
        }
        Ok(())
    }

    fn check_additional(
        &self,
        field: &Value,
        schema: &'a Value,
        at: &str,
        depth: usize,
    ) -> Result<(), String> {
        match schema.get("additionalProperties") {
            Some(Value::Bool(true)) => Ok(()),
            Some(extra @ Value::Object(_)) => self.check_depth(field, extra, at, depth + 1),
            _ => Err(format!("{at}: field is not documented in the contract")),
        }
    }

    fn check_items(
        &self,
        items: &[Value],
        schema: &'a Value,
        at: &str,
        depth: usize,
    ) -> Result<(), String> {
        let Some(item_schema) = schema.get("items") else {
            return Ok(());
        };
        for (i, item) in items.iter().enumerate() {
            self.check_depth(item, item_schema, &format!("{at}[{i}]"), depth + 1)?;
        }
        Ok(())
    }
}

/// True when a schema constrains nothing about an object: no `type`, `properties`,
/// `additionalProperties` or `required` (annotations such as `description` may be present).
pub fn is_unconstrained(schema: &Value) -> bool {
    ["type", "properties", "additionalProperties", "required"]
        .iter()
        .all(|key| schema.get(key).is_none())
}

/// Names listed in a schema's `required` array.
pub fn required(schema: &Value) -> Vec<&str> {
    schema
        .get("required")
        .and_then(Value::as_array)
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

fn check_const_and_enum(value: &Value, schema: &Value, at: &str) -> Result<(), String> {
    if let Some(constant) = schema.get("const") {
        if constant != value {
            return Err(format!(
                "{at}: {value} is not the documented const {constant}"
            ));
        }
    }
    if let Some(options) = schema.get("enum").and_then(Value::as_array) {
        if !options.contains(value) {
            return Err(format!("{at}: {value} is not in the documented enum"));
        }
    }
    Ok(())
}

fn check_type(value: &Value, schema: &Value, at: &str) -> Result<(), String> {
    let allowed: Vec<&str> = match schema.get("type") {
        None => return Ok(()),
        Some(Value::String(one)) => vec![one.as_str()],
        Some(Value::Array(many)) => many.iter().filter_map(Value::as_str).collect(),
        Some(other) => return Err(format!("{at}: malformed type {other}")),
    };
    if allowed.iter().any(|t| type_matches(value, t)) {
        return Ok(());
    }
    Err(format!("{at}: {value} is not of type {allowed:?}"))
}

fn type_matches(value: &Value, name: &str) -> bool {
    match name {
        "null" => value.is_null(),
        "boolean" => value.is_boolean(),
        "integer" => value.is_i64() || value.is_u64(),
        "number" => value.is_number(),
        "string" => value.is_string(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        _ => false,
    }
}

fn check_bounds(value: &Value, schema: &Value, at: &str) -> Result<(), String> {
    let number = value.as_f64();
    let below = schema
        .get("minimum")
        .and_then(Value::as_f64)
        .zip(number)
        .is_some_and(|(min, n)| n < min);
    let above = schema
        .get("maximum")
        .and_then(Value::as_f64)
        .zip(number)
        .is_some_and(|(max, n)| n > max);
    let short = schema
        .get("minLength")
        .and_then(Value::as_u64)
        .zip(value.as_str())
        .is_some_and(|(min, s)| (s.chars().count() as u64) < min);
    let long = schema
        .get("maxLength")
        .and_then(Value::as_u64)
        .zip(value.as_str())
        .is_some_and(|(max, s)| (s.chars().count() as u64) > max);
    if below || above || short || long {
        return Err(format!("{at}: {value} is outside the documented bounds"));
    }
    Ok(())
}

/// Every schema keyword in `schema` (recursively) that [`KEYWORDS`] does not cover,
/// prefixed with its location.
pub fn unsupported_keywords(schema: &Value, at: &str) -> Vec<String> {
    let mut found = Vec::new();
    collect_unsupported(schema, at, 0, &mut found);
    found
}

fn collect_unsupported(schema: &Value, at: &str, depth: usize, found: &mut Vec<String>) {
    let Some(map) = schema.as_object() else {
        return;
    };
    if depth > MAX_DEPTH {
        found.push(format!("{at}: nesting deeper than {MAX_DEPTH}"));
        return;
    }
    for (key, child) in map {
        if !KEYWORDS.contains(&key.as_str()) {
            found.push(format!("{at}.{key}"));
        }
        match key.as_str() {
            "properties" => child
                .as_object()
                .into_iter()
                .flatten()
                .for_each(|(name, prop)| {
                    collect_unsupported(prop, &format!("{at}.{name}"), depth + 1, found)
                }),
            "items" | "additionalProperties" => {
                collect_unsupported(child, &format!("{at}.{key}"), depth + 1, found)
            }
            "anyOf" => child.as_array().into_iter().flatten().for_each(|branch| {
                collect_unsupported(branch, &format!("{at}.anyOf"), depth + 1, found)
            }),
            _ => {}
        }
    }
}
