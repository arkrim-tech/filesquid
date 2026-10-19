//! Extract EXIF, ID3 and embedded XMP packets from caller-supplied bytes and
//! map them onto statements (MD-S-007).
//!
//! Extraction never opens a path ([ADR 027](../../../../../resources/architecture/027-filesquid-metadata-pluggable-stores.md),
//! MD-S-009): the caller reads the bytes (through `filesquid-ocfl`, a browser
//! file handle or anything else) and hands them over with the [`FileRef`] they
//! belong to. It is independent of any [`MetadataStore`](crate::store::MetadataStore).

use std::io::{BufRead, Cursor, ErrorKind, Read, Seek};

use exif::Reader;
use id3::TagLike;
use thiserror::Error;

use crate::file_ref::FileRef;
use crate::record::Record;
use crate::xmp::dcterms_pairs;
use crate::MetadataNamespace;

/// Errors from reading the supplied bytes.
#[derive(Debug, Error)]
pub enum ExtractError {
    /// The reader failed (other than by ending early inside a tag block).
    #[error("could not read the file bytes: {0}")]
    Io(#[from] std::io::Error),
}

/// What an extraction found in one file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Extracted {
    /// The file the report describes, as the caller supplied it.
    pub file: FileRef,
    /// An EXIF block was found and read.
    pub exif: bool,
    /// An ID3 tag was found and read.
    pub id3: bool,
    /// An embedded XMP packet (`<x:xmpmeta>`) was found and read.
    pub xmp: bool,
    /// Statements the record refused (a failed register check); never silently dropped.
    pub rejected: usize,
}

/// Read embedded tags from `bytes` and merge them into `record`.
///
/// # Errors
///
/// [`ExtractError::Io`] when the in-memory reader fails, which it does not do
/// in practice; unreadable or absent tag blocks are not errors.
pub fn extract_from_bytes(
    bytes: &[u8],
    file: &FileRef,
    record: &mut Record,
) -> Result<Extracted, ExtractError> {
    extract_from_reader(&mut Cursor::new(bytes), file, record)
}

/// Read embedded tags from `reader` and merge them into `record`.
///
/// The reader is rewound before each of the EXIF, ID3 and XMP passes. A file with no EXIF
/// block or no ID3 tag is not an error; the returned [`Extracted`] says which
/// blocks were found.
///
/// # Errors
///
/// [`ExtractError::Io`] when the reader fails for a reason other than the data
/// ending inside a tag block. `record` may then hold the statements of the
/// passes that finished before the failure, so discard it on `Err`.
pub fn extract_from_reader<R: BufRead + Seek>(
    reader: &mut R,
    file: &FileRef,
    record: &mut Record,
) -> Result<Extracted, ExtractError> {
    let mut out = Extracted {
        file: file.clone(),
        exif: false,
        id3: false,
        xmp: false,
        rejected: 0,
    };
    reader.rewind()?;
    read_exif(reader, record, &mut out)?;
    reader.rewind()?;
    read_id3(reader, record, &mut out)?;
    reader.rewind()?;
    read_xmp(reader, record, &mut out)?;
    Ok(out)
}

/// Set a statement, counting a refusal in `out` instead of dropping it silently.
fn apply(
    record: &mut Record,
    out: &mut Extracted,
    namespace: MetadataNamespace,
    term: &str,
    value: impl Into<String>,
) {
    if record.set(namespace, term, value).is_err() {
        out.rejected += 1;
    }
}

/// Return whether an I/O failure only means the data ended inside a block.
fn is_truncation(err: &std::io::Error) -> bool {
    err.kind() == ErrorKind::UnexpectedEof
}

fn read_exif<R: BufRead + Seek>(
    reader: &mut R,
    record: &mut Record,
    out: &mut Extracted,
) -> Result<(), ExtractError> {
    let exif = match Reader::new().read_from_container(reader) {
        Ok(exif) => exif,
        Err(exif::Error::Io(err)) if !is_truncation(&err) => return Err(err.into()),
        Err(_) => return Ok(()),
    };
    out.exif = true;
    for field in exif.fields() {
        let tag = format!("{}", field.tag);
        if tag.is_empty() || tag.starts_with("0x") {
            continue;
        }
        let value = field.display_value().with_unit(&exif).to_string();
        apply(record, out, MetadataNamespace::Exif, &tag, value.clone());
        map_exif_to_dcterms(&tag, &value, record, out);
    }
    Ok(())
}

