//! The per-file metadata record `<file-uuid>.metadata.md` (MD-S-008, MD-S-013;
//! [ADR 038](../../../../resources/architecture/038-import-file-record.md),
//! [ADR 027](../../../../resources/architecture/027-filesquid-metadata-pluggable-stores.md)).
//!
//! A [`FileRecord`] is THE metadata record of one file: the owner's exact
//! original file name and its normalisation form, the display name (initially
//! the original name), the extension, the format and media type, the
//! descriptive statements ([`Record`]) and the append-only `import` section
//! (a list of [`ImportEntry`]). It is pure data and builds for `wasm32`; the
//! `ocfl` store persists it.
//!
//! The front matter opens with a versioned schema identifier, then one JSON
//! value per header line (JSON is YAML, so binary names round-trip as
//! base64), the descriptive namespaces, and last the `import` list with one
//! JSON entry per line:
//!
//! ```text
//! ---
//! schema: filesquid.original.v1
//! original_name: "Holiday 2023.JPG"
//! name_normalization: "nfc"
//! display_name: "Holiday 2023.JPG"
//! extension: "JPG"
//! format: null
//! media_type: "image/jpeg"
//! dcterms:
//!   title: Harbour
//! import:
//!   - {"acquisition": {...}, ...}
//! ---
//! ```
//!
//! [`FileRecord::check_extends`] is the append-only rule: a record may only
//! add import entries, change statements and the display name.
//!
//! [`FileRecord::scrubbed`] is the outgoing copy for exports and shares
//! (MD-S-013, ADR 038 Decision 6): the chosen identity classes are withheld
//! in every import entry and, for
//! [`IdentityClass::OriginalName`], the header's two names are written as
//! the unavailable fact `{"unavailable":"withheld_by_policy"}`. Such a copy
//! reads back, but the `ocfl` store refuses to save it
//! ([`RecordError::NamesWithheld`]).

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::fact::{Fact, Reason};
use crate::import::scrub::IdentityClass;
use crate::import::{ByteStorage, ImportEntry, ImportError, ImportMethod, NameForm, SCHEMA};
use crate::markdown;
use crate::raw::Raw;
use crate::record::Record;

/// Longest original or display name, in bytes (API-S-026).
pub const MAX_NAME_BYTES: usize = 1024;

/// Longest recorded extension or format label, in bytes.
const MAX_LABEL_BYTES: usize = 255;

/// The header keys of a file record, in the order they are written.
const HEADER_KEYS: [&str; 6] = [
    "original_name",
    "name_normalization",
    "display_name",
    "extension",
    "format",
    "media_type",
];

