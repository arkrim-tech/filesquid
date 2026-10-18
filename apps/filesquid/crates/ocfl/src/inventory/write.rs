//! Writing an inventory as JSON. Keys are sorted (OCFL gives key order no
//! meaning, and sorted output is stable), optional keys are written only
//! when set, and the text is pretty-printed with a final newline.

use serde_json::{json, Map, Value};

use super::{DigestMap, Inventory, User, Version};

/// The inventory's JSON bytes.
pub(super) fn inventory_json(inventory: &Inventory) -> Vec<u8> {
    let mut root = Map::new();
    root.insert("id".into(), json!(inventory.id));
    root.insert("type".into(), json!(inventory.spec.inventory_type()));
    root.insert(
        "digestAlgorithm".into(),
        json!(inventory.digest_algorithm.name()),
    );
    root.insert("head".into(), json!(inventory.head.to_string()));
    if let Some(name) = &inventory.content_directory {
        root.insert("contentDirectory".into(), json!(name));
    }
    root.insert("manifest".into(), digest_map(&inventory.manifest));
    let versions = inventory
        .versions
        .iter()
        .map(|(num, block)| (num.to_string(), version_json(block)))
        .collect();
    root.insert("versions".into(), Value::Object(versions));
    if let Some(fixity) = &inventory.fixity {
        let blocks = fixity
            .iter()
            .map(|(algorithm, map)| (algorithm.clone(), digest_map(map)))
            .collect();
        root.insert("fixity".into(), Value::Object(blocks));
    }
    // A `Value` always formats; `{:#}` is serde_json's pretty printer.
    let mut text = format!("{:#}", Value::Object(root));
    text.push('\n');
    text.into_bytes()
}

/// One version block.
fn version_json(block: &Version) -> Value {
    let mut object = Map::new();
    object.insert("created".into(), json!(block.created));
    object.insert("state".into(), digest_map(&block.state));
    if let Some(message) = &block.message {
        object.insert("message".into(), json!(message));
    }
    if let Some(user) = &block.user {
        object.insert("user".into(), user_json(user));
    }
    Value::Object(object)
}

/// A user block.
fn user_json(user: &User) -> Value {
    let mut object = Map::new();
    object.insert("name".into(), json!(user.name));
    if let Some(address) = &user.address {
        object.insert("address".into(), json!(address));
    }
    Value::Object(object)
}

/// A digest map as a JSON object of string arrays.
fn digest_map(map: &DigestMap) -> Value {
    Value::Object(
        map.iter()
            .map(|(digest, paths)| (digest.clone(), json!(paths)))
            .collect(),
    )
}
