//! Caller-supplied fetcher for remote data (SRS-F-054, C2PA-S-008).
//!
//! Remote manifest stores, OCSP and CRL revocation data and RFC 3161
//! timestamp responses are fetched only through a [`Fetcher`] the caller
//! supplies. The engine has no network client. [`OfflineFetcher`] refuses
//! every request and is what an offline or privacy-first host passes.
//! [`fetch_bounded`] enforces each request's byte limit on the response.

use std::future::Future;

/// Default response limit for a fetch: 16 MiB.
pub const DEFAULT_MAX_FETCH_BYTES: u64 = 16 * 1024 * 1024;

/// Why the engine wants a remote resource, so the caller can apply policy
/// (for example allow revocation checks but refuse remote manifests).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FetchPurpose {
    /// A manifest store referenced by URL from an asset.
    RemoteManifest,
    /// OCSP or CRL revocation data for a certificate.
    Revocation,
    /// An RFC 3161 time-stamp authority request.
    Timestamp,
}

/// One request the engine asks the caller to perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchRequest {
    /// Absolute URL.
    pub url: String,
    /// Why the engine needs it.
    pub purpose: FetchPurpose,
    /// Request body; `None` means an HTTP GET, `Some` a POST.
    pub body: Option<Vec<u8>>,
    /// Media type of `body`.
    pub content_type: Option<String>,
    /// Largest response body the engine accepts. Fetchers SHOULD stop reading
    /// beyond it; [`fetch_bounded`] rejects a larger body either way.
    pub max_bytes: u64,
}

impl FetchRequest {
    /// A GET request for `url`.
    pub fn get(url: impl Into<String>, purpose: FetchPurpose, max_bytes: u64) -> Self {
        Self {
            url: url.into(),
            purpose,
            body: None,
            content_type: None,
            max_bytes,
        }
    }

    /// A POST request for `url` carrying `body` of media type `content_type`.
    pub fn post(
        url: impl Into<String>,
        purpose: FetchPurpose,
        content_type: impl Into<String>,
        body: Vec<u8>,
        max_bytes: u64,
    ) -> Self {
        Self {
            url: url.into(),
            purpose,
            body: Some(body),
            content_type: Some(content_type.into()),
            max_bytes,
        }
    }
}

/// A successful response from the caller's fetcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchResponse {
    /// Media type reported by the server, if any.
    pub content_type: Option<String>,
    /// Response body.
    pub body: Vec<u8>,
}

/// Errors from a fetch.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FetchError {
    /// The caller does not allow network access for this request.
    #[error("network access refused for {url}")]
    Offline {
        /// The refused URL.
        url: String,
    },
    /// The response exceeded the request's `max_bytes`.
    #[error("response from {url} is {actual} bytes, over the {max_bytes}-byte limit")]
    TooLarge {
        /// The requested URL.
        url: String,
        /// The request's limit.
        max_bytes: u64,
        /// The response size.
        actual: u64,
    },
    /// The caller's fetcher failed.
    #[error("fetch of {url} failed: {reason}")]
    Failed {
        /// The requested URL.
        url: String,
        /// The caller's description.
        reason: String,
    },
}

/// Performs network requests on the engine's behalf. Implemented by the host.
pub trait Fetcher {
    /// Perform `request` and return the response body.
    fn fetch(
        &self,
        request: &FetchRequest,
    ) -> impl Future<Output = Result<FetchResponse, FetchError>>;
}

/// A fetcher that refuses every request. Use it to run fully offline.
#[derive(Debug, Clone, Copy, Default)]
pub struct OfflineFetcher;

impl Fetcher for OfflineFetcher {
    async fn fetch(&self, request: &FetchRequest) -> Result<FetchResponse, FetchError> {
        Err(FetchError::Offline {
            url: request.url.clone(),
        })
    }
}

/// Perform `request` through `fetcher` and reject a body over `max_bytes`.
///
/// Every engine fetch goes through this function. Refusals and oversize
/// responses are logged with `tracing` at `warn` level (URL and sizes only).
pub async fn fetch_bounded<F: Fetcher + ?Sized>(
    fetcher: &F,
    request: &FetchRequest,
) -> Result<FetchResponse, FetchError> {
    let response = fetcher.fetch(request).await.inspect_err(|err| {
        tracing::warn!(url = %request.url, purpose = ?request.purpose, error = %err, "c2pa fetch failed");
    })?;
    let actual = response.body.len() as u64;
    if actual > request.max_bytes {
        tracing::warn!(url = %request.url, actual, max_bytes = request.max_bytes, "c2pa fetch over limit");
        return Err(FetchError::TooLarge {
            url: request.url.clone(),
            max_bytes: request.max_bytes,
            actual,
        });
    }
    Ok(response)
}