/// Why a file record was refused. Messages carry no stored value.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RecordError {
    /// The original name is empty, over 1,024 bytes, or holds `/` or NUL.
    #[error("the original file name must be 1 to 1,024 bytes without `/` or NUL")]
    InvalidName,
    /// The display name is empty, over 1,024 bytes, or holds `/` or NUL.
    #[error("the display name must be 1 to 1,024 bytes without `/` or NUL")]
    InvalidDisplayName,
    /// The extension is empty, too long, or holds `/` or NUL.
    #[error("the extension must be 1 to 255 bytes without `/` or NUL")]
    InvalidExtension,
    /// The format label is too long or holds a control character.
    #[error("the format must be at most 255 bytes without control characters")]
    InvalidFormat,
    /// The media type is not `type/subtype` of RFC 6838 restricted names.
    #[error("the media type must be type/subtype")]
    InvalidMediaType,
    /// A file record has at least one import entry.
    #[error("a file record needs at least one import entry")]
    NoImport,
    /// An import entry failed validation.
    #[error("invalid import entry: {0}")]
    Import(#[from] ImportError),
    /// The text is not a file record of schema `filesquid.original.v1`.
    #[error("not a filesquid.original.v1 file record")]
    UnsupportedSchema,
    /// A header field is missing.
    #[error("the file record lacks `{0}`")]
    MissingField(&'static str),
    /// The text is not well formed.
    #[error("malformed file record: {0}")]
    Malformed(&'static str),
    /// A save changed the recorded original file name.
    #[error("the recorded original file name cannot change")]
    OriginalNameChanged,
    /// A save changed an existing import entry.
    #[error("import entry {index} cannot change")]
    ImportEntryChanged {
        /// Zero-based position of the entry.
        index: usize,
    },
    /// A save removed an existing import entry.
    #[error("import entries cannot be removed")]
    ImportEntryRemoved,
    /// A new entry cites another OCFL object than the one it is saved in.
    #[error("an import entry cites another OCFL object")]
    BoundToOtherObject,
    /// A new managed entry cites another logical path than the stored original.
    #[error("an import entry cites another logical path")]
    BoundToOtherPath,
    /// The file has no record yet.
    #[error("the file has no metadata record")]
    NoFileRecord,
    /// A new managed entry cites the path the record itself is saved at, so
    /// the record would replace the imported bytes.
    #[error("the metadata record would replace the imported content")]
    RecordAtContentPath,
    /// A new managed entry's content is not staged at its logical path.
    #[error("the imported content is not staged at its cited logical path")]
    ContentNotStaged,
    /// The content staged at the cited path is not the content the entry
    /// verified.
    #[error("the staged content differs from the entry's cited SHA-256")]
    ContentDigestMismatch,
    /// The record is an outgoing copy whose original file name is withheld
    /// by policy ([`FileRecord::scrubbed`]): it goes into exports and shares
    /// only, never back into the library.
    #[error("an outgoing copy with the original file name withheld cannot be saved")]
    NamesWithheld,
}

/// The file name extension: what follows the last dot, unless the dot starts
/// the name or ends it.
pub fn extension_of(name: &str) -> Option<&str> {
    let (stem, extension) = name.rsplit_once('.')?;
    if stem.is_empty() || extension.is_empty() {
        return None;
    }
    Some(extension)
}

/// [`extension_of`] over the raw bytes of a name, which need not be UTF-8.
pub fn extension_of_bytes(name: &[u8]) -> Option<&[u8]> {
    let dot = name.iter().rposition(|b| *b == b'.')?;
    let (stem, extension) = (&name[..dot], &name[dot + 1..]);
    if stem.is_empty() || extension.is_empty() {
        return None;
    }
    Some(extension)
}

/// The display name of the API's one safe transformation (API-S-005):
/// control characters removed, `/` and `\` replaced by `_`, surrounding
/// spaces trimmed, at most 1,024 bytes kept, and `file` for a name left
/// empty. The metadata projection and the API's listings and downloads use
/// this one function so they cannot diverge.
pub fn safe_display_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| !c.is_control())
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect();
    let mut kept = String::new();
    for c in cleaned.trim_matches(' ').chars() {
        if kept.len() + c.len_utf8() > MAX_NAME_BYTES {
            break;
        }
        kept.push(c);
    }
    let kept = kept.trim_matches(' ');
    if kept.is_empty() {
        return "file".to_string();
    }
    kept.to_string()
}

/// The extension a stored original keeps: only 1 to 16 ASCII letters or
/// digits, so no source name can become an OCFL path (ADR 038, API-S-026).
pub fn stored_extension(name: &str) -> Option<&str> {
    let extension = extension_of(name)?;
    let fits =
        (1..=16).contains(&extension.len()) && extension.bytes().all(|b| b.is_ascii_alphanumeric());
    fits.then_some(extension)
}

/// The record's file name beside the stored original: `<file-uuid>.metadata.md`.
pub fn record_name(file_uuid: &str) -> String {
    format!("{file_uuid}.metadata.md")
}

/// The metadata record of one file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileRecord {
    /// The owner's exact original file name.
    original_name: Raw,
    /// The normalisation form of the original name.
    name_normalization: NameForm,
    /// The name shown everywhere; initially the original name.
    display_name: String,
    /// The file extension; `None` when the name has none.
    extension: Option<String>,
    /// The format label.
    format: Option<String>,
    /// The media type.
    media_type: Option<String>,
    /// The append-only import entries, oldest first.
    imports: Vec<ImportEntry>,
    /// The descriptive statements and the Markdown body.
    statements: Record,
    /// Whether this is an outgoing copy with the header's names withheld by
    /// policy; `original_name` and `display_name` are then empty.
    names_withheld: bool,
}

/// The read-only projection of a record for the metadata routes (API-S-027).
/// It has no absolute path, import root, host name or account.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FileFacts {
    /// The original file name exactly as reported.
    pub original_name: Raw,
    /// The name shown everywhere, after the safe transformation of API-S-005
    /// ([`safe_display_name`]).
    pub display_name: String,
    /// The file extension.
    pub extension: Option<String>,
    /// The format label.
    pub format: Option<String>,
    /// The media type.
    pub media_type: Option<String>,
    /// The content size in bytes.
    pub size: u64,
    /// The content's SHA-256, lowercase hex.
    pub sha256: String,
    /// One summary per acquisition, oldest first.
    pub imports: Vec<ImportFacts>,
}

