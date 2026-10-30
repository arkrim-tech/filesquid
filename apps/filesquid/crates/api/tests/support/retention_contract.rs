//! The retention check of API-S-014 (Task API-F2.5): the contract offers no way to delete,
//! expire or thin versions, whatever the HTTP method or path shape.

use serde_json::Value;

/// Words that name taking versions away.
const REMOVAL_WORDS: [&str; 8] = [
    "delet", "remov", "expir", "thin", "prun", "purg", "discard", "drop",
];

/// The `DELETE` operations that remove no library content: an import plan holds no file and
/// made no version, so discarding it (API-S-036 **Review**) is not a version removal.
const CONTENT_FREE_DELETES: [&str; 1] = ["/api/v1/imports/{plan_id}"];

/// Every operation of `doc` that could take versions away: a `DELETE`, a path naming
/// expiry, thinning, pruning or purging, or an operation whose ID or summary names both a
/// removal and a version. Pure.
pub fn version_removals(doc: &Value) -> Vec<String> {
    let mut found = Vec::new();
    for (path, item) in doc["paths"].as_object().expect("paths") {
        if item.get("delete").is_some() && !CONTENT_FREE_DELETES.contains(&path.as_str()) {
            found.push(format!("DELETE {path}"));
        }
        let lower = path.to_lowercase();
        if ["expir", "thin", "prune", "purge"]
            .iter()
            .any(|w| lower.contains(w))
        {
            found.push(path.clone());
        }
        for (method, op) in item.as_object().expect("item") {
            if names_version_removal(op) {
                found.push(format!("{method} {path}"));
            }
        }
    }
    found
}

/// True when the operation's ID or summary names both a removal and a version. Pure.
fn names_version_removal(op: &Value) -> bool {
    let text = format!(
        "{} {}",
        op["operationId"].as_str().unwrap_or_default(),
        op["summary"].as_str().unwrap_or_default()
    )
    .to_lowercase();
    text.contains("version") && REMOVAL_WORDS.iter().any(|word| text.contains(word))
}
