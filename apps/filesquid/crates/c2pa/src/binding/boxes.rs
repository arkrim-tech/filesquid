//! General box-hash validation (`c2pa.hash.boxes`, C2PA 2.4 sections
//! 15.12.3 and 18.7) against the boxes a format handler laid out.

use std::ops::Range;

use super::{hash_algorithm, included_ranges, BindingError, BindingStatus, Check, Outcome};
use crate::cbor::CborValue;
use crate::crypto::CryptoBackend;
use crate::format::AssetLayout;
use crate::io::ByteSource;

/// Success code.
const MATCH: &str = "assertion.boxesHash.match";
/// Failure code for a hash that differs, boxes out of order, or an
/// exclusion past its box.
const MISMATCH: &str = "assertion.boxesHash.mismatch";
/// Failure code for an unreadable assertion.
const MALFORMED: &str = "assertion.boxesHash.malformed";
/// Failure code for an asset box the assertion does not list.
const UNKNOWN: &str = "assertion.boxesHash.unknownBox";
/// Informational code for exclusion ranges.
const ADDITIONAL: &str = "assertion.boxesHash.additionalExclusionsPresent";
/// The name of the logical box after the last box (section 18.7.2).
const AFTER: &str = "c2pa.after";
/// The longest box name (`box-name` is `tstr .size (1..10)`).
const MAX_NAME: usize = 10;

/// One `box-hash-map`.
struct Entry<'a> {
    /// Box names, in asset order.
    names: Vec<&'a str>,
    /// The recorded hash.
    hash: Option<&'a [u8]>,
    /// Whether validators may skip this entry.
    excluded: bool,
    /// Exclusions: start relative to the box, length, box index.
    exclusions: Vec<(u64, u64, usize)>,
    /// The entry's own `alg`.
    alg: Option<&'a CborValue>,
}

impl Entry<'_> {
    /// Whether this is the store's own entry, which is never hashed.
    fn is_store(&self) -> bool {
        self.names == ["C2PA"]
    }
}

/// Validate the box-hash assertion `value` against the asset.
pub(super) async fn validate<S, C>(
    check: &mut Check<'_, S, C>,
    value: &CborValue,
) -> Result<Outcome, BindingError>
where
    S: ByteSource + ?Sized,
    C: CryptoBackend,
{
    let Some(layout) = check.layout else {
        return Ok(Outcome {
            status: BindingStatus::NotChecked {
                reason: "a box hash needs the asset's format handler",
            },
            informational: Vec::new(),
        });
    };
    let Some(entries) = entries(value) else {
        return Ok(Outcome::invalid(
            MALFORMED,
            "the boxes list is missing or malformed",
        ));
    };
    let spans = match assign(&entries, layout) {
        Ok(spans) => spans,
        Err(outcome) => return Ok(outcome),
    };
    let mut info = Vec::new();
    for (entry, boxes) in entries.iter().zip(&spans) {
        if entry.excluded || entry.is_store() {
            continue;
        }
        let Some(alg) = hash_algorithm(&[entry.alg, value.get("alg")], check.claim_alg) else {
            return Ok(Outcome::invalid(
                "algorithm.unsupported",
                "the hash algorithm is not allowed",
            ));
        };
        let excluded = match exclusions(entry, boxes) {
            Ok(excluded) => excluded,
            Err(outcome) => return Ok(outcome),
        };
        if !excluded.is_empty() {
            info = vec![ADDITIONAL];
        }
        let Some(expected) = entry.hash else {
            return Ok(Outcome::invalid(MISMATCH, "a box entry has no hash"));
        };
        let whole = boxes.first().map_or(0, |b| b.start)..boxes.last().map_or(0, |b| b.end);
        let included = included_ranges(whole, &excluded);
        let actual = check
            .crypto
            .digest_ranges(alg, check.asset, &included)
            .await?;
        if actual != expected {
            return Ok(Outcome::compared(false, MATCH, MISMATCH, info));
        }
    }
    Ok(Outcome::compared(true, MATCH, MISMATCH, info))
}

/// Read the `boxes` array, or `None` when it is missing, empty or has a
/// malformed entry.
fn entries(value: &CborValue) -> Option<Vec<Entry<'_>>> {
    let boxes = value.get("boxes")?.as_array()?;
    if boxes.is_empty() {
        return None;
    }
    boxes.iter().map(entry).collect()
}