/// The summary of one acquisition in a [`FileFacts`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ImportFacts {
    /// When the import finished (RFC 3339 UTC).
    pub imported_at: String,
    /// How the file reached FileSquid.
    pub method: ImportMethod,
    /// Copied or referenced.
    pub storage: ByteStorage,
    /// The path relative to the import root, when known.
    pub relative_path: Option<Raw>,
    /// The source volume's name, when known.
    pub volume_name: Option<Raw>,
    /// The OCFL object that holds the content.
    pub object_id: String,
    /// The OCFL version that holds the content.
    pub object_version: String,
}

impl FileRecord {
    /// A record for a newly imported file. The display name starts as the
    /// original name (a non-UTF-8 name is shown with U+FFFD for invalid
    /// bytes; the exact bytes stay in `original_name`).
    ///
    /// # Errors
    ///
    /// A [`RecordError`] when a name, the extension, the format, the media
    /// type or the entry is invalid.
    pub fn new(
        original_name: Raw,
        name_normalization: NameForm,
        extension: Option<String>,
        format: Option<String>,
        media_type: Option<String>,
        first_import: ImportEntry,
    ) -> Result<Self, RecordError> {
        let display_name = original_name.display_lossy();
        let record = Self {
            original_name,
            name_normalization,
            display_name,
            extension,
            format,
            media_type,
            imports: vec![first_import],
            statements: Record::default(),
            names_withheld: false,
        };
        record.validate()?;
        Ok(record)
    }

    /// The owner's exact original file name; empty in an outgoing copy whose
    /// names are withheld ([`names_withheld`](Self::names_withheld)).
    pub fn original_name(&self) -> &Raw {
        &self.original_name
    }

    /// Whether this is an outgoing copy with the original file name withheld
    /// by policy: in the header, or as any entry's source file name. The
    /// `ocfl` store refuses to save such a record.
    pub fn names_withheld(&self) -> bool {
        self.names_withheld
            || self
                .imports
                .iter()
                .any(|entry| entry.source.name_withheld())
    }

    /// The outgoing copy of the record for an export or share (MD-S-013):
    /// every import entry scrubbed of `classes`
    /// ([`ImportEntry::scrubbed`]) and, when `classes` holds
    /// [`IdentityClass::OriginalName`], the header's original name and
    /// display name withheld by policy. The extension, format, media type
    /// and statements stay. The copy renders and reads back, but the store
    /// refuses to save it.
    #[must_use]
    pub fn scrubbed(&self, classes: &[IdentityClass]) -> FileRecord {
        let mut copy = self.clone();
        copy.imports = self.imports.iter().map(|e| e.scrubbed(classes)).collect();
        if classes.contains(&IdentityClass::OriginalName) {
            copy.original_name = Raw::from_bytes(b"");
            copy.display_name = String::new();
            copy.names_withheld = true;
        }
        copy
    }

    /// The normalisation form of the original name.
    pub fn name_normalization(&self) -> NameForm {
        self.name_normalization
    }

    /// The name shown everywhere (the original name until a rename); empty
    /// in an outgoing copy whose names are withheld.
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// The file extension, when the name has one.
    pub fn extension(&self) -> Option<&str> {
        self.extension.as_deref()
    }

    /// The format label, when known.
    pub fn format(&self) -> Option<&str> {
        self.format.as_deref()
    }

    /// The media type, when known.
    pub fn media_type(&self) -> Option<&str> {
        self.media_type.as_deref()
    }

    /// The import entries, oldest first.
    pub fn imports(&self) -> &[ImportEntry] {
        &self.imports
    }

    /// The descriptive statements and body.
    pub fn statements(&self) -> &Record {
        &self.statements
    }

    /// The descriptive statements and body, for editing. The header and the
    /// import entries are not reachable through it.
    pub fn statements_mut(&mut self) -> &mut Record {
        &mut self.statements
    }

    /// This record with its statements replaced; the header and every import
    /// entry stay.
    #[must_use]
    pub fn with_statements(mut self, statements: Record) -> Self {
        self.statements = statements;
        self
    }

