//! Extraction reads caller-supplied bytes or a reader, never a path (MD-S-009).

use std::io::Cursor;

use filesquid_metadata::extract::{extract_from_bytes, extract_from_reader};
use filesquid_metadata::file_ref::FileRef;
use filesquid_metadata::record::Record;
use id3::{Tag, TagLike, Version};

/// Big-endian TIFF with one ASCII `ImageDescription` ("Cat").
const TIFF_DESCRIPTION: &[u8] = &[
    b'M', b'M', 0, 42, 0, 0, 0, 8, // header, first IFD at 8
    0, 1, // one entry
    0x01, 0x0E, 0, 2, 0, 0, 0, 4, b'C', b'a', b't', 0, // ImageDescription, inline
    0, 0, 0, 0, // no next IFD
];

fn file_ref(path: &str) -> FileRef {
    FileRef::new("urn:example:o", path).unwrap()
}

/// ID3v2.4 tag bytes with a title and an artist.
fn id3_bytes() -> Vec<u8> {
    let mut tag = Tag::new();
    tag.set_title("Harbour");
    tag.set_artist("George");
    let mut out = Cursor::new(Vec::new());
    tag.write_to(&mut out, Version::Id3v24).unwrap();
    out.into_inner()
}

#[test]
fn exif_from_bytes_sets_native_and_dcterms_statements() {
    let mut record = Record::default();
    let found = extract_from_bytes(TIFF_DESCRIPTION, &file_ref("a.tif"), &mut record).unwrap();
    assert!(found.exif);
    assert!(!found.id3);
    assert_eq!(found.file, file_ref("a.tif"));
    assert!(record
        .get("exif:ImageDescription")
        .unwrap()
        .value
        .contains("Cat"));
    assert!(record
        .get("dcterms:description")
        .unwrap()
        .value
        .contains("Cat"));
}

#[test]
fn id3_from_a_reader_maps_title_and_artist() {
    let bytes = id3_bytes();
    let mut record = Record::default();
    let found =
        extract_from_reader(&mut Cursor::new(bytes), &file_ref("a.mp3"), &mut record).unwrap();
    assert!(found.id3);
    assert_eq!(record.get("dcterms:title").unwrap().value, "Harbour");
    assert_eq!(record.get("dcterms:creator").unwrap().value, "George");
}

#[test]
fn bytes_without_tags_extract_nothing_and_are_not_an_error() {
    let mut record = Record::default();
    let found = extract_from_bytes(b"plain text", &file_ref("a.txt"), &mut record).unwrap();
    assert!(!found.exif && !found.id3);
    assert_eq!(found.rejected, 0);
    assert!(record.statements.is_empty());
}

#[test]
fn truncated_tag_blocks_are_not_an_error() {
    let mut bytes = id3_bytes();
    bytes.truncate(14);
    let mut record = Record::default();
    let found = extract_from_bytes(&bytes, &file_ref("a.mp3"), &mut record).unwrap();
    assert_eq!(found.rejected, 0);
    assert!(record.get("dcterms:title").is_none());
}

#[test]
fn a_reader_can_be_extracted_twice() {
    let mut reader = Cursor::new(TIFF_DESCRIPTION.to_vec());
    let mut first = Record::default();
    let mut second = Record::default();
    extract_from_reader(&mut reader, &file_ref("a.tif"), &mut first).unwrap();
    extract_from_reader(&mut reader, &file_ref("a.tif"), &mut second).unwrap();
    assert_eq!(first, second);
}
