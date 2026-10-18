//! BMFF-hash validation (`c2pa.hash.bmff.v2` and `.v3`, C2PA 2.4 sections
//! 15.12.2, 18.6 and A.5.6).
//!
//! The asset's root boxes are read, each exclusion's xpath is followed
//! down through container boxes (parsed only along the path), and every
//! root box that is not excluded whole contributes its 8-byte big-endian
//! file offset followed by its bytes minus the excluded ranges. Merkle
//! trees are reported as not checked.

use std::ops::Range;

use super::{hash_algorithm, included_ranges, BindingError, BindingStatus, Check, Outcome};
use crate::cbor::CborValue;
use crate::crypto::{hash_range, CryptoBackend};
use crate::format::reader::Reader;
use crate::format::{BoxBudget, FormatError};
use crate::io::ByteSource;

/// Success code.
const MATCH: &str = "assertion.bmffHash.match";
/// Failure code for a hash that differs or an unreadable asset.
const MISMATCH: &str = "assertion.bmffHash.mismatch";
/// Failure code for an unreadable assertion.
const MALFORMED: &str = "assertion.bmffHash.malformed";
/// Informational code for exclusions beyond the required ones.
const ADDITIONAL: &str = "assertion.bmffHash.additionalExclusionsPresent";
/// Exclusions that are required (A.5.6) or plain padding.
const EXPECTED_XPATHS: [&str; 5] = ["/uuid", "/ftyp", "/mfra", "/free", "/skip"];
/// Most nodes in an xpath; it also bounds how deep boxes are parsed.
pub const MAX_XPATH_NODES: usize = 16;

/// One xpath node: a box type and an optional 1-based index.
type Node = ([u8; 4], Option<u64>);

/// One `exclusions-map`.
struct Exclusion<'a> {
    /// The xpath text.
    xpath: &'a str,
    /// Its parsed nodes.
    nodes: Vec<Node>,
    /// Required box length.
    length: Option<u64>,
    /// Required FullBox version.
    version: Option<u64>,
    /// Required FullBox flags.
    flags: Option<[u8; 3]>,
    /// Whether flags must match exactly.
    exact: bool,
    /// Required bytes at offsets from the box start.
    data: Vec<(u64, &'a [u8])>,
    /// Parts of the box to exclude (offset, length; 0 means to the end).
    subset: Option<Vec<(u64, u64)>>,
}

/// One BMFF box.
#[derive(Clone)]
struct BmffBox {
    /// Its type.
    kind: [u8; 4],
    /// Where it is, header included.
    range: Range<u64>,
    /// Header length (size, type, large size, extended type).
    header_len: u64,
}

/// Validate the BMFF-hash assertion `value` against the asset.
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
    let Some(exclusions) = exclusions(value.get("exclusions")) else {
        return Ok(Outcome::invalid(
            MALFORMED,
            "exclusions are missing or malformed",
        ));
    };
    if value.get("merkle").is_some() {
        let reason = "BMFF Merkle tree hashes are not supported yet";
        return Ok(Outcome {
            status: BindingStatus::NotChecked { reason },
            informational: Vec::new(),
        });
    }
    let Some(expected) = value.get("hash").and_then(CborValue::as_bytes) else {
        return Ok(Outcome::invalid(MISMATCH, "the hash field is missing"));
    };
    let (roots, excluded) =
        match layout_exclusions(check.asset, &exclusions, check.max_boxes).await? {
            Ok(found) => found,
            Err(outcome) => return Ok(outcome),
        };
    let mut hasher = check.crypto.incremental_hasher(alg)?;
    for root in &roots {
        if excluded
            .iter()
            .any(|r| r.start <= root.start && r.end >= root.end)
        {
            continue;
        }
        hasher.update(&root.start.to_be_bytes());
        for range in included_ranges(root.clone(), &excluded) {
            hash_range(hasher.as_mut(), check.asset, &range).await?;
        }
    }
    let actual = hasher.finalize();
    let additional = exclusions
        .iter()
        .any(|e| !EXPECTED_XPATHS.contains(&e.xpath));
    let info = if additional {
        vec![ADDITIONAL]
    } else {
        Vec::new()
    };
    Ok(Outcome::compared(actual == expected, MATCH, MISMATCH, info))
}

