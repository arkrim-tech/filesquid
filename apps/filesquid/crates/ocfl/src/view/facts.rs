//! Shared storage facts: the version that last set a path's content, the
//! extension type hint, and JSON forms of version records. Pure.

use serde_json::{json, Value};

use crate::change::state_digests;
use crate::inventory::{Inventory, User, VersionNum};
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

/// The earliest version, up to `version`, from which `path` has held
/// `digest` without a break: the version that last set its content.
pub(crate) fn last_change(
    inventory: &Inventory,
    version: &VersionNum,
    path: &str,
    digest: &str,
) -> VersionNum {
    let digest = digest.to_ascii_lowercase();
    let mut since = version.clone();
    for (earlier, _) in inventory.versions.range(..version).rev() {
        // Every version the inventory lists resolves; one that does not
        // cannot have held the path.
        let holds = state_digests(Some(inventory), earlier)
            .is_ok_and(|state| state.get(path) == Some(&digest));
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