fn map_exif_to_dcterms(tag: &str, value: &str, record: &mut Record, out: &mut Extracted) {
    let dcterms = match tag {
        "ImageDescription" | "UserComment" => Some("description"),
        "Artist" | "XPAuthor" => Some("creator"),
        "Copyright" => Some("rights"),
        "DateTimeOriginal" | "DateTime" => Some("created"),
        "ImageUniqueID" => Some("identifier"),
        _ => None,
    };
    if let Some(term) = dcterms {
        if record.get(&format!("dcterms:{term}")).is_none() {
            apply(record, out, MetadataNamespace::Dcterms, term, value);
        }
    }
}

fn read_id3<R: BufRead + Seek>(
    reader: &mut R,
    record: &mut Record,
    out: &mut Extracted,
) -> Result<(), ExtractError> {
    let tag = match id3::Tag::read_from2(reader) {
        Ok(tag) => tag,
        Err(id3::Error {
            kind: id3::ErrorKind::Io(err),
            ..
        }) if !is_truncation(&err) => return Err(err.into()),
        Err(_) => return Ok(()),
    };
    out.id3 = true;
    if let Some(title) = tag.title() {
        apply(record, out, MetadataNamespace::Dcterms, "title", title);
    }
    if let Some(artist) = tag.artist() {
        apply(record, out, MetadataNamespace::Dcterms, "creator", artist);
    }
    if let Some(album) = tag.album() {
        apply(record, out, MetadataNamespace::SchemaOrg, "album", album);
    }
    if let Some(year) = tag.year() {
        let year = year.to_string();
        apply(
            record,
            out,
            MetadataNamespace::Dcterms,
            "dateCopyrighted",
            year,
        );
    }
    Ok(())
}

/// Most bytes the XMP pass scans for a packet: 16 MiB.
pub const MAX_XMP_SCAN_BYTES: u64 = 16 * 1024 * 1024;

/// Start of an XMP packet's root element.
const XMP_OPEN: &[u8] = b"<x:xmpmeta";

/// End of an XMP packet's root element.
const XMP_CLOSE: &[u8] = b"</x:xmpmeta>";

/// Find the first XMP packet in the first [`MAX_XMP_SCAN_BYTES`] of `reader`
/// and map its `dc:` and `dcterms:` elements onto `dcterms:`. A missing,
/// unterminated or non-UTF-8 packet is not an error; a term the register
/// refuses is counted in `out.rejected`.
fn read_xmp<R: BufRead + Seek>(
    reader: &mut R,
    record: &mut Record,
    out: &mut Extracted,
) -> Result<(), ExtractError> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(MAX_XMP_SCAN_BYTES)
        .read_to_end(&mut bytes)?;
    let Some(packet) = find_packet(&bytes) else {
        return Ok(());
    };
    let Ok(text) = std::str::from_utf8(packet) else {
        return Ok(());
    };
    out.xmp = true;
    for (term, value) in dcterms_pairs(text) {
        apply(record, out, MetadataNamespace::Dcterms, &term, value);
    }
    Ok(())
}

/// The first `<x:xmpmeta>`…`</x:xmpmeta>` span of `bytes`. The opening name
/// must end at whitespace, `>` or `/`, so `<x:xmpmetadata>` is not a packet.
fn find_packet(bytes: &[u8]) -> Option<&[u8]> {
    let mut offset = 0;
    while let Some(found) = find(&bytes[offset..], XMP_OPEN) {
        let start = offset + found;
        let after = bytes.get(start + XMP_OPEN.len()).copied();
        if matches!(after, Some(b' ' | b'\t' | b'\r' | b'\n' | b'>' | b'/')) {
            let length = find(&bytes[start..], XMP_CLOSE)? + XMP_CLOSE.len();
            return Some(&bytes[start..start + length]);
        }
        offset = start + XMP_OPEN.len();
    }
    None
}

/// The offset of the first `needle` in `haystack`.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