/// The root box ranges and the merged excluded ranges, or the outcome when
/// the asset cannot be walked.
async fn layout_exclusions<S: ByteSource + ?Sized>(
    asset: &mut S,
    exclusions: &[Exclusion<'_>],
    max_boxes: usize,
) -> Result<Result<(Vec<Range<u64>>, Vec<Range<u64>>), Outcome>, BindingError> {
    let mut reader = Reader::new(asset).await.map_err(io_error)?;
    let mut budget = BoxBudget::new(max_boxes);
    let size = reader.size();
    let roots = match boxes(&mut reader, 0..size, &mut budget).await {
        Ok(roots) => roots,
        Err(FormatError::Io(err)) => return Err(err.into()),
        Err(FormatError::TooManyBoxes { .. }) => {
            return Ok(Err(Outcome {
                status: BindingStatus::NotChecked {
                    reason: "the asset has more boxes than ParseLimits::max_asset_boxes",
                },
                informational: Vec::new(),
            }))
        }
        Err(_) => {
            return Ok(Err(Outcome::invalid(
                MISMATCH,
                "the asset is not a valid BMFF box structure",
            )))
        }
    };
    let mut excluded = Vec::new();
    for exclusion in exclusions {
        for matched in matching(&mut reader, &roots, &exclusion.nodes, &mut budget).await? {
            if conditions_hold(&mut reader, &matched, exclusion).await? {
                excluded.extend(excluded_parts(&matched, exclusion));
            }
        }
    }
    Ok(Ok((
        roots.into_iter().map(|b| b.range).collect(),
        merge(excluded),
    )))
}

/// An I/O failure while reading the asset.
fn io_error(err: FormatError) -> BindingError {
    match err {
        FormatError::Io(io) => io.into(),
        _ => BindingError::Io(crate::io::ByteIoError::Backend(
            "unexpected format error".into(),
        )),
    }
}

/// The boxes from `range.start` to `range.end`. Every box is at least 8
/// bytes, so the range bounds the loop.
async fn boxes<S: ByteSource + ?Sized>(
    reader: &mut Reader<'_, S>,
    range: Range<u64>,
    budget: &mut BoxBudget,
) -> Result<Vec<BmffBox>, FormatError> {
    let mut out = Vec::new();
    let mut at = range.start;
    for _ in 0..=(range.end - range.start) / 8 {
        if at >= range.end {
            return Ok(out);
        }
        let found = one_box(reader, at, range.end).await?;
        budget.take(at)?;
        at = found.range.end;
        out.push(found);
    }
    Err(FormatError::Truncated { offset: at })
}

/// The box at `at` inside a container ending at `end`.
async fn one_box<S: ByteSource + ?Sized>(
    reader: &mut Reader<'_, S>,
    at: u64,
    end: u64,
) -> Result<BmffBox, FormatError> {
    let header = reader.bytes(at, 8).await?;
    let size32 = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
    let kind = [header[4], header[5], header[6], header[7]];
    let (mut header_len, size) = match size32 {
        0 => (8, end - at),
        1 => {
            let large = reader.bytes(at + 8, 8).await?;
            (
                16,
                u64::from_be_bytes(
                    large
                        .try_into()
                        .map_err(|_| FormatError::Truncated { offset: at })?,
                ),
            )
        }
        n => (8, u64::from(n)),
    };
    if &kind == b"uuid" {
        header_len += 16;
    }
    let box_end = at.checked_add(size).filter(|e| *e <= end);
    match box_end {
        Some(box_end) if size >= header_len => Ok(BmffBox {
            kind,
            range: at..box_end,
            header_len,
        }),
        _ => Err(FormatError::Malformed {
            offset: at,
            reason: "box size is invalid",
        }),
    }
}

/// The boxes `nodes` selects, starting from `roots`. Children are parsed
/// only under selected boxes; a box whose content is not a box list has
/// no children. The xpath length bounds the descent.
async fn matching<S: ByteSource + ?Sized>(
    reader: &mut Reader<'_, S>,
    roots: &[BmffBox],
    nodes: &[Node],
    budget: &mut BoxBudget,
) -> Result<Vec<BmffBox>, BindingError> {
    let Some((first, rest)) = nodes.split_first() else {
        return Ok(Vec::new());
    };
    let mut current = select(roots, first);
    for node in rest {
        let mut next = Vec::new();
        for parent in &current {
            let content = parent.range.start + parent.header_len..parent.range.end;
            match boxes(reader, content, budget).await {
                Ok(children) => next.extend(select(&children, node)),
                Err(FormatError::Io(err)) => return Err(err.into()),
                Err(_) => {}
            }
        }
        current = next;
    }
    Ok(current)
}

/// The boxes of `list` that match `node`.
fn select(list: &[BmffBox], (kind, index): &Node) -> Vec<BmffBox> {
    let same: Vec<&BmffBox> = list.iter().filter(|b| b.kind == *kind).collect();
    match index {
        None => same.into_iter().cloned().collect(),
        Some(n) => same
            .get(usize::try_from(n.wrapping_sub(1)).unwrap_or(usize::MAX))
            .map(|b| vec![(*b).clone()])
            .unwrap_or_default(),
    }
}

/// Whether `b` meets the exclusion's length, version, flags and data
/// conditions.
async fn conditions_hold<S: ByteSource + ?Sized>(
    reader: &mut Reader<'_, S>,
    b: &BmffBox,
    exclusion: &Exclusion<'_>,
) -> Result<bool, BindingError> {
    if exclusion
        .length
        .is_some_and(|len| len != b.range.end - b.range.start)
    {
        return Ok(false);
    }
    if exclusion.version.is_some() || exclusion.flags.is_some() {
        let Some(full) = read_in_box(reader, b, b.header_len, 4).await? else {
            return Ok(false);
        };
        if exclusion.version.is_some_and(|v| v != u64::from(full[0])) {
            return Ok(false);
        }
        if let Some(flags) = exclusion.flags {
            let has = [full[1], full[2], full[3]];
            let ok = match exclusion.exact {
                true => has == flags,
                false => (0..3).all(|i| has[i] & flags[i] == flags[i]),
            };
            if !ok {
                return Ok(false);
            }
        }
    }
    for (offset, value) in &exclusion.data {
        let found = read_in_box(reader, b, *offset, value.len()).await?;
        if found.as_deref() != Some(*value) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// `len` bytes at `offset` from the start of `b`, or `None` when they are
/// not all inside it.
async fn read_in_box<S: ByteSource + ?Sized>(
    reader: &mut Reader<'_, S>,
    b: &BmffBox,
    offset: u64,
    len: usize,
) -> Result<Option<Vec<u8>>, BindingError> {
    let start = b.range.start.checked_add(offset);
    let end = start.and_then(|s| s.checked_add(len as u64));
    let (Some(start), Some(end)) = (start, end) else {
        return Ok(None);
    };
    if end > b.range.end {
        return Ok(None);
    }
    match reader.bytes(start, len).await {
        Ok(bytes) => Ok(Some(bytes)),
        Err(FormatError::Io(err)) => Err(err.into()),
        Err(_) => Ok(None),
    }
}

/// The ranges of `b` the exclusion removes: the whole box, or its subset
/// ranges clipped to the box (length 0 runs to the end).
fn excluded_parts(b: &BmffBox, exclusion: &Exclusion<'_>) -> Vec<Range<u64>> {
    let Some(subset) = &exclusion.subset else {
        return vec![b.range.clone()];
    };
    subset
        .iter()
        .filter_map(|(offset, length)| {
            let start = b.range.start.checked_add(*offset)?.min(b.range.end);
            let end = match length {
                0 => b.range.end,
                n => start.saturating_add(*n).min(b.range.end),
            };
            (start < end).then_some(start..end)
        })
        .collect()
}

/// Sort `ranges` and merge overlapping or touching ones.
fn merge(mut ranges: Vec<Range<u64>>) -> Vec<Range<u64>> {
    ranges.sort_by_key(|r| (r.start, r.end));
    let mut out: Vec<Range<u64>> = Vec::with_capacity(ranges.len());
    for range in ranges {
        match out.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => out.push(range),
        }
    }
    out
}

/// Parse the `exclusions` array, which must be present and non-empty.
fn exclusions(value: Option<&CborValue>) -> Option<Vec<Exclusion<'_>>> {
    let list = value?.as_array()?;
    if list.is_empty() {
        return None;
    }
    list.iter().map(exclusion).collect()
}

/// Parse one `exclusions-map`.
fn exclusion(value: &CborValue) -> Option<Exclusion<'_>> {
    let xpath = value.get("xpath")?.as_text()?;
    let flags = match value.get("flags") {
        None => None,
        Some(bytes) => Some(<[u8; 3]>::try_from(bytes.as_bytes()?).ok()?),
    };
    let exact = match value.get("exact") {
        None => true,
        Some(CborValue::Bool(b)) => *b,
        Some(_) => return None,
    };
    Some(Exclusion {
        xpath,
        nodes: xpath_nodes(xpath)?,
        length: optional_uint(value.get("length"))?,
        version: optional_uint(value.get("version"))?,
        flags,
        exact,
        data: data_entries(value.get("data"))?,
        subset: subsets(value.get("subset"))?,
    })
}

