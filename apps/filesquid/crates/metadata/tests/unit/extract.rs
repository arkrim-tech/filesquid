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

/// A JPEG-like byte stream with an embedded XMP packet (MD-S-007).
fn xmp_bytes() -> Vec<u8> {
    let packet = "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
<rdf:Description xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\">\
<dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">Harbour &amp; dawn</rdf:li></rdf:Alt></dc:title>\
<dc:subject><rdf:Bag><rdf:li>coast</rdf:li><rdf:li>boats</rdf:li></rdf:Bag></dc:subject>\
<dcterms:creator>George</dcterms:creator>\
<dcterms:name>not a term</dcterms:name>\
</rdf:Description></rdf:RDF></x:xmpmeta><?xpacket end=\"w\"?>";
    let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE1, 0x00, 0x10];
    bytes.extend_from_slice(b"http://ns.adobe.com/xap/1.0/\0");
    bytes.extend_from_slice(packet.as_bytes());
    bytes.extend_from_slice(&[0xFF, 0xD9]);
    bytes
}

#[test]
fn an_embedded_xmp_packet_maps_onto_dcterms() {
    let mut record = Record::default();
    let found = extract_from_bytes(&xmp_bytes(), &file_ref("a.jpg"), &mut record).unwrap();
    assert!(found.xmp);
    assert_eq!(record.get("dcterms:title").unwrap().value, "Harbour & dawn");
    assert_eq!(record.get("dcterms:subject").unwrap().value, "coast; boats");
    assert_eq!(record.get("dcterms:creator").unwrap().value, "George");
    assert!(record.get("dcterms:name").is_none());
    assert_eq!(
        found.rejected, 1,
        "the refused term is counted, not dropped"
    );
}

#[test]
fn bytes_without_an_xmp_packet_report_none() {
    let mut record = Record::default();
    let found = extract_from_bytes(TIFF_DESCRIPTION, &file_ref("a.tif"), &mut record).unwrap();
    assert!(!found.xmp);
}

#[test]
fn an_unterminated_xmp_packet_is_not_an_error() {
    let mut bytes = xmp_bytes();
    bytes.truncate(bytes.len() / 2);
    let mut record = Record::default();
    let found = extract_from_bytes(&bytes, &file_ref("a.jpg"), &mut record).unwrap();
    assert!(!found.xmp);
    assert!(record.get("dcterms:title").is_none());
}
