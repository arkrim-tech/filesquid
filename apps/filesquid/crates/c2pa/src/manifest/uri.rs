//! `self#jumbf` URIs and their resolution (C2PA 2.4 section 8.4.2.1).
//!
//! A claim refers to its signature and assertions, and ingredients refer to
//! other manifests, with URIs such as `self#jumbf=c2pa.assertions/c2pa.actions`
//! (relative to the current manifest) or
//! `self#jumbf=/c2pa/urn:c2pa:.../c2pa.signature` (relative to the store).
//! Each path segment is the label of a child superbox.

use super::ManifestStore;
use crate::jumbf::Superbox;

/// The prefix of every URI that points into the same manifest store.
const SELF_PREFIX: &str = "self#jumbf=";

/// Most path segments in a URI; a store cannot nest deeper than its box
/// depth limit, which defaults to the same value.
pub const MAX_URI_SEGMENTS: usize = 16;

/// A parsed `self#jumbf` URI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumbfUri {
    /// Whether the path starts at the store (`/` first).
    absolute: bool,
    /// The labels along the path.
    segments: Vec<String>,
}

/// Why a URI is not a usable `self#jumbf` URI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum UriError {
    /// It does not start with `self#jumbf=`.
    #[error("not a self#jumbf URI")]
    NotSelfJumbf,
    /// The path is empty.
    #[error("the JUMBF URI path is empty")]
    Empty,
    /// The path contains `..`, which the specification forbids.
    #[error("the JUMBF URI contains '..'")]
    ParentReference,
    /// The path has an empty segment (`//` or a trailing `/`).
    #[error("the JUMBF URI has an empty path segment")]
    EmptySegment,
    /// More than [`MAX_URI_SEGMENTS`] segments.
    #[error("the JUMBF URI has more than {MAX_URI_SEGMENTS} segments")]
    TooManySegments,
}

impl JumbfUri {
    /// Parse `text`.
    pub fn parse(text: &str) -> Result<Self, UriError> {
        let path = text
            .strip_prefix(SELF_PREFIX)
            .ok_or(UriError::NotSelfJumbf)?;
        if path.is_empty() {
            return Err(UriError::Empty);
        }
        if path.contains("..") {
            return Err(UriError::ParentReference);
        }
        let absolute = path.starts_with('/');
        let relative = path.strip_prefix('/').unwrap_or(path);
        let segments: Vec<&str> = relative.split('/').take(MAX_URI_SEGMENTS + 1).collect();
        if segments.len() > MAX_URI_SEGMENTS {
            return Err(UriError::TooManySegments);
        }
        if segments.iter().any(|s| s.is_empty()) {
            return Err(UriError::EmptySegment);
        }
        Ok(Self {
            absolute,
            segments: segments.into_iter().map(str::to_string).collect(),
        })
    }

    /// Whether the path starts at the store.
    pub fn is_absolute(&self) -> bool {
        self.absolute
    }

    /// The labels along the path; for an absolute URI the first is the
    /// store's label (`c2pa`).
    pub fn segments(&self) -> &[String] {
        &self.segments
    }
}

/// Why a URI did not resolve to exactly one superbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ResolveError {
    /// No superbox has a label on the path.
    #[error("no box matches the JUMBF URI")]
    NotFound,
    /// Two sibling superboxes share a label on the path.
    #[error("more than one box matches the JUMBF URI")]
    Ambiguous,
    /// The manifest index is out of range.
    #[error("no manifest has that index")]
    NoSuchManifest,
}

impl ManifestStore {
    /// Resolve `uri`, found in the manifest at index `manifest` (into
    /// [`ManifestStore::manifests`]), to the superbox it names.
    pub fn resolve(&self, uri: &JumbfUri, manifest: usize) -> Result<&Superbox, ResolveError> {
        let entry = self
            .manifests()
            .get(manifest)
            .ok_or(ResolveError::NoSuchManifest)?;
        if !uri.is_absolute() {
            let start = self
                .root()
                .superboxes()
                .find(|sb| sb.range == entry.range)
                .ok_or(ResolveError::NoSuchManifest)?;
            return walk(start, uri.segments());
        }
        let (first, rest) = uri.segments().split_first().ok_or(ResolveError::NotFound)?;
        if self.root().label() != Some(first.as_str()) {
            return Err(ResolveError::NotFound);
        }
        walk(self.root(), rest)
    }
}

/// Follow `segments` down from `start`, one child label per segment.
fn walk<'s>(start: &'s Superbox, segments: &[String]) -> Result<&'s Superbox, ResolveError> {
    let mut node = start;
    for segment in segments {
        let mut matches = node
            .superboxes()
            .filter(|sb| sb.label() == Some(segment.as_str()));
        node = matches.next().ok_or(ResolveError::NotFound)?;
        if matches.next().is_some() {
            return Err(ResolveError::Ambiguous);
        }
    }
    Ok(node)
}
