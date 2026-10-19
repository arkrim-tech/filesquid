//! Shared storage facts: the version that last set a path's content, the
//! extension type hint, and JSON forms of version records. Pure.

use serde_json::{json, Value};

use crate::inventory::{Inventory, User, Version, VersionNum};
use crate::read::VersionInfo;

/// The extension of `name`, lowercase, as a display hint only (C-055): no
/// bytes are read and no format is identified.
pub(crate) fn type_hint(name: &str) -> Option<String> {
    let (stem, ext) = name.rsplit_once('.')?;
    (!stem.is_empty() && !ext.is_empty()).then(|| ext.to_ascii_lowercase())
}

/// The last segment of `path`.
pub(crate) fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The record of `version` in `inventory`, if it lists it.
pub(crate) fn version_info(inventory: &Inventory, version: &VersionNum) -> Option<VersionInfo> {
    let block = inventory.versions.get(version)?;
    Some(VersionInfo {
        version: version.clone(),
        created: block.created.clone(),
        message: block.message.clone(),
        user: block.user.clone(),
    })
}

/// The digest `path` has in `block`'s state, as the inventory writes it.
/// Only that path is looked up; no state is rebuilt.
pub(crate) fn digest_at<'i>(block: &'i Version, path: &str) -> Option<&'i String> {
    block
        .state
        .iter()
        .find(|(_, paths)| paths.iter().any(|p| p == path))
        .map(|(digest, _)| digest)
}

/// The earliest version, up to `version`, from which `path` has held
/// `digest` without a break: the version that last set its content.
pub(crate) fn last_change(
    inventory: &Inventory,
    version: &VersionNum,
    path: &str,
    digest: &str,
) -> VersionNum {
    let mut since = version.clone();
    for (earlier, block) in inventory.versions.range(..version).rev() {
        let holds = digest_at(block, path).is_some_and(|d| d.eq_ignore_ascii_case(digest));
        if !holds {
            break;
        }
        since = earlier.clone();
    }
    since
}

/// One version record as JSON: `version`, `created`, `message`, `user`.
pub fn version_json(info: &VersionInfo) -> Value {
    json!({
        "version": info.version.to_string(),
        "created": info.created,
        "message": info.message,
        "user": info.user.as_ref().map(user_json),
    })
}

/// Version records as a JSON array.
pub fn versions_json(infos: &[VersionInfo]) -> Value {
    Value::Array(infos.iter().map(version_json).collect())
}

/// A version's `user` as JSON.
fn user_json(user: &User) -> Value {
    json!({ "name": user.name, "address": user.address })
}
