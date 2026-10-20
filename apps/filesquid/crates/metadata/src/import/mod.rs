//! The import record: the append-only `import` section of a file's metadata
//! record `<file-uuid>.metadata.md` ([ADR 038](../../../../../resources/architecture/038-import-file-record.md),
//! MD-S-013).
//!
//! One [`ImportEntry`] describes one acquisition of a file: how and when it
//! was imported ([`Acquisition`]), the source file ([`SourceFile`]), the
//! source volume ([`SourceVolume`]), its extended attributes
//! ([`attributes::Attributes`]) and the importing host ([`Host`]). The crate
//! defines, validates and (through the `ocfl` store) persists the entry; the
//! caller supplies every fact. It reads no filesystem, so it builds for
//! `wasm32`. Each fact the source or host could not provide is a
//! [`Fact::Unavailable`] with a [`Reason`]; nothing is guessed.
//!
//! [`ImportEntry::for_upload`] builds the entry of a browser or API upload,
//! where only a name, size, time and media type are known.

pub mod attributes;
pub mod scrub;
pub(crate) mod time;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::fact::{Fact, Reason};
use crate::file_record::extension_of_bytes;
use crate::file_ref::is_valid_logical_path;
use crate::raw::Raw;
use attributes::Attributes;
use time::is_rfc3339_utc;

/// The versioned schema identifier at the top of every file record.
pub const SCHEMA: &str = "filesquid.original.v1";

/// Why an import entry was refused. Messages carry no stored value.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ImportError {
    /// The size the source reported differs from the size verified from the
    /// bytes read and hashed: the import fails verification and stays pending.
    #[error("the reported size {reported} differs from the verified size {verified}")]
    SizeMismatch {
        /// Size as the source reported it.
        reported: u64,
        /// Size of the bytes read and hashed.
        verified: u64,
    },
    /// The size cited from the OCFL object differs from the verified size.
    #[error("the cited size {cited} differs from the verified size {verified}")]
    CitedSizeMismatch {
        /// Size cited for the OCFL object.
        cited: u64,
        /// Size of the bytes read and hashed.
        verified: u64,
    },
    /// A time is not an RFC 3339 UTC timestamp.
    #[error("{field} is not an RFC 3339 UTC timestamp")]
    InvalidTimestamp {
        /// The field name.
        field: &'static str,
    },
    /// The cited digest is not 64 lowercase hex digits.
    #[error("the cited SHA-256 digest is not 64 lowercase hex digits")]
    InvalidDigest,
    /// A managed import must cite the logical path of the stored original.
    #[error("a managed import must cite its logical path")]
    MissingLogicalPath,
    /// A referenced import has no stored original to cite.
    #[error("a referenced import has no logical path to cite")]
    UnexpectedLogicalPath,
    /// More than 256 attribute names.
    #[error("more than 256 attribute names")]
    TooManyAttributeNames,
    /// More than 1 MiB of attribute value bytes read.
    #[error("more than 1 MiB of attribute value bytes read")]
    AttributeReadOverBudget,
    /// A value over 64 KiB stored whole.
    #[error("an attribute value over 64 KiB was stored whole")]
    AttributeValueTooLarge,
    /// The whole values stored hold more bytes than the entry says it read.
    #[error("the attribute values stored exceed the bytes the entry says it read")]
    AttributeBytesUnderReported,
    /// The cited logical path is not a valid relative logical path.
    #[error("the cited logical path is not a valid relative logical path")]
    InvalidLogicalPath,
    /// Attribute names were left out without the truncated mark.
    #[error("attribute names were left out without the truncated mark")]
    TruncationNotMarked,
}

/// How the file reached FileSquid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportMethod {
    /// A file picker.
    FilePicker,
    /// A folder import.
    Folder,
    /// Drag and drop.
    DragAndDrop,
    /// Another application handed the file over.
    OtherApplication,
    /// An upload through the API.
    ApiUpload,
}

/// Whether the bytes were copied into the library or only referenced.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ByteStorage {
    /// Copied into the OCFL object.
    Copied,
    /// Left at their source location (explicit referenced import).
    Referenced,
}

/// The Unicode normalisation form of a file name as the source reported it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NameForm {
    /// Canonical composition.
    Nfc,
    /// Canonical decomposition (macOS HFS+ and APFS file names).
    Nfd,
    /// Compatibility composition.
    Nfkc,
    /// Compatibility decomposition.
    Nfkd,
    /// In none of the four forms, or not valid Unicode.
    Unnormalised,
}

/// Whether a volume is local, removable or network storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolumeKind {
    /// A local disk.
    Local,
    /// Removable media.
    Removable,
    /// Network storage.
    Network,
}

/// Whether the source was a link, and where it pointed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Link {
    /// An ordinary file.
    None,
    /// A symbolic link; the bytes read are the target's.
    Symlink {
        /// The link's target path as reported.
        target: Raw,
    },
    /// A macOS alias; the bytes read are the target's.
    Alias {
        /// The alias's target path as reported.
        target: Raw,
    },
}

