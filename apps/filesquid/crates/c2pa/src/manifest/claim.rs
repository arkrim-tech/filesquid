//! Claims: `c2pa.claim.v2` and the legacy `c2pa.claim` (C2PA 2.4 section
//! 10.2, required fields per section 15.6.2).
//!
//! [`Claim::from_cbor`] checks the fields the CDDL defines and keeps the
//! whole decoded map, so fields this crate does not interpret (for example
//! the deprecated `metadata`) are still available. Unknown fields are kept,
//! not rejected. The signature and every assertion reference must be a
//! `self#jumbf` URI ([`JumbfUri::parse`]); resolving them is validation's
//! job. A missing or mistyped field is a [`ClaimError`], which a validator
//! reports as `claim.malformed`.

use super::uri::JumbfUri;
use crate::cbor::CborValue;

/// Which claim map a claim uses, from its box label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimVersion {
    /// The deprecated `c2pa.claim` (`claim-map`). Validators still accept
    /// it; claim generators no longer write it.
    V1,
    /// `c2pa.claim.v2` (`claim-map-v2`).
    V2,
}

/// A hashed URI: a reference to a box in the same store and its hash
/// (C2PA 2.4 section 8.4.2.1). The hash is not checked here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashedUri {
    /// The `self#jumbf=` URI.
    pub url: String,
    /// The hash algorithm, when the reference names one; otherwise the
    /// claim's `alg` applies.
    pub alg: Option<String>,
    /// The expected hash.
    pub hash: Vec<u8>,
}

/// One `generator-info-map`: the software or hardware that made a claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratorInfo {
    /// Its name (required, non-empty).
    pub name: String,
    /// Its version, if given.
    pub version: Option<String>,
    /// The whole map, including `icon`, `operating_system`, `specVersion`
    /// and any entity-specific fields.
    pub map: CborValue,
}

/// A parsed claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    /// Which claim map this is.
    pub version: ClaimVersion,
    /// `instanceID`.
    pub instance_id: String,
    /// `signature`: the URI of the claim signature box.
    pub signature: String,
    /// `created_assertions` (v2), or `assertions` (v1, where every
    /// assertion is attributed to the signer).
    pub created_assertions: Vec<HashedUri>,
    /// `gathered_assertions` (v2 only); empty when absent.
    pub gathered_assertions: Vec<HashedUri>,
    /// `redacted_assertions`; empty when absent.
    pub redacted_assertions: Vec<String>,
    /// `claim_generator_info`: one map in v2, an optional array in v1.
    pub claim_generator_info: Vec<GeneratorInfo>,
    /// `claim_generator` (v1 only).
    pub claim_generator: Option<String>,
    /// `dc:format` (v1 only).
    pub format: Option<String>,
    /// `dc:title`.
    pub title: Option<String>,
    /// `alg`: the default hash algorithm.
    pub alg: Option<String>,
    /// `alg_soft`: the default soft-binding algorithm.
    pub alg_soft: Option<String>,
    /// The whole decoded claim map.
    pub map: CborValue,
}

/// A claim field that is missing or does not match the claim CDDL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("field {field}: {reason}")]
pub struct ClaimError {
    /// The field (`claim` for the claim map itself).
    pub field: &'static str,
    /// Which rule was broken.
    pub reason: &'static str,
}

/// Reason text for [`ClaimError`].
const NOT_A_MAP: &str = "a claim is a map with text keys";
/// Reason text for [`ClaimError`].
const MISSING_TEXT: &str = "required text field is missing";
/// Reason text for [`ClaimError`].
const NOT_TEXT: &str = "must be a text string";
/// Reason text for [`ClaimError`].
const EMPTY_TEXT: &str = "must not be empty";
/// Reason text for [`ClaimError`].
const MISSING_ARRAY: &str = "required array is missing";
/// Reason text for [`ClaimError`].
const BAD_ARRAY: &str = "must be a non-empty array";
/// Reason text for [`ClaimError`].
const BAD_HASHED_URI: &str = "each entry must be a map with a self#jumbf URI url, a byte-string hash and an optional non-empty text alg";
/// Reason text for [`ClaimError`].
const BAD_URI_LIST: &str = "each entry must be a self#jumbf URI";
/// Reason text for [`ClaimError`].
const BAD_SIGNATURE_URI: &str = "must be a self#jumbf URI";
/// Reason text for [`ClaimError`].
const MISSING_GENERATOR: &str = "required map is missing";
/// Reason text for [`ClaimError`].
const BAD_GENERATOR: &str = "must be a map with text keys, a non-empty text name, text version, operating_system and specVersion when present, and a hashed URI icon when present";

