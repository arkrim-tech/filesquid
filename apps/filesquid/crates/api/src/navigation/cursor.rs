//! The opaque paging position of the navigation listing (API-S-021).
//!
//! A cursor names the parent it was issued for (a short digest of the parent's handle, so one
//! parent's cursor is refused at another), and the last position scanned: its kind rank (`library`, `collection`,
//! `virtual_collection`, `recently_deleted`, `folder`, the fixed order of the listing) and its key (the catalog's
//! `order:id` for the first three, the folder's name for a folder). It is `c1.` and lowercase
//! hex, and decodes only when it is exactly what [`Position::encode`] writes.

use super::error::NavigationError;
use crate::library::PageKey;

/// Prefix of a version 1 cursor.
const CURSOR_PREFIX: &str = "c1.";

/// Longest cursor accepted, in bytes (bounds the decode work).
const MAX_CURSOR_BYTES: usize = 1024;

/// The kind of a child, in the fixed listing order (never a lexical or enum-derived sort).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Rank {
    Library = 0,
    Collection = 1,
    VirtualCollection = 2,
    Folder = 3,
    /// The repository's one Recently deleted node, after its libraries. Its rank is new, so
    /// cursors issued before it existed keep their meaning; ranks are compared only among the
    /// children of one parent, and no parent has both folders and this node.
    RecentlyDeleted = 4,
}

impl Rank {
    /// The rank a cursor spells.
    fn from_digit(digit: &str) -> Option<Self> {
        match digit {
            "0" => Some(Self::Library),
            "1" => Some(Self::Collection),
            "2" => Some(Self::VirtualCollection),
            "3" => Some(Self::Folder),
            "4" => Some(Self::RecentlyDeleted),
            _ => None,
        }
    }
}

/// Where a scan stopped: after the child with `key` among those of `rank`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Position {
    pub rank: Rank,
    pub key: String,
}

/// The short tag of the parent a cursor belongs to: the first 8 bytes of the SHA-256 of the
/// parent's handle as hex, or `-` for the repository level.
pub(crate) fn parent_tag(handle: Option<&str>) -> String {
    use sha2::{Digest, Sha256};
    match handle {
        None => "-".to_string(),
        Some(handle) => Sha256::digest(handle.as_bytes())[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    }
}

/// The refusal of a cursor this API did not issue.
fn invalid() -> NavigationError {
    NavigationError::invalid("The paging position is not valid. Start again.")
}

impl Position {
    /// The position after the catalog row `key`.
    pub fn of_row(rank: Rank, key: &PageKey) -> Self {
        Self {
            rank,
            key: format!("{}:{}", key.order, key.id),
        }
    }

    /// The position after the folder named `name`.
    pub fn of_folder(name: &str) -> Self {
        Self {
            rank: Rank::Folder,
            key: name.to_string(),
        }
    }

    /// The catalog key this position names, for a catalog rank.
    pub fn page_key(&self) -> Option<PageKey> {
        let (order, id) = self.key.split_once(':')?;
        Some(PageKey {
            order: order.parse().ok()?,
            id: id.to_string(),
        })
    }

    /// This position as an opaque cursor.
    ///
    /// # Errors
    ///
    /// `internal` when the cursor would pass the bound [`Self::decode`] enforces.
    pub fn encode(&self, parent: &str) -> Result<String, NavigationError> {
        let plain = format!("{}:{parent}:{}", self.rank as u8, self.key);
        let hex: String = plain.bytes().map(|b| format!("{b:02x}")).collect();
        let text = format!("{CURSOR_PREFIX}{hex}");
        if text.len() > MAX_CURSOR_BYTES {
            tracing::info!(target: super::LOG_TARGET, "page key too long for a cursor");
            return Err(NavigationError::internal());
        }
        Ok(text)
    }

    /// The position `text` names.
    ///
    /// # Errors
    ///
    /// `invalid_request` for any text [`Self::encode`] did not produce for this `parent`.
    pub fn decode(text: &str, parent: &str) -> Result<Self, NavigationError> {
        let position = Self::parse(text, parent).ok_or_else(invalid)?;
        match position.encode(parent) {
            Ok(canonical) if canonical == text => Ok(position),
            _ => Err(invalid()),
        }
    }

    /// The position the text spells, without the canonical-form check.
    fn parse(text: &str, parent: &str) -> Option<Self> {
        let hex = text
            .strip_prefix(CURSOR_PREFIX)
            .filter(|hex| text.len() <= MAX_CURSOR_BYTES && !hex.is_empty())?;
        let plain = decode_hex(hex)?;
        let (rank, rest) = plain.split_once(':')?;
        let (tag, key) = rest.split_once(':')?;
        if tag != parent {
            return None;
        }
        Some(Self {
            rank: Rank::from_digit(rank)?,
            key: key.to_string(),
        })
    }
}

/// Decode hex pairs into UTF-8 text (case is checked by the canonical re-encoding).
fn decode_hex(hex: &str) -> Option<String> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
        .collect();
    String::from_utf8(bytes?).ok()
}