/// The content type the operating system reports.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentType {
    /// Uniform Type Identifier (macOS).
    pub uti: Fact<String>,
    /// MIME type.
    pub mime: Fact<String>,
}

/// When, how and as what an import happened, and the content it cites.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Acquisition {
    /// When the record was written (RFC 3339 UTC).
    pub recorded_at: String,
    /// When the import started (RFC 3339 UTC).
    pub started_at: String,
    /// When the import finished (RFC 3339 UTC).
    pub finished_at: String,
    /// How the file reached FileSquid.
    pub method: ImportMethod,
    /// Copied or referenced.
    pub storage: ByteStorage,
    /// The import job.
    pub job_id: Fact<String>,
    /// The operation.
    pub operation_id: Fact<String>,
    /// The OCFL object the content is in (cited for binding).
    pub object_id: String,
    /// The OCFL version that holds the content (cited for binding).
    pub object_version: String,
    /// The logical path of the stored original; `None` for a referenced import.
    pub logical_path: Option<String>,
    /// SHA-256 of the content, lowercase hex (cited for binding).
    pub sha256: String,
    /// Size of the content in bytes (cited for binding).
    pub size: u64,
}

/// The source file as the source system reported it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFile {
    /// The file name exactly as reported.
    pub original_name: Raw,
    /// The normalisation form of that name.
    pub name_normalization: NameForm,
    /// Original absolute path.
    pub absolute_path: Fact<Raw>,
    /// The import root.
    pub import_root: Fact<Raw>,
    /// The path relative to the import root.
    pub relative_path: Fact<Raw>,
    /// Size in bytes as the source reported it before FileSquid read it (for
    /// a link or alias, the followed target's size).
    pub reported_size: Fact<u64>,
    /// Size in bytes of the bytes FileSquid read and hashed.
    pub verified_size: u64,
    /// Birth (created) time at native precision.
    pub birth_time: Fact<String>,
    /// Modified time at native precision.
    pub modified_time: Fact<String>,
    /// Status-changed time at native precision.
    pub status_changed_time: Fact<String>,
    /// Accessed time at native precision, captured before the read.
    pub accessed_time: Fact<String>,
    /// Whether the accessed time was read after the access could have
    /// changed it (a filesystem that updates it anyway).
    pub accessed_after_read: bool,
    /// "Date added" time at native precision.
    pub date_added_time: Fact<String>,
    /// Owner name.
    pub owner_name: Fact<Raw>,
    /// Owner numeric ID.
    pub owner_id: Fact<u64>,
    /// Group name.
    pub group_name: Fact<Raw>,
    /// Group numeric ID.
    pub group_id: Fact<u64>,
    /// POSIX mode bits.
    pub posix_mode: Fact<u32>,
    /// ACL entries as reported.
    pub acl: Fact<Vec<Raw>>,
    /// File flags (hidden, locked, ...).
    pub flags: Fact<Vec<String>>,
    /// Whether the source was a link or alias, and its target.
    pub link: Fact<Link>,
    /// The content type the operating system reports.
    pub os_content_type: Fact<ContentType>,
    /// The file name extension; `None` when the name has none.
    pub extension: Option<String>,
}

/// The volume the file came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceVolume {
    /// Volume name.
    pub name: Fact<Raw>,
    /// Volume UUID.
    pub uuid: Fact<String>,
    /// Filesystem type.
    pub filesystem_type: Fact<String>,
    /// Mount point.
    pub mount_point: Fact<Raw>,
    /// Local, removable or network.
    pub kind: Fact<VolumeKind>,
}

/// The host that ran the import.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Host {
    /// Operating system name.
    pub os_name: Fact<String>,
    /// Operating system version.
    pub os_version: Fact<String>,
    /// Operating system build.
    pub os_build: Fact<String>,
    /// CPU architecture.
    pub architecture: Fact<String>,
    /// Host name.
    pub host_name: Fact<String>,
    /// The operating-system account that ran the import.
    pub account: Fact<String>,
    /// FileSquid version.
    pub filesquid_version: String,
    /// FileSquid build.
    pub filesquid_build: String,
    /// FileSquid distribution (for example `filesquid`, `imagesquid`).
    pub distribution: String,
}

/// One acquisition of a file: an entry of the `import` section.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportEntry {
    /// When, how and as what the import happened.
    pub acquisition: Acquisition,
    /// The source file.
    pub source: SourceFile,
    /// The source volume.
    pub volume: SourceVolume,
    /// The extended attributes, or why they are unavailable.
    pub attributes: Fact<Attributes>,
    /// The importing host.
    pub host: Host,
}

/// What a browser or API upload provides.
#[derive(Clone, Debug)]
pub struct UploadFacts {
    /// The acquisition; its `size` is the verified size of the bytes read.
    pub acquisition: Acquisition,
    /// The file name the client sent.
    pub name: Raw,
    /// The normalisation form of that name.
    pub name_normalization: NameForm,
    /// The size the client reported.
    pub reported_size: u64,
    /// The last-modified time the client reported (RFC 3339 UTC).
    pub last_modified: Option<String>,
    /// The media type the client reported.
    pub media_type: Option<String>,
    /// The host that received the upload.
    pub host: Host,
}

