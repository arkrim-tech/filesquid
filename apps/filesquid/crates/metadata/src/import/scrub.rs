//! Identity-field classes of the import record and their scrub (ADR 038
//! Decision 6; the outgoing disclosure policy of F-046).
//!
//! Paths, account and host names, numeric IDs, origin URLs, quarantine
//! agents, Finder comments and the original file name are identity and
//! location fields: exports and shares scrub them by default, and the
//! owner's own library keeps them. [`ImportEntry::scrubbed`] returns a copy
//! of one entry with the chosen classes withheld, and
//! [`FileRecord::scrubbed`](crate::file_record::FileRecord::scrubbed) the
//! outgoing copy of a whole record; each removed fact is listed as
//! unavailable with the reason [`Reason::WithheldByPolicy`].
//!
//! [`IdentityClass::OriginalName`] (owner decision 2026-10-20, ADR 038
//! Decision 6 as amended) covers the original file name wherever the record
//! holds it: each entry's source file name here, and the header's original
//! name and display name in the record. Like every class it is scrubbed by
//! default ([`IdentityClass::ALL`]); a caller whose saved outgoing policy
//! selects **Use original filename** (C-033) leaves it out of the classes,
//! so the name inside a shared record follows the shared file's own name.
//! The outgoing copy goes only into exports and shares: the `ocfl` store
//! refuses to save it.

use super::attributes::{AttrValue, Attribute, Attributes};
use super::{Host, ImportEntry, SourceFile, SourceVolume};
use crate::fact::{Fact, Reason};
use crate::raw::Raw;

/// A class of identity or location fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityClass {
    /// Absolute, import-root and relative paths, link targets, mount points.
    Path,
    /// Owner and group names, ACL entries and the host account.
    Account,
    /// The host name.
    Host,
    /// Owner and group numeric IDs.
    NumericId,
    /// Origin URLs (`com.apple.metadata:kMDItemWhereFroms`).
    OriginUrl,
    /// The quarantine record (`com.apple.quarantine`: agent, time, event).
    QuarantineAgent,
    /// The Finder comment (`com.apple.metadata:kMDItemFinderComment`).
    FinderComment,
    /// The original file name: each entry's source file name and, in a
    /// whole record, the header's original name and display name. Kept only
    /// when the saved outgoing policy selects **Use original filename**.
    OriginalName,
}

impl IdentityClass {
    /// Every class: what an export, share or publication scrubs by default.
    pub const ALL: [IdentityClass; 8] = [
        IdentityClass::Path,
        IdentityClass::Account,
        IdentityClass::Host,
        IdentityClass::NumericId,
        IdentityClass::OriginUrl,
        IdentityClass::QuarantineAgent,
        IdentityClass::FinderComment,
        IdentityClass::OriginalName,
    ];

    /// The extended attribute this class withholds, if it is an attribute class.
    fn attribute_name(self) -> Option<&'static [u8]> {
        match self {
            Self::OriginUrl => Some(b"com.apple.metadata:kMDItemWhereFroms"),
            Self::QuarantineAgent => Some(b"com.apple.quarantine"),
            Self::FinderComment => Some(b"com.apple.metadata:kMDItemFinderComment"),
            _ => None,
        }
    }
}

/// A fact removed by policy.
fn withheld<T>() -> Fact<T> {
    Fact::unavailable(Reason::WithheldByPolicy)
}

impl ImportEntry {
    /// A copy of the entry with every field of `classes` withheld. The entry
    /// stays valid: bounds and sizes are untouched.
    #[must_use]
    pub fn scrubbed(&self, classes: &[IdentityClass]) -> ImportEntry {
        let mut entry = self.clone();
        for class in classes {
            scrub_source(&mut entry.source, *class);
            scrub_volume(&mut entry.volume, *class);
            scrub_host(&mut entry.host, *class);
            if let Fact::Value(attributes) = &mut entry.attributes {
                scrub_attributes(attributes, *class);
            }
        }
        entry
    }
}

/// Withhold the source-file fields of `class`.
fn scrub_source(source: &mut SourceFile, class: IdentityClass) {
    match class {
        IdentityClass::Path => {
            source.absolute_path = withheld();
            source.import_root = withheld();
            source.relative_path = withheld();
            source.link = withheld();
        }
        IdentityClass::Account => {
            source.owner_name = withheld();
            source.group_name = withheld();
            source.acl = withheld();
        }
        IdentityClass::NumericId => {
            source.owner_id = withheld();
            source.group_id = withheld();
        }
        IdentityClass::OriginalName => source.original_name = Raw::from_bytes(b""),
        _ => {}
    }
}

/// Withhold the volume fields of `class`.
fn scrub_volume(volume: &mut SourceVolume, class: IdentityClass) {
    if class == IdentityClass::Path {
        volume.mount_point = withheld();
    }
}

/// Withhold the host fields of `class`.
fn scrub_host(host: &mut Host, class: IdentityClass) {
    match class {
        IdentityClass::Account => host.account = withheld(),
        IdentityClass::Host => host.host_name = withheld(),
        _ => {}
    }
}

/// Withhold the value of the known attribute `class` names; its name and
/// size stay.
fn scrub_attributes(attributes: &mut Attributes, class: IdentityClass) {
    let Some(name) = class.attribute_name() else {
        return;
    };
    for attribute in attributes
        .entries
        .iter_mut()
        .filter(|a| a.name.as_bytes() == name)
    {
        withhold_value(attribute);
    }
}

/// Replace one attribute's value and decoded fields with the withheld mark.
fn withhold_value(attribute: &mut Attribute) {
    attribute.value = AttrValue::Withheld;
    attribute.decoded.clear();
}