impl Claim {
    /// Read a claim of `version` from its decoded CBOR `map`.
    pub fn from_cbor(version: ClaimVersion, map: CborValue) -> Result<Self, ClaimError> {
        let text_keys = map
            .as_map()
            .is_some_and(|pairs| pairs.iter().all(|(k, _)| k.as_text().is_some()));
        if !text_keys {
            return Err(error("claim", NOT_A_MAP));
        }
        match version {
            ClaimVersion::V2 => from_v2(map),
            ClaimVersion::V1 => from_v1(map),
        }
    }
}

/// Read a `claim-map-v2`.
fn from_v2(map: CborValue) -> Result<Claim, ClaimError> {
    let info = map
        .get("claim_generator_info")
        .ok_or(error("claim_generator_info", MISSING_GENERATOR))?;
    optional_text(&map, "specVersion", false)?;
    Ok(Claim {
        version: ClaimVersion::V2,
        instance_id: required_text(&map, "instanceID", true)?,
        signature: signature_uri(&map)?,
        created_assertions: hashed_uris(&map, "created_assertions")?
            .ok_or(error("created_assertions", MISSING_ARRAY))?,
        gathered_assertions: hashed_uris(&map, "gathered_assertions")?.unwrap_or_default(),
        redacted_assertions: uri_list(&map, "redacted_assertions")?,
        claim_generator_info: vec![generator(info, "claim_generator_info")?],
        claim_generator: None,
        format: None,
        title: optional_text(&map, "dc:title", true)?,
        alg: optional_text(&map, "alg", true)?,
        alg_soft: optional_text(&map, "alg_soft", true)?,
        map,
    })
}

/// Read a legacy `claim-map`. `claim_generator_info` is optional here:
/// claims written before it existed are still accepted.
fn from_v1(map: CborValue) -> Result<Claim, ClaimError> {
    Ok(Claim {
        version: ClaimVersion::V1,
        claim_generator: Some(required_text(&map, "claim_generator", false)?),
        signature: signature_uri(&map)?,
        created_assertions: hashed_uris(&map, "assertions")?
            .ok_or(error("assertions", MISSING_ARRAY))?,
        format: Some(required_text(&map, "dc:format", false)?),
        instance_id: required_text(&map, "instanceID", true)?,
        gathered_assertions: Vec::new(),
        redacted_assertions: uri_list(&map, "redacted_assertions")?,
        claim_generator_info: generator_list(&map)?,
        title: optional_text(&map, "dc:title", true)?,
        alg: optional_text(&map, "alg", true)?,
        alg_soft: optional_text(&map, "alg_soft", true)?,
        map,
    })
}

/// A claim error.
fn error(field: &'static str, reason: &'static str) -> ClaimError {
    ClaimError { field, reason }
}

/// The `signature` field: a `self#jumbf` URI (C2PA 2.4 section 10.2.1).
fn signature_uri(map: &CborValue) -> Result<String, ClaimError> {
    let uri = required_text(map, "signature", false)?;
    if JumbfUri::parse(&uri).is_err() {
        return Err(error("signature", BAD_SIGNATURE_URI));
    }
    Ok(uri)
}

/// The text value of `field`, which must be present (and non-empty when
/// `non_empty`).
fn required_text(
    map: &CborValue,
    field: &'static str,
    non_empty: bool,
) -> Result<String, ClaimError> {
    optional_text(map, field, non_empty)?.ok_or(error(field, MISSING_TEXT))
}

