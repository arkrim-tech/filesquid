//! Identity-field classes of the import record and their scrub (ADR 038
//! Decision 6; the outgoing disclosure policy of F-046).
//!
//! Paths, account and host names, numeric IDs, origin URLs, quarantine
//! agents and Finder comments are identity and location fields: exports and
//! shares scrub them by default, and the owner's own library keeps them.
//! [`ImportEntry::scrubbed`] returns a copy with the chosen classes
//! withheld; each removed fact is listed as unavailable with the reason
//! [`Reason::WithheldByPolicy`]. The original file name is not an identity
//! class here (ADR 038 Decision 6 lists the classes); the outgoing filename
//! policy of C-033 and API-S-027 governs the names of shared files.

use super::attributes::{AttrValue, Attribute, Attributes};
use super::{Host, ImportEntry, SourceFile, SourceVolume};
use crate::fact::{Fact, Reason};

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
}

impl IdentityClass {
    /// Every class: what an export scrubs by default.
    pub const ALL: [IdentityClass; 7] = [
        IdentityClass::Path,
        IdentityClass::Account,
        IdentityClass::Host,
        IdentityClass::NumericId,
        IdentityClass::OriginUrl,
        IdentityClass::QuarantineAgent,
        IdentityClass::FinderComment,
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