    /// Rename: change the display name only.
    ///
    /// # Errors
    ///
    /// [`RecordError::InvalidDisplayName`] for an empty name, one over 1,024
    /// bytes, or one with `/` or NUL; [`RecordError::NamesWithheld`] for an
    /// outgoing copy whose names are withheld.
    pub fn set_display_name(&mut self, name: &str) -> Result<(), RecordError> {
        if self.names_withheld {
            return Err(RecordError::NamesWithheld);
        }
        if !valid_name(name.as_bytes()) {
            return Err(RecordError::InvalidDisplayName);
        }
        self.display_name = name.to_string();
        Ok(())
    }

    /// Append the entry of a re-import. Earlier entries stay as they are.
    ///
    /// # Errors
    ///
    /// [`RecordError::Import`] when the entry is invalid; nothing is added.
    pub fn append_import(&mut self, entry: ImportEntry) -> Result<(), RecordError> {
        entry.validate()?;
        self.imports.push(entry);
        Ok(())
    }

    /// Check the record's own rules: names, an extension that is the
    /// original name's, format, media type, at least one import entry, and
    /// every entry. In an outgoing copy whose names are withheld, the name
    /// rules and the extension's match with the name are not checked.
    ///
    /// # Errors
    ///
    /// The first rule that fails, as a [`RecordError`].
    pub fn validate(&self) -> Result<(), RecordError> {
        if !self.names_withheld {
            self.validate_names()?;
        }
        if self
            .extension
            .as_deref()
            .is_some_and(|e| !valid_extension(e))
        {
            return Err(RecordError::InvalidExtension);
        }
        if self.format.as_deref().is_some_and(|f| !valid_format(f)) {
            return Err(RecordError::InvalidFormat);
        }
        if self
            .media_type
            .as_deref()
            .is_some_and(|m| !valid_media_type(m))
        {
            return Err(RecordError::InvalidMediaType);
        }
        if self.imports.is_empty() {
            return Err(RecordError::NoImport);
        }
        self.imports.iter().try_for_each(|e| Ok(e.validate()?))
    }

    /// The name rules: valid original and display names, and an extension
    /// that is the original name's.
    fn validate_names(&self) -> Result<(), RecordError> {
        if !valid_name(self.original_name.as_bytes()) {
            return Err(RecordError::InvalidName);
        }
        if !valid_name(self.display_name.as_bytes()) {
            return Err(RecordError::InvalidDisplayName);
        }
        let named = extension_of_bytes(self.original_name.as_bytes())
            .and_then(|e| std::str::from_utf8(e).ok());
        if self.extension.as_deref() != named {
            return Err(RecordError::InvalidExtension);
        }
        Ok(())
    }

    /// The append-only rule (ADR 038 Decision 5): this record, saved over
    /// `base`, keeps the original name and every existing import entry, and
    /// only adds entries. Statements and the display name may change.
    ///
    /// # Errors
    ///
    /// [`RecordError::OriginalNameChanged`], [`RecordError::ImportEntryChanged`]
    /// or [`RecordError::ImportEntryRemoved`].
    pub fn check_extends(&self, base: &FileRecord) -> Result<(), RecordError> {
        if self.original_name != base.original_name {
            return Err(RecordError::OriginalNameChanged);
        }
        for (index, existing) in base.imports.iter().enumerate() {
            match self.imports.get(index) {
                Some(entry) if entry == existing => {}
                Some(_) => return Err(RecordError::ImportEntryChanged { index }),
                None => return Err(RecordError::ImportEntryRemoved),
            }
        }
        Ok(())
    }

    /// Check that the entries from `first_new` on cite `object_id` and, for a
    /// managed import, the stored original at `logical_path`, and that the
    /// record, saved at `record_path`, would not replace that original.
    ///
    /// # Errors
    ///
    /// [`RecordError::BoundToOtherObject`], [`RecordError::BoundToOtherPath`]
    /// or [`RecordError::RecordAtContentPath`].
    pub fn check_binding(
        &self,
        first_new: usize,
        object_id: &str,
        logical_path: &str,
        record_path: &str,
    ) -> Result<(), RecordError> {
        for entry in self.imports.iter().skip(first_new) {
            let cited = &entry.acquisition;
            if cited.object_id != object_id {
                return Err(RecordError::BoundToOtherObject);
            }
            let stored = cited.storage == ByteStorage::Copied;
            if stored && cited.logical_path.as_deref() != Some(logical_path) {
                return Err(RecordError::BoundToOtherPath);
            }
            if stored && logical_path == record_path {
                return Err(RecordError::RecordAtContentPath);
            }
        }
        Ok(())
    }