/// Shorthand for a fact an upload cannot provide.
fn not_uploaded<T>() -> Fact<T> {
    Fact::unavailable(Reason::NotProvidedByUpload)
}

impl ImportEntry {
    /// The entry of an upload: the facts the client provided, and every other
    /// fact listed as unavailable because an upload does not carry it.
    pub fn for_upload(facts: UploadFacts) -> Self {
        let extension = extension_of_bytes(facts.name.as_bytes())
            .and_then(|e| std::str::from_utf8(e).ok())
            .map(str::to_string);
        let modified = facts.last_modified.map_or_else(not_uploaded, Fact::value);
        let mime = facts.media_type;
        let verified_size = facts.acquisition.size;
        ImportEntry {
            acquisition: facts.acquisition,
            source: SourceFile {
                original_name: facts.name,
                name_normalization: facts.name_normalization,
                absolute_path: not_uploaded(),
                import_root: not_uploaded(),
                relative_path: not_uploaded(),
                reported_size: Fact::value(facts.reported_size),
                verified_size,
                birth_time: not_uploaded(),
                modified_time: modified,
                status_changed_time: not_uploaded(),
                accessed_time: not_uploaded(),
                accessed_after_read: false,
                date_added_time: not_uploaded(),
                owner_name: not_uploaded(),
                owner_id: not_uploaded(),
                group_name: not_uploaded(),
                group_id: not_uploaded(),
                posix_mode: not_uploaded(),
                acl: not_uploaded(),
                flags: not_uploaded(),
                link: not_uploaded(),
                os_content_type: match mime {
                    Some(mime) => Fact::value(ContentType {
                        uti: not_uploaded(),
                        mime: Fact::value(mime),
                    }),
                    None => not_uploaded(),
                },
                extension,
            },
            volume: SourceVolume {
                name: not_uploaded(),
                uuid: not_uploaded(),
                filesystem_type: not_uploaded(),
                mount_point: not_uploaded(),
                kind: not_uploaded(),
            },
            attributes: not_uploaded(),
            host: facts.host,
        }
    }

    /// Check every rule of MD-S-013 that needs no store: the two sizes agree,
    /// the cited size and digest are well formed, every time is RFC 3339 UTC
    /// and the attribute bounds hold.
    ///
    /// # Errors
    ///
    /// The first rule that fails, as an [`ImportError`].
    pub fn validate(&self) -> Result<(), ImportError> {
        self.validate_sizes()?;
        self.validate_citation()?;
        self.validate_times()?;
        match &self.attributes {
            Fact::Value(attributes) => attributes.validate(),
            Fact::Unavailable(_) => Ok(()),
        }
    }

    /// The reported and cited sizes must equal the verified size.
    fn validate_sizes(&self) -> Result<(), ImportError> {
        let verified = self.source.verified_size;
        if let Fact::Value(reported) = self.source.reported_size {
            if reported != verified {
                return Err(ImportError::SizeMismatch { reported, verified });
            }
        }
        let cited = self.acquisition.size;
        if cited != verified {
            return Err(ImportError::CitedSizeMismatch { cited, verified });
        }
        Ok(())
    }

    /// The cited digest and logical path must be well formed.
    fn validate_citation(&self) -> Result<(), ImportError> {
        let a = &self.acquisition;
        let hex = a.sha256.len() == 64
            && a.sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if !hex {
            return Err(ImportError::InvalidDigest);
        }
        match (a.storage, &a.logical_path) {
            (ByteStorage::Copied, None) => Err(ImportError::MissingLogicalPath),
            (ByteStorage::Referenced, Some(_)) => Err(ImportError::UnexpectedLogicalPath),
            (ByteStorage::Copied, Some(path)) if !is_valid_logical_path(path) => {
                Err(ImportError::InvalidLogicalPath)
            }
            _ => Ok(()),
        }
    }

    /// Every time must be an RFC 3339 UTC timestamp.
    fn validate_times(&self) -> Result<(), ImportError> {
        let a = &self.acquisition;
        let s = &self.source;
        let times: [(&'static str, Option<&String>); 8] = [
            ("recorded_at", Some(&a.recorded_at)),
            ("started_at", Some(&a.started_at)),
            ("finished_at", Some(&a.finished_at)),
            ("birth_time", s.birth_time.as_value()),
            ("modified_time", s.modified_time.as_value()),
            ("status_changed_time", s.status_changed_time.as_value()),
            ("accessed_time", s.accessed_time.as_value()),
            ("date_added_time", s.date_added_time.as_value()),
        ];
        for (field, time) in times {
            if time.is_some_and(|t| !is_rfc3339_utc(t)) {
                return Err(ImportError::InvalidTimestamp { field });
            }
        }
        Ok(())
    }
}