/// An optional unsigned field: `Some(None)` when absent, `None` when not
/// an unsigned integer.
fn optional_uint(value: Option<&CborValue>) -> Option<Option<u64>> {
    match value {
        None => Some(None),
        Some(CborValue::Unsigned(n)) => Some(Some(*n)),
        Some(_) => None,
    }
}

/// Parse `data` entries `{offset, value}`.
fn data_entries(value: Option<&CborValue>) -> Option<Vec<(u64, &[u8])>> {
    let Some(value) = value else {
        return Some(Vec::new());
    };
    value
        .as_array()?
        .iter()
        .map(|entry| {
            Some((
                optional_uint(entry.get("offset"))??,
                entry.get("value")?.as_bytes()?,
            ))
        })
        .collect()
}

/// Parse `subset` entries `{offset, length}`: increasing and not
/// overlapping; a 0 length (to the end) must come last.
fn subsets(value: Option<&CborValue>) -> Option<Option<Vec<(u64, u64)>>> {
    let Some(value) = value else {
        return Some(None);
    };
    let mut out: Vec<(u64, u64)> = Vec::new();
    for entry in value.as_array()? {
        let offset = optional_uint(entry.get("offset"))??;
        let length = optional_uint(entry.get("length"))??;
        if let Some((prev_offset, prev_len)) = out.last() {
            let prev_end = if *prev_len == 0 {
                u64::MAX
            } else {
                prev_offset.saturating_add(*prev_len)
            };
            if offset < prev_end {
                return None;
            }
        }
        out.push((offset, length));
    }
    Some(Some(out))
}

/// Parse an xpath in the restricted syntax of section 18.6.2:
/// `/box4cc[/box4cc]...` with optional `[n]` indices (n >= 1, no leading
/// zeros), at most [`MAX_XPATH_NODES`] nodes.
fn xpath_nodes(xpath: &str) -> Option<Vec<Node>> {
    let rest = xpath.strip_prefix('/')?;
    let parts: Vec<&str> = rest.split('/').collect();
    if parts.len() > MAX_XPATH_NODES {
        return None;
    }
    parts.into_iter().map(xpath_node).collect()
}

/// Parse one xpath node.
fn xpath_node(node: &str) -> Option<Node> {
    let (name, index) = match node.split_once('[') {
        None => (node, None),
        Some((name, tail)) => {
            let digits = tail.strip_suffix(']')?;
            let valid = !digits.is_empty()
                && !digits.starts_with('0')
                && digits.bytes().all(|b| b.is_ascii_digit());
            (name, Some(digits.parse::<u64>().ok().filter(|_| valid)?))
        }
    };
    let kind: [u8; 4] = name.as_bytes().try_into().ok()?;
    Some((kind, index))
}