    /// The read-only projection for the metadata routes (API-S-027).
    pub fn facts(&self) -> FileFacts {
        let first = &self.imports[0].acquisition;
        FileFacts {
            original_name: self.original_name.clone(),
            display_name: safe_display_name(&self.display_name),
            extension: self.extension.clone(),
            format: self.format.clone(),
            media_type: self.media_type.clone(),
            size: first.size,
            sha256: first.sha256.clone(),
            imports: self.imports.iter().map(import_facts).collect(),
        }
    }

    /// Render the record as `<file-uuid>.metadata.md` text.
    pub fn to_markdown(&self) -> String {
        markdown::render(&self.statements, &self.header(), &self.import_block())
    }

    /// Parse the text of a `<file-uuid>.metadata.md`.
    ///
    /// # Errors
    ///
    /// [`RecordError::UnsupportedSchema`] for text that is not a
    /// `filesquid.original.v1` record, [`RecordError::MissingField`],
    /// [`RecordError::Malformed`], or any validation failure.
    pub fn from_markdown(text: &str) -> Result<Self, RecordError> {
        let front = front_matter(text).ok_or(RecordError::UnsupportedSchema)?;
        let header = Header::read(front)?;
        let record = Self {
            original_name: header.original_name,
            name_normalization: header.name_normalization,
            display_name: header.display_name,
            extension: header.extension,
            format: header.format,
            media_type: header.media_type,
            imports: read_imports(front)?,
            statements: markdown::from_markdown(text),
            names_withheld: header.names_withheld,
        };
        record.validate()?;
        Ok(record)
    }

    /// The schema and header lines.
    fn header(&self) -> String {
        let mut out = format!("schema: {SCHEMA}\n");
        let withheld = Fact::<()>::unavailable(Reason::WithheldByPolicy);
        if self.names_withheld {
            push_json(&mut out, "original_name", &withheld);
        } else {
            push_json(&mut out, "original_name", &self.original_name);
        }
        push_json(&mut out, "name_normalization", &self.name_normalization);
        if self.names_withheld {
            push_json(&mut out, "display_name", &withheld);
        } else {
            push_json(&mut out, "display_name", &self.display_name);
        }
        push_json(&mut out, "extension", &self.extension);
        push_json(&mut out, "format", &self.format);
        push_json(&mut out, "media_type", &self.media_type);
        out
    }

    /// The `import:` list, one JSON entry per line.
    fn import_block(&self) -> String {
        let mut out = String::from("import:\n");
        for entry in &self.imports {
            out.push_str("  - ");
            out.push_str(&to_json(entry));
            out.push('\n');
        }
        out
    }
}

/// The summary of one entry for the projection.
fn import_facts(entry: &ImportEntry) -> ImportFacts {
    let a = &entry.acquisition;
    ImportFacts {
        imported_at: a.finished_at.clone(),
        method: a.method,
        storage: a.storage,
        relative_path: entry.source.relative_path.as_value().cloned(),
        volume_name: entry.volume.name.as_value().cloned(),
        object_id: a.object_id.clone(),
        object_version: a.object_version.clone(),
    }
}

/// One header line `key: <json>`.
fn push_json<T: Serialize>(out: &mut String, key: &str, value: &T) {
    out.push_str(key);
    out.push_str(": ");
    out.push_str(&to_json(value));
    out.push('\n');
}

/// `value` as one line of JSON. The record types serialize without failing.
fn to_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_string())
}

/// The front matter between the opening `---` and the closing `---`.
fn front_matter(text: &str) -> Option<&str> {
    let rest = text.trim_start().strip_prefix("---")?;
    let rest = rest.strip_prefix('\n')?;
    rest.split_once("\n---").map(|(front, _)| front)
}

/// The parsed header lines.
struct Header {
    /// `original_name`.
    original_name: Raw,
    /// `name_normalization`.
    name_normalization: NameForm,
    /// `display_name`.
    display_name: String,
    /// `extension`.
    extension: Option<String>,
    /// `format`.
    format: Option<String>,
    /// `media_type`.
    media_type: Option<String>,
    /// Whether `original_name` and `display_name` are withheld by policy.
    names_withheld: bool,
}

