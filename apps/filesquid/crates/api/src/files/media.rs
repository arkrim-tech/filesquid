//! The media type a content answer carries. OCFL stores no media type, and the file record
//! that will hold the type the client gave at import arrives with MD-0.8 and API-F1.4, so
//! until then the type comes from the logical path's extension through this fixed table.
//! It is never sniffed from the bytes (C-055), and every content answer carries
//! `X-Content-Type-Options: nosniff`. Pure.

/// The fallback for an extension the table does not know.
pub const OCTET_STREAM: &str = "application/octet-stream";

/// Lowercase extension to media type.
const TYPES: &[(&str, &str)] = &[
    ("avif", "image/avif"),
    ("bmp", "image/bmp"),
    ("gif", "image/gif"),
    ("heic", "image/heic"),
    ("heif", "image/heif"),
    ("jpeg", "image/jpeg"),
    ("jpg", "image/jpeg"),
    ("png", "image/png"),
    ("svg", "image/svg+xml"),
    ("tif", "image/tiff"),
    ("tiff", "image/tiff"),
    ("webp", "image/webp"),
    ("m4v", "video/mp4"),
    ("mkv", "video/x-matroska"),
    ("mov", "video/quicktime"),
    ("mp4", "video/mp4"),
    ("webm", "video/webm"),
    ("flac", "audio/flac"),
    ("m4a", "audio/mp4"),
    ("mp3", "audio/mpeg"),
    ("oga", "audio/ogg"),
    ("ogg", "audio/ogg"),
    ("opus", "audio/ogg"),
    ("wav", "audio/wav"),
    ("csv", "text/csv"),
    ("json", "application/json"),
    ("md", "text/markdown"),
    ("pdf", "application/pdf"),
    ("txt", "text/plain"),
    ("xml", "application/xml"),
];

/// The media type of the file at logical `path`, from its extension.
pub fn media_type(path: &str) -> &'static str {
    let name = path.rsplit('/').next().unwrap_or(path);
    let Some((stem, ext)) = name.rsplit_once('.') else {
        return OCTET_STREAM;
    };
    if stem.is_empty() {
        return OCTET_STREAM;
    }
    let ext = ext.to_ascii_lowercase();
    TYPES
        .iter()
        .find(|(known, _)| *known == ext)
        .map_or(OCTET_STREAM, |(_, media)| media)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_map_without_case_and_unknown_ones_fall_back() {
        assert_eq!(media_type("holiday.MP4"), "video/mp4");
        assert_eq!(media_type("photos/a.jpg"), "image/jpeg");
        assert_eq!(media_type("photos.d/readme"), OCTET_STREAM);
        assert_eq!(media_type(".hidden"), OCTET_STREAM);
        assert_eq!(media_type("archive.xyz"), OCTET_STREAM);
    }
}
