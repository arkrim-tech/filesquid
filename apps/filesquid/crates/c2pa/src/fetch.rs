//! Caller-supplied fetcher for remote data (SRS-F-054, C2PA-S-008).
//!
//! Remote manifest stores, OCSP and CRL revocation data and RFC 3161
//! timestamp responses are fetched only through a [`Fetcher`] the caller
//! supplies. The engine has no network client. [`OfflineFetcher`] refuses
//! every request and is what an offline or privacy-first host passes.
//! Fetchers enforce each request's byte limit while they read (see
//! [`BoundedBody`]); [`fetch_bounded`] checks it again on the response.

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
    /// Largest response body the engine accepts. A [`Fetcher`] MUST stop
    /// reading once the body would exceed it and return
    /// [`FetchError::TooLarge`] (use [`BoundedBody`] while streaming), so an
    /// oversized response is never buffered. [`fetch_bounded`] checks again
    /// as defence in depth.
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
        /// Bytes received when the limit was hit.
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

impl FetchError {
    /// A fixed, non-sensitive category name for logs and metrics.
    pub fn category(&self) -> &'static str {
        match self {
            Self::Offline { .. } => "offline",
            Self::TooLarge { .. } => "too_large",
            Self::Failed { .. } => "failed",
        }
    }
}

/// Performs network requests on the engine's behalf. Implemented by the host.
///
/// Contract: an implementation MUST enforce `request.max_bytes` while it reads
/// the response (for example by feeding chunks into a [`BoundedBody`]) and
/// MUST NOT buffer more than `max_bytes` bytes of body.
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

/// A response body buffer that refuses to grow past a request's `max_bytes`.
///
/// Fetcher implementations push each received chunk with
/// [`BoundedBody::push`] and stop reading at the first error, so memory use
/// is capped at `max_bytes` whatever the server sends.
#[derive(Debug, Clone)]
pub struct BoundedBody {
    /// URL, carried into the error.
    url: String,
    /// The cap.
    max_bytes: u64,
    /// Bytes accepted so far.
    bytes: Vec<u8>,
}

impl BoundedBody {
    /// An empty body bounded by `request.max_bytes`.
    pub fn new(request: &FetchRequest) -> Self {
        Self {
            url: request.url.clone(),
            max_bytes: request.max_bytes,
            bytes: Vec::new(),
        }
    }

    /// Append `chunk`, or fail with [`FetchError::TooLarge`] without keeping
    /// any of `chunk` when the body would exceed the cap.
    pub fn push(&mut self, chunk: &[u8]) -> Result<(), FetchError> {
        let actual = self.bytes.len() as u64 + chunk.len() as u64;
        if actual > self.max_bytes {
            return Err(FetchError::TooLarge {
                url: self.url.clone(),
                max_bytes: self.max_bytes,
                actual,
            });
        }
        self.bytes.extend_from_slice(chunk);
        Ok(())
    }

    /// Bytes accepted so far.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether no bytes were accepted.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// The accepted body.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// Placeholder logged instead of text that is not an absolute URL.
pub const UNPARSED_URL: &str = "<unparsed-url>";

/// Reduce `url` to `scheme://host[:port]` for logging, dropping userinfo,
/// path, query and fragment, which may carry credentials or signed tokens.
/// Text that is not an absolute URL becomes [`UNPARSED_URL`].
pub fn redacted_origin(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return UNPARSED_URL.to_string();
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit('@').next().unwrap_or("");
    let scheme_ok = !scheme.is_empty()
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c));
    if !scheme_ok || host.is_empty() {
        return UNPARSED_URL.to_string();
    }
    format!("{scheme}://{host}")
}

/// Perform `request` through `fetcher` and reject a body over `max_bytes`.
///
/// Every engine fetch goes through this function. A failure is logged with
/// `tracing` at `warn` level with the redacted origin, the purpose and the
/// error category only; never the full URL or the caller's error text.
pub async fn fetch_bounded<F: Fetcher + ?Sized>(
    fetcher: &F,
    request: &FetchRequest,
) -> Result<FetchResponse, FetchError> {
    let result = fetcher
        .fetch(request)
        .await
        .and_then(|response| check_size(request, response));
    if let Err(err) = &result {
        tracing::warn!(
            origin = %redacted_origin(&request.url),
            purpose = ?request.purpose,
            category = err.category(),
            "c2pa fetch failed"
        );
    }
    result
}

/// Reject `response` when its body exceeds `request.max_bytes`.
fn check_size(
    request: &FetchRequest,
    response: FetchResponse,
) -> Result<FetchResponse, FetchError> {
    let actual = response.body.len() as u64;
    if actual > request.max_bytes {
        return Err(FetchError::TooLarge {
            url: request.url.clone(),
            max_bytes: request.max_bytes,
            actual,
        });
    }
    Ok(response)
}