impl Header {
    /// Read and check the schema and header lines of `front`.
    fn read(front: &str) -> Result<Self, RecordError> {
        let lines: Vec<&str> = front.lines().filter(|l| !l.starts_with(' ')).collect();
        let schema = lines.iter().find_map(|l| l.strip_prefix("schema: "));
        if schema.map(str::trim) != Some(SCHEMA) {
            return Err(RecordError::UnsupportedSchema);
        }
        let value = |key: &'static str| -> Result<&str, RecordError> {
            let prefix = format!("{key}: ");
            let line = lines.iter().find_map(|l| l.strip_prefix(prefix.as_str()));
            line.ok_or(RecordError::MissingField(key))
        };
        let (original_name, display_name, names_withheld) = read_names(
            parse(value(HEADER_KEYS[0])?)?,
            parse(value(HEADER_KEYS[2])?)?,
        )?;
        Ok(Self {
            original_name,
            name_normalization: parse(value(HEADER_KEYS[1])?)?,
            display_name,
            extension: parse(value(HEADER_KEYS[3])?)?,
            format: parse(value(HEADER_KEYS[4])?)?,
            media_type: parse(value(HEADER_KEYS[5])?)?,
            names_withheld,
        })
    }
}

/// The header's two names: both known, or both withheld by policy (an
/// outgoing copy, [`FileRecord::scrubbed`]); anything else is malformed.
fn read_names(
    original: Fact<Raw>,
    display: Fact<String>,
) -> Result<(Raw, String, bool), RecordError> {
    let withheld = withheld_by_policy(&original) && withheld_by_policy(&display);
    match (original, display) {
        (Fact::Value(original), Fact::Value(display)) => Ok((original, display, false)),
        _ if withheld => Ok((Raw::from_bytes(b""), String::new(), true)),
        _ => Err(RecordError::Malformed(
            "the original and display names must be known or both withheld by policy",
        )),
    }
}

/// Whether `fact` is listed as withheld by the outgoing disclosure policy.
fn withheld_by_policy<T>(fact: &Fact<T>) -> bool {
    matches!(fact, Fact::Unavailable(u) if u.unavailable == Reason::WithheldByPolicy)
}

/// Parse one header value.
fn parse<T: for<'de> Deserialize<'de>>(json: &str) -> Result<T, RecordError> {
    serde_json::from_str(json).map_err(|_| RecordError::Malformed("a header value is not valid"))
}

/// Read the `import:` list of `front`: the `  - <json>` lines after `import:`.
fn read_imports(front: &str) -> Result<Vec<ImportEntry>, RecordError> {
    let lines = front.lines().skip_while(|l| *l != "import:").skip(1);
    let mut entries = Vec::new();
    for line in lines {
        let json = line.strip_prefix("  - ").ok_or(RecordError::Malformed(
            "a line after `import:` is not an entry",
        ))?;
        let entry = serde_json::from_str(json)
            .map_err(|_| RecordError::Malformed("an import entry is not valid"))?;
        entries.push(entry);
    }
    Ok(entries)
}

/// A name of 1 to 1,024 bytes without `/` or NUL.
fn valid_name(name: &[u8]) -> bool {
    (1..=MAX_NAME_BYTES).contains(&name.len()) && !name.contains(&b'/') && !name.contains(&0)
}

/// An extension of 1 to 255 bytes without `/` or NUL.
fn valid_extension(extension: &str) -> bool {
    extension.len() <= MAX_LABEL_BYTES && valid_name(extension.as_bytes())
}

/// A format label of at most 255 bytes without control characters.
fn valid_format(format: &str) -> bool {
    format.len() <= MAX_LABEL_BYTES && !format.chars().any(char::is_control)
}

/// `type/subtype`, each an RFC 6838 restricted name.
fn valid_media_type(media_type: &str) -> bool {
    let Some((kind, subtype)) = media_type.split_once('/') else {
        return false;
    };
    restricted_name(kind) && restricted_name(subtype)
}

/// An RFC 6838 restricted name: a letter or digit, then up to 126 of
/// letters, digits and `!#$&-^_.+`.
fn restricted_name(name: &str) -> bool {
    let mut chars = name.chars();
    let first = chars.next().is_some_and(|c| c.is_ascii_alphanumeric());
    let rest =
        name.len() <= 127 && chars.all(|c| c.is_ascii_alphanumeric() || "!#$&-^_.+".contains(c));
    first && rest
}