/// Read one `box-hash-map`.
fn entry(value: &CborValue) -> Option<Entry<'_>> {
    let names = value
        .get("names")?
        .as_array()?
        .iter()
        .map(|n| {
            n.as_text()
                .filter(|t| !t.is_empty() && t.chars().count() <= MAX_NAME)
        })
        .collect::<Option<Vec<_>>>()?;
    if names.is_empty() {
        return None;
    }
    let hash = match value.get("hash") {
        None => None,
        Some(hash) => Some(hash.as_bytes()?),
    };
    let excluded = match value.get("excluded") {
        None => false,
        Some(CborValue::Bool(b)) => *b,
        Some(_) => return None,
    };
    let exclusions = match value.get("exclusions") {
        None => Vec::new(),
        Some(list) => list
            .as_array()?
            .iter()
            .map(exclusion)
            .collect::<Option<_>>()?,
    };
    Some(Entry {
        names,
        hash,
        excluded,
        exclusions,
        alg: value.get("alg"),
    })
}

/// One `{start, length, boxIndex?}` exclusion.
fn exclusion(value: &CborValue) -> Option<(u64, u64, usize)> {
    let uint = |key| match value.get(key) {
        Some(CborValue::Unsigned(n)) => Some(*n),
        _ => None,
    };
    let index = match value.get("boxIndex") {
        None => 0,
        Some(_) => usize::try_from(uint("boxIndex")?).ok()?,
    };
    Some((uint("start")?, uint("length")?, index))
}

/// Match the entries' names to the asset's boxes, in order, and return
/// each entry's box ranges.
fn assign(entries: &[Entry<'_>], layout: &AssetLayout) -> Result<Vec<Vec<Range<u64>>>, Outcome> {
    let boxes = &layout.boxes;
    let all_names: Vec<&str> = entries
        .iter()
        .flat_map(|e| e.names.iter().copied())
        .collect();
    let (mut next_box, mut next_name) = (0usize, 0usize);
    let mut spans = Vec::with_capacity(entries.len());
    for (k, entry) in entries.iter().enumerate() {
        if entry.names == [AFTER] {
            if k + 1 != entries.len() {
                return Err(Outcome::invalid(
                    MALFORMED,
                    "c2pa.after is not the last entry",
                ));
            }
            let start = boxes.last().map_or(0, |b| b.range.end);
            let after = start..layout.size.max(start);
            spans.push(vec![after]);
            next_name += 1;
            continue;
        }
        let mut ranges = Vec::with_capacity(entry.names.len());
        for name in &entry.names {
            let Some(asset_box) = boxes.get(next_box) else {
                return Err(Outcome::invalid(
                    MISMATCH,
                    "a listed box is not in the asset",
                ));
            };
            if asset_box.name != *name {
                return Err(out_of_place(&asset_box.name, &all_names[next_name..]));
            }
            ranges.push(asset_box.range.clone());
            (next_box, next_name) = (next_box + 1, next_name + 1);
        }
        spans.push(ranges);
    }
    if next_box < boxes.len() {
        return Err(Outcome::invalid(
            UNKNOWN,
            "the asset has a box the assertion does not list",
        ));
    }
    Ok(spans)
}

/// The failure for an asset box named `found` where another name was
/// expected: out of order when it is listed later, otherwise unknown.
fn out_of_place(found: &str, remaining: &[&str]) -> Outcome {
    if remaining.contains(&found) {
        Outcome::invalid(MISMATCH, "boxes appear out of order")
    } else {
        Outcome::invalid(UNKNOWN, "the asset has a box the assertion does not list")
    }
}

/// The entry's exclusions as absolute ranges, checked against its boxes.
fn exclusions(entry: &Entry<'_>, boxes: &[Range<u64>]) -> Result<Vec<Range<u64>>, Outcome> {
    let mut out: Vec<Range<u64>> = Vec::with_capacity(entry.exclusions.len());
    for (start, length, index) in &entry.exclusions {
        let Some(target) = boxes.get(*index) else {
            return Err(Outcome::invalid(
                MALFORMED,
                "an exclusion's boxIndex names no box",
            ));
        };
        let range = target
            .start
            .checked_add(*start)
            .and_then(|s| Some(s..s.checked_add(*length)?));
        let Some(range) = range else {
            return Err(Outcome::invalid(MALFORMED, "an exclusion overflows"));
        };
        if out.last().is_some_and(|prev| range.start < prev.end) {
            return Err(Outcome::invalid(
                MALFORMED,
                "exclusions are unordered or overlap",
            ));
        }
        if range.end > target.end {
            return Err(Outcome::invalid(
                MISMATCH,
                "an exclusion runs past the end of its box",
            ));
        }
        out.push(range);
    }
    Ok(out)
}
