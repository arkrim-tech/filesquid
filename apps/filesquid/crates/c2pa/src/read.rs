//! Reading manifest stores from caller-supplied bytes (C2PA-S-001,
//! C2PA-S-008).
//!
//! [`read_manifest_store`] reads a whole store, such as a sidecar `.c2pa`
//! file, from any [`ByteSource`]. [`fetch_remote_manifest_store`] reads a
//! store by remote reference (an XMP `dcterms:provenance` value or an HTTP
//! `Link` header, C2PA 2.4 section 15.5.3) through the caller's [`Fetcher`].
//! Neither touches the filesystem or the network itself.
//!
//! Every outcome is logged with `tracing`: a rejection at `warn` level with
//! a fixed error category only, never a URL, label or other text from the
//! input.

use crate::fetch::{fetch_bounded, FetchError, FetchPurpose, FetchRequest, Fetcher};
use crate::io::{read_exact_at, ByteIoError, ByteSource};
use crate::manifest::{ManifestStore, ParseLimits, StoreError};

/// Why a manifest store could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReadError {
    /// The byte source failed.
    #[error("byte source error: {0}")]
    Io(ByteIoError),
    /// The source is larger than [`ParseLimits::max_store_bytes`] (or than
    /// this platform can address). Nothing was read.
    #[error("the manifest store is {size} bytes, over the {max}-byte limit")]
    TooLarge {
        /// The source's reported size.
        size: u64,
        /// The limit applied.
        max: u64,
    },
    /// Memory for the store could not be reserved.
    #[error("could not reserve {size} bytes for the manifest store")]
    Allocation {
        /// The bytes requested.
        size: u64,
    },
    /// The caller's fetcher failed or refused the request
    /// (`manifest.inaccessible`). The message shows only the error category,
    /// because the URL came from the asset and may carry tokens.
    #[error("remote manifest store unavailable ({})", .0.category())]
    Fetch(FetchError),
    /// A remote reference that is not an `http` or `https` URL.
    #[error("remote manifest references must be http or https URLs")]
    UnsupportedUrl,
    /// A remote reference with a `#jumbf=` fragment, which points into an
    /// asset and needs that asset's format handler.
    #[error("the reference points into an asset; read the asset instead")]
    EmbeddedReference,
    /// The bytes are not a usable manifest store.
    #[error("{0}")]
    Store(StoreError),
}

impl ReadError {
    /// The C2PA 2.4 failure code for this error, or `None` where the
    /// specification defines none. A store that is referenced but cannot be
    /// fetched is `manifest.inaccessible` (section 15.5.3.1).
    pub fn status_code(&self) -> Option<&'static str> {
        match self {
            Self::Fetch(_) => Some("manifest.inaccessible"),
            _ => None,
        }
    }

    /// A fixed category name for logs and metrics.
    pub fn category(&self) -> &'static str {
        match self {
            Self::Io(_) => "io",
            Self::TooLarge { .. } => "too_large",
            Self::Allocation { .. } => "allocation",
            Self::Fetch(err) => err.category(),
            Self::UnsupportedUrl => "unsupported_url",
            Self::EmbeddedReference => "embedded_reference",
            Self::Store(err) => err.category(),
        }
    }
}

/// Read and parse the whole manifest store in `source`, such as a sidecar
/// `.c2pa` file. The source's size is checked against
/// [`ParseLimits::max_store_bytes`] before anything is read.
pub async fn read_manifest_store<S: ByteSource + ?Sized>(
    source: &mut S,
    limits: &ParseLimits,
) -> Result<ManifestStore, ReadError> {
    let result = match read_store_bytes(source, limits).await {
        Ok(bytes) => ManifestStore::from_bytes(bytes, limits).map_err(ReadError::Store),
        Err(err) => Err(err),
    };
    log_outcome("byte_source", &result);
    result
}

/// Fetch the manifest store at `url` through `fetcher` and parse it.
///
/// Only `http` and `https` URLs are requested; a URL with a `#jumbf=`
/// fragment points into an asset and is refused. The request is a GET with
/// [`FetchPurpose::RemoteManifest`] and `max_bytes` set to
/// [`ParseLimits::max_store_bytes`]. The response's media type is not
/// checked (the specification serves stores as `application/c2pa`, but
/// servers vary); the bytes must parse as a manifest store.
pub async fn fetch_remote_manifest_store<F: Fetcher + ?Sized>(
    fetcher: &F,
    url: &str,
    limits: &ParseLimits,
) -> Result<ManifestStore, ReadError> {
    let result = fetch_store(fetcher, url, limits).await;
    log_outcome("remote", &result);
    result
}

/// Read all of `source` into memory after checking its size.
async fn read_store_bytes<S: ByteSource + ?Sized>(
    source: &mut S,
    limits: &ParseLimits,
) -> Result<Vec<u8>, ReadError> {
    let size = source.size().await.map_err(ReadError::Io)?;
    let max = limits.max_store_bytes.min(isize::MAX as u64);
    if size > max {
        return Err(ReadError::TooLarge { size, max });
    }
    let len = usize::try_from(size).map_err(|_| ReadError::TooLarge { size, max })?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(len)
        .map_err(|_| ReadError::Allocation { size })?;
    bytes.resize(len, 0);
    read_exact_at(source, 0, &mut bytes)
        .await
        .map_err(ReadError::Io)?;
    Ok(bytes)
}

/// Check `url`, fetch it and parse the body.
async fn fetch_store<F: Fetcher + ?Sized>(
    fetcher: &F,
    url: &str,
    limits: &ParseLimits,
) -> Result<ManifestStore, ReadError> {
    check_remote_url(url)?;
    let request = FetchRequest::get(url, FetchPurpose::RemoteManifest, limits.max_store_bytes);
    let response = fetch_bounded(fetcher, &request)
        .await
        .map_err(ReadError::Fetch)?;
    ManifestStore::from_bytes(response.body, limits).map_err(ReadError::Store)
}

/// Accept only `http` and `https` URLs without a `#jumbf=` fragment.
fn check_remote_url(url: &str) -> Result<(), ReadError> {
    let Some((scheme, rest)) = url.split_once("://") else {
        return Err(ReadError::UnsupportedUrl);
    };
    let web = scheme.eq_ignore_ascii_case("https") || scheme.eq_ignore_ascii_case("http");
    if !web || rest.is_empty() {
        return Err(ReadError::UnsupportedUrl);
    }
    if url.contains("#jumbf=") {
        return Err(ReadError::EmbeddedReference);
    }
    Ok(())
}

/// Log a read outcome with counts or an error category only.
fn log_outcome(source: &'static str, result: &Result<ManifestStore, ReadError>) {
    match result {
        Ok(store) => tracing::debug!(
            source,
            manifests = store.manifests().len(),
            ignored = store.ignored().len(),
            "c2pa manifest store read"
        ),
        Err(err) => tracing::warn!(
            source,
            category = err.category(),
            "c2pa manifest store rejected"
        ),
    }
}