/// The text value of `field` when present (non-empty when `non_empty`).
fn optional_text(
    map: &CborValue,
    field: &'static str,
    non_empty: bool,
) -> Result<Option<String>, ClaimError> {
    let Some(value) = map.get(field) else {
        return Ok(None);
    };
    let text = value.as_text().ok_or(error(field, NOT_TEXT))?;
    if non_empty && text.is_empty() {
        return Err(error(field, EMPTY_TEXT));
    }
    Ok(Some(text.to_string()))
}

/// The non-empty array under `field`, if present.
fn non_empty_array<'a>(
    map: &'a CborValue,
    field: &'static str,
) -> Result<Option<&'a [CborValue]>, ClaimError> {
    let Some(value) = map.get(field) else {
        return Ok(None);
    };
    match value.as_array() {
        Some(items) if !items.is_empty() => Ok(Some(items)),
        _ => Err(error(field, BAD_ARRAY)),
    }
}

/// The hashed URIs under `field`, if present.
fn hashed_uris(map: &CborValue, field: &'static str) -> Result<Option<Vec<HashedUri>>, ClaimError> {
    let Some(items) = non_empty_array(map, field)? else {
        return Ok(None);
    };
    items
        .iter()
        .map(|item| {
            hashed_uri(item)
                .filter(|uri| JumbfUri::parse(&uri.url).is_ok())
                .ok_or(error(field, BAD_HASHED_URI))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

/// One hashed URI map, or `None` when it does not match `hashed-uri-map`.
fn hashed_uri(value: &CborValue) -> Option<HashedUri> {
    let url = value.get("url")?.as_text()?.to_string();
    let hash = value.get("hash")?.as_bytes()?.to_vec();
    let alg = match value.get("alg") {
        None => None,
        Some(alg) => Some(alg.as_text().filter(|a| !a.is_empty())?.to_string()),
    };
    Some(HashedUri { url, alg, hash })
}

/// The URIs under `field` (empty when absent).
fn uri_list(map: &CborValue, field: &'static str) -> Result<Vec<String>, ClaimError> {
    let Some(items) = non_empty_array(map, field)? else {
        return Ok(Vec::new());
    };
    items
        .iter()
        .map(|item| {
            item.as_text()
                .filter(|uri| JumbfUri::parse(uri).is_ok())
                .map(str::to_string)
                .ok_or(error(field, BAD_URI_LIST))
        })
        .collect()
}

/// The legacy optional `claim_generator_info` array.
fn generator_list(map: &CborValue) -> Result<Vec<GeneratorInfo>, ClaimError> {
    let field = "claim_generator_info";
    let Some(items) = non_empty_array(map, field)? else {
        return Ok(Vec::new());
    };
    items.iter().map(|item| generator(item, field)).collect()
}

/// One `generator-info-map`.
fn generator(value: &CborValue, field: &'static str) -> Result<GeneratorInfo, ClaimError> {
    let bad = error(field, BAD_GENERATOR);
    let text_keys = value
        .as_map()
        .is_some_and(|pairs| pairs.iter().all(|(k, _)| k.as_text().is_some()));
    if !text_keys || value.get("icon").is_some_and(|icon| !valid_icon(icon)) {
        return Err(bad);
    }
    let name = optional_text(value, "name", true)
        .map_err(|_| bad)?
        .ok_or(bad)?;
    let version = optional_text(value, "version", false).map_err(|_| bad)?;
    for other in ["operating_system", "specVersion"] {
        optional_text(value, other, false).map_err(|_| bad)?;
    }
    Ok(GeneratorInfo {
        name,
        version,
        map: value.clone(),
    })
}

/// Whether `icon` is a `hashed-uri-map` to an embedded assertion or a
/// `hashed-ext-uri-map` to an `http` or `https` resource, which must name
/// its `alg` (C2PA 2.4 sections 8.4.2.1, 8.4.2.2 and 10.2.3.2).
fn valid_icon(icon: &CborValue) -> bool {
    let Some(uri) = hashed_uri(icon) else {
        return false;
    };
    let lower = uri.url.to_ascii_lowercase();
    let external = lower.starts_with("https://") || lower.starts_with("http://");
    JumbfUri::parse(&uri.url).is_ok() || (external && uri.alg.is_some())
}
