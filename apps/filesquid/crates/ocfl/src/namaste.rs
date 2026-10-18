//! NAMASTE conformance declarations (OCFL 1.1 sections 3.2 and 4.2).
//!
//! An OCFL object root holds exactly one `0=ocfl_object_1.1` file and a
//! storage root one `0=ocfl_1.1` file. The file's text is its `dvalue` (the
//! part after `0=`) followed by one newline.

use crate::inventory::FormatError;

/// Most `0=` files directly in one root that a reader looks at before it
/// refuses the root as not having exactly one declaration. A root needs one,
/// so this bound only stops an untrusted root from making a reader hold many.
pub(crate) const MAX_DECLARATION_ENTRIES: usize = 16;

/// An OCFL specification version this engine recognises.
///
/// The engine reads and writes 1.1. It recognises 1.0 so that an object
/// whose earlier versions were written under 1.0 can be identified; reading
/// whole 1.0 objects is out of scope (OCFL-S-011).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SpecVersion {
    /// OCFL 1.0.
    V1_0,
    /// OCFL 1.1, the version the engine writes.
    V1_1,
}

impl SpecVersion {
    /// The version number as OCFL writes it, such as `1.1`.
    pub fn number(self) -> &'static str {
        match self {
            Self::V1_0 => "1.0",
            Self::V1_1 => "1.1",
        }
    }

    /// The inventory `type` URI for this version.
    pub fn inventory_type(self) -> &'static str {
        match self {
            Self::V1_0 => "https://ocfl.io/1.0/spec/#inventory",
            Self::V1_1 => "https://ocfl.io/1.1/spec/#inventory",
        }
    }

    /// The version whose inventory `type` URI is `uri`.
    pub fn from_inventory_type(uri: &str) -> Option<Self> {
        [Self::V1_0, Self::V1_1]
            .into_iter()
            .find(|spec| spec.inventory_type() == uri)
    }

    /// The version whose number is `number`.
    fn from_number(number: &str) -> Option<Self> {
        [Self::V1_0, Self::V1_1]
            .into_iter()
            .find(|spec| spec.number() == number)
    }
}

/// Which root a declaration marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeclarationKind {
    /// An OCFL object root (`0=ocfl_object_<version>`).
    Object,
    /// An OCFL storage root (`0=ocfl_<version>`).
    StorageRoot,
}

/// One NAMASTE conformance declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Declaration {
    /// The root it marks.
    pub kind: DeclarationKind,
    /// The OCFL version it declares.
    pub spec: SpecVersion,
}

impl Declaration {
    /// The object-root declaration for `spec`.
    pub fn object(spec: SpecVersion) -> Self {
        Self {
            kind: DeclarationKind::Object,
            spec,
        }
    }

    /// The storage-root declaration for `spec`.
    pub fn storage_root(spec: SpecVersion) -> Self {
        Self {
            kind: DeclarationKind::StorageRoot,
            spec,
        }
    }

    /// The NAMASTE `dvalue`, such as `ocfl_object_1.1`.
    pub fn dvalue(&self) -> String {
        match self.kind {
            DeclarationKind::Object => format!("ocfl_object_{}", self.spec.number()),
            DeclarationKind::StorageRoot => format!("ocfl_{}", self.spec.number()),
        }
    }

    /// The declaration's file name, such as `0=ocfl_object_1.1`.
    pub fn file_name(&self) -> String {
        format!("0={}", self.dvalue())
    }

    /// The declaration's exact file contents: the `dvalue` and a newline.
    pub fn contents(&self) -> String {
        format!("{}\n", self.dvalue())
    }

    /// The declaration a file called `name` makes, or `None` when `name` is
    /// not an OCFL object or storage-root declaration of a known version.
    pub fn from_file_name(name: &str) -> Option<Self> {
        let dvalue = name.strip_prefix("0=")?;
        if let Some(number) = dvalue.strip_prefix("ocfl_object_") {
            return SpecVersion::from_number(number).map(Self::object);
        }
        let number = dvalue.strip_prefix("ocfl_")?;
        SpecVersion::from_number(number).map(Self::storage_root)
    }

    /// Check a declaration file's bytes.
    ///
    /// # Errors
    ///
    /// [`FormatError::Invalid`] with E007 (object) or E080 (storage root)
    /// unless the bytes are exactly [`contents`](Self::contents).
    pub fn check_contents(&self, bytes: &[u8]) -> Result<(), FormatError> {
        if bytes == self.contents().as_bytes() {
            return Ok(());
        }
        let code = match self.kind {
            DeclarationKind::Object => "E007",
            DeclarationKind::StorageRoot => "E080",
        };
        Err(FormatError::invalid(
            code,
            self.file_name(),
            format!("the contents must be {:?}", self.contents()),
        ))
    }
}
