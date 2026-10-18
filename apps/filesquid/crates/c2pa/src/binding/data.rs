//! Data-hash validation (`c2pa.hash.data`, C2PA 2.4 sections 15.12.1 and
//! 18.5).

use std::ops::Range;

use super::{hash_algorithm, included_ranges, BindingError, Check, Outcome};
use crate::cbor::CborValue;
use crate::crypto::CryptoBackend;
use crate::io::ByteSource;

/// Success code.
const MATCH: &str = "assertion.dataHash.match";
/// Failure code for a hash or exclusion that does not fit the asset.
const MISMATCH: &str = "assertion.dataHash.mismatch";
/// Failure code for an unreadable exclusion list.
const MALFORMED: &str = "assertion.dataHash.malformed";
/// Informational code for exclusions other than the manifest store's.
const ADDITIONAL: &str = "assertion.dataHash.additionalExclusionsPresent";

/// Validate the data-hash assertion `value` against the asset.
pub(super) async fn validate<S, C>(
    check: &mut Check<'_, S, C>,
    value: &CborValue,
) -> Result<Outcome, BindingError>
where
    S: ByteSource + ?Sized,
    C: CryptoBackend,
{
    let Some(alg) = hash_algorithm(&[value.get("alg")], check.claim_alg) else {
        return Ok(Outcome::invalid(
            "algorithm.unsupported",
            "the hash algorithm is not allowed",
        ));
    };
    let Some(mut exclusions) = exclusions(value.get("exclusions")) else {
        return Ok(Outcome::invalid(
            MALFORMED,
            "exclusions are malformed, unordered or overlapping",
        ));
    };
    let Some(expected) = value.get("hash").and_then(CborValue::as_bytes) else {
        return Ok(Outcome::invalid(MISMATCH, "the hash field is missing"));
    };
    let container = check.layout.and_then(|l| match l.stores.as_slice() {
        [one] => Some(one.container.clone()),
        _ => None,
    });
    let size = check.asset.size().await?;
    let placed = place_exclusions(
        &mut exclusions,
        container.as_ref(),
        check.updates_seen,
        size,
    );
    if let Err(outcome) = placed {
        return Ok(outcome);
    }
    let additional = exclusions.iter().any(|r| Some(r) != container.as_ref());
    let included = included_ranges(0..size, &exclusions);
    let actual = check
        .crypto
        .digest_ranges(alg, check.asset, &included)
        .await?;
    let info = if additional {
        vec![ADDITIONAL]
    } else {
        Vec::new()
    };
    Ok(Outcome::compared(actual == expected, MATCH, MISMATCH, info))
}

/// Adjust the exclusions for update manifests, then check that they lie
/// within the asset's `size` bytes and that the one holding the store's
/// `container` holds nothing else (section 15.12.1).
fn place_exclusions(
    exclusions: &mut [Range<u64>],
    container: Option<&Range<u64>>,
    updates_seen: bool,
    size: u64,
) -> Result<(), Outcome> {
    let adjusted = match (updates_seen, container) {
        (true, Some(container)) => adjust_for_update(exclusions, container),
        _ => Some(()),
    };
    if adjusted.is_none() {
        return Err(Outcome::invalid(
            MALFORMED,
            "exclusions cannot be adjusted for the update",
        ));
    }
    if exclusions.iter().any(|range| range.end > size) {
        return Err(Outcome::invalid(
            MISMATCH,
            "an exclusion runs past the end of the asset",
        ));
    }
    let too_wide = container.is_some_and(|c| exclusions.iter().any(|r| overlaps(r, c) && r != c));
    if too_wide {
        return Err(Outcome::invalid(
            MISMATCH,
            "the store's exclusion covers other bytes",
        ));
    }
    Ok(())
}

/// The exclusion ranges, or `None` when they are not an array of
/// `{start, length}` maps of unsigned integers, in increasing order and
/// not overlapping. An absent field is no exclusions.
fn exclusions(value: Option<&CborValue>) -> Option<Vec<Range<u64>>> {
    let Some(value) = value else {
        return Some(Vec::new());
    };
    let mut out: Vec<Range<u64>> = Vec::new();
    for item in value.as_array()? {
        let start = unsigned(item.get("start")?)?;
        let end = start.checked_add(unsigned(item.get("length")?)?)?;
        if out.last().is_some_and(|prev| start < prev.end) {
            return None;
        }
        out.push(start..end);
    }
    Some(out)
}

/// An unsigned integer value.
fn unsigned(value: &CborValue) -> Option<u64> {
    match value {
        CborValue::Unsigned(n) => Some(*n),
        _ => None,
    }
}

/// Whether two ranges share a byte.
fn overlaps(a: &Range<u64>, b: &Range<u64>) -> bool {
    a.start < b.end && b.start < a.end
}

/// With update manifests present, the exclusion that starts at the store
/// takes the store's current length and later exclusions shift by the
/// difference (section 15.12.1.1). `None` when the arithmetic overflows.
fn adjust_for_update(exclusions: &mut [Range<u64>], container: &Range<u64>) -> Option<()> {
    let Some(at) = exclusions.iter().position(|r| r.start == container.start) else {
        return Some(());
    };
    let old_end = exclusions[at].end;
    let new_end = container.end;
    exclusions[at].end = new_end;
    for range in &mut exclusions[at + 1..] {
        let len = range.end - range.start;
        let start = range.start.checked_add(new_end)?.checked_sub(old_end)?;
        *range = start..start.checked_add(len)?;
    }
    Some(())
}
