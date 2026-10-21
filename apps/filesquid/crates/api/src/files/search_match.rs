//! Whether a file or folder row answers a search query, and how relevant it is (Task
//! API-F2.3; API-S-012, SRS-F-063). Pure.
//!
//! - **Free text** matches the display name and every `dcterms:` statement value, compared
//!   after [`fold`]; a phrase matches as one run of text.
//! - **Relevance** adds, for each free-text term that is not excluded, 3 when the display name
//!   holds it, 2 when a `dcterms:title` does and 1 when another `dcterms:` value does.
//! - **Dates.** `before:` and `after:` compare the calendar date of the head version's
//!   instant in the caller's zone. `createdbefore:` and `createdafter:` read the capture date
//!   (`exif:DateTimeOriginal`, else `dcterms:created`): a value with a zone is an instant and
//!   compares by its date in the caller's zone; a value without one compares by the calendar
//!   date it states, so no instant is invented for it. A file without a capture date matches
//!   neither.
//! - **Folder rows** match free text by name, `type:folder` and `owner:me`; every
//!   file-only clause leaves them out unless it is excluded.

use super::drive::{FileFacts, FolderRow};
use super::search_query::{fold, parse_date, Clause, Query, Term, TypeClass};
use crate::journal::parse_rfc3339_ms;
use time::{Date, OffsetDateTime, UtcOffset};

/// The capture-date statements, in the order they are read.
const CAPTURE_TERMS: [&str; 2] = ["exif:DateTimeOriginal", "dcterms:created"];

/// Media types of `type:document` besides the office families.
const DOCUMENT_TYPES: [&str; 3] = ["application/pdf", "application/msword", "application/rtf"];

/// Media-type prefixes of the office document families of `type:document`.
const DOCUMENT_PREFIXES: [&str; 3] = [
    "application/vnd.openxmlformats-officedocument.",
    "application/vnd.oasis.opendocument.",
    "application/vnd.ms-",
];

/// One file a search examines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchFile {
    /// What the file is at its head.
    pub facts: FileFacts,
    /// The folded title of its home collection, or `None` when it is unfiled.
    pub collection: Option<String>,
    /// True when it is recoverably deleted.
    pub deleted: bool,
}

/// Who searches and in which zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Searcher {
    /// The caller's declared zone, for date boundaries.
    pub zone: UtcOffset,
    /// True when the caller owns the files of this host (`owner:me`).
    pub owns_files: bool,
}

/// True when `file` answers every term of `query`.
pub fn file_matches(query: &Query, file: &SearchFile, searcher: Searcher) -> bool {
    query
        .terms
        .iter()
        .all(|term| file_clause(&term.clause, file, searcher) != term.negated)
}

/// True when `folder` answers every term of `query`.
pub fn folder_matches(query: &Query, folder: &FolderRow, searcher: Searcher) -> bool {
    query
        .terms
        .iter()
        .all(|term| folder_clause(term, folder, searcher) != term.negated)
}

/// The relevance of `file` to `query`.
pub fn file_score(query: &Query, file: &SearchFile) -> i64 {
    let name = fold(&file.facts.display_name);
    query
        .text()
        .map(|text| {
            let in_name = name.contains(text);
            let in_title = dcterms(&file.facts, "title").any(|v| v.contains(text));
            let elsewhere = dcterms(&file.facts, "").any(|v| v.contains(text));
            3 * i64::from(in_name) + 2 * i64::from(in_title) + i64::from(elsewhere)
        })
        .sum()
}

/// The relevance of `folder` to `query`: 3 for each free-text term its name holds.
pub fn folder_score(query: &Query, folder: &FolderRow) -> i64 {
    let name = fold(&folder.name);
    query
        .text()
        .map(|text| 3 * i64::from(name.contains(text)))
        .sum()
}

/// Whether `file` answers `clause`.
fn file_clause(clause: &Clause, file: &SearchFile, searcher: Searcher) -> bool {
    let facts = &file.facts;
    match clause {
        Clause::Text(text) => {
            fold(&facts.display_name).contains(text.as_str())
                || dcterms(facts, "").any(|v| v.contains(text.as_str()))
        }
        Clause::Type(class) => type_matches(*class, &facts.media_type),
        Clause::In(collection) => file.collection.as_deref() == Some(collection.as_str()),
        Clause::Title(text) => dcterms(facts, "title").any(|v| v.contains(text.as_str())),
        Clause::OwnerMe => searcher.owns_files,
        Clause::Creator(name) => dcterms(facts, "creator").any(|v| v == *name),
        Clause::Tag(tag) => dcterms(facts, "subject").any(|v| v.contains(tag.as_str())),
        Clause::Deleted => file.deleted,
        Clause::Unfiled => file.collection.is_none(),
        Clause::Before(date) => instant_date(facts.modified_ms, searcher.zone) < Some(*date),
        Clause::After(date) => instant_date(facts.modified_ms, searcher.zone) >= Some(*date),
        Clause::CreatedBefore(date) => {
            capture_date(facts, searcher.zone).is_some_and(|d| d < *date)
        }
        Clause::CreatedAfter(date) => {
            capture_date(facts, searcher.zone).is_some_and(|d| d >= *date)
        }
        Clause::Source(text) => dcterms(facts, "source").any(|v| v.contains(text.as_str())),
        Clause::LargerThan(bytes) => facts.size > *bytes,
        Clause::SmallerThan(bytes) => facts.size < *bytes,
    }
}

/// Whether `folder` answers the clause of `term`: by name, `type:folder` or `owner:me`; a
/// file-only clause leaves it out when wanted and keeps it when excluded.
fn folder_clause(term: &Term, folder: &FolderRow, searcher: Searcher) -> bool {
    match &term.clause {
        Clause::Text(text) => fold(&folder.name).contains(text.as_str()),
        Clause::Type(class) => *class == TypeClass::Folder,
        Clause::OwnerMe => searcher.owns_files,
        _ => false,
    }
}

/// The folded values of the `dcterms:<term>` statements of `facts` (every `dcterms:` value
/// when `term` is empty).
fn dcterms<'a>(facts: &'a FileFacts, term: &'a str) -> impl Iterator<Item = String> + 'a {
    facts
        .statements
        .iter()
        .filter(move |(curie, _)| {
            curie
                .strip_prefix("dcterms:")
                .is_some_and(|t| term.is_empty() || t == term)
        })
        .map(|(_, value)| fold(value))
}

/// True when `media_type` belongs to `class`.
fn type_matches(class: TypeClass, media_type: &str) -> bool {
    let media_type = media_type.to_ascii_lowercase();
    match class {
        TypeClass::Image => media_type.starts_with("image/"),
        TypeClass::Video => media_type.starts_with("video/"),
        TypeClass::Audio => media_type.starts_with("audio/"),
        TypeClass::Text => media_type.starts_with("text/"),
        TypeClass::Document => {
            DOCUMENT_TYPES.contains(&media_type.as_str())
                || DOCUMENT_PREFIXES.iter().any(|p| media_type.starts_with(p))
        }
        TypeClass::Folder => false,
    }
}

/// The calendar date of the instant `ms` in `zone`.
fn instant_date(ms: i64, zone: UtcOffset) -> Option<Date> {
    let nanos = i128::from(ms) * 1_000_000;
    OffsetDateTime::from_unix_timestamp_nanos(nanos)
        .ok()
        .map(|t| t.to_offset(zone).date())
}

/// The capture date of `facts`: the first capture statement that names one.
fn capture_date(facts: &FileFacts, zone: UtcOffset) -> Option<Date> {
    CAPTURE_TERMS.iter().find_map(|term| {
        facts
            .statements
            .iter()
            .filter(|(curie, _)| curie == term)
            .find_map(|(_, value)| statement_date(value, zone))
    })
}

/// The date a capture value names: an instant's date in `zone` when the value has a zone,
/// else the calendar date it states (`YYYY-MM-DD` or EXIF `YYYY:MM:DD`).
fn statement_date(value: &str, zone: UtcOffset) -> Option<Date> {
    if let Some(ms) = parse_rfc3339_ms(value) {
        return instant_date(ms, zone);
    }
    let day = value.get(..10)?.replace(':', "-");
    parse_date(&day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::search_query::parse;

    /// A file named `name` with `statements`, `size` bytes and head created at `ms`.
    fn file(name: &str, statements: &[(&str, &str)], size: u64, ms: i64) -> SearchFile {
        SearchFile {
            facts: FileFacts {
                file_id: "f".into(),
                object_id: "urn:uuid:f".into(),
                display_name: name.into(),
                media_type: "image/jpeg".into(),
                size,
                modified_ms: ms,
                statements: statements
                    .iter()
                    .map(|(c, v)| (c.to_string(), v.to_string()))
                    .collect(),
            },
            collection: Some("holidays".into()),
            deleted: false,
        }
    }

    /// UTC, as the owner.
    const UTC: Searcher = Searcher {
        zone: UtcOffset::UTC,
        owns_files: true,
    };

    /// True when `query` matches `file` for `searcher`.
    fn hit(query: &str, file: &SearchFile, searcher: Searcher) -> bool {
        file_matches(&parse(query).expect(query), file, searcher)
    }

    #[test]
    fn free_text_reads_the_name_and_dublin_core_values() {
        let photo = file(
            "img_0001.jpg",
            &[("dcterms:description", "Sunset over the harbour")],
            1,
            0,
        );
        assert!(hit("harbour", &photo, UTC));
        assert!(hit("\"over the\" IMG", &photo, UTC));
        assert!(!hit("harbour -sunset", &photo, UTC));
        assert!(!hit("exif", &photo, UTC));
    }

    #[test]
    fn operators_read_their_statements_and_facts() {
        let doc = file(
            "budget.jpg",
            &[
                ("dcterms:title", "Budget 2026"),
                ("dcterms:creator", "Ana Example"),
                ("dcterms:subject", "finance, plans"),
                ("dcterms:source", "https://example.org/a"),
            ],
            200 << 20,
            0,
        );
        for query in [
            "title:budget",
            "creator:\"ana example\"",
            "tag:finance",
            "source:example.org",
            "size>100MB",
            "type:image",
            "in:Holidays",
            "owner:me",
            "-is:deleted",
            "-is:unfiled",
        ] {
            assert!(hit(query, &doc, UTC), "{query}");
        }
        for query in [
            "creator:ana",
            "size<1MB",
            "type:video",
            "type:folder",
            "in:work",
            "is:deleted",
        ] {
            assert!(!hit(query, &doc, UTC), "{query}");
        }
    }

    #[test]
    fn head_dates_use_the_callers_zone() {
        let late = file("a", &[], 1, 1_772_323_200_000 - 3_600_000);
        assert!(hit("after:2026-02-28", &late, UTC));
        assert!(!hit("after:2026-03-01", &late, UTC));
        let west = Searcher {
            zone: UtcOffset::from_hms(-8, 0, 0).expect("zone"),
            owns_files: true,
        };
        assert!(hit("before:2026-03-01", &late, west));
    }

    #[test]
    fn capture_dates_without_a_zone_compare_by_calendar_date() {
        let zoneless = file(
            "a",
            &[("exif:DateTimeOriginal", "2026:03:01 00:30:00")],
            1,
            0,
        );
        let west = Searcher {
            zone: UtcOffset::from_hms(-8, 0, 0).expect("zone"),
            owns_files: true,
        };
        assert!(hit("createdafter:2026-02-28", &zoneless, west));
        assert!(
            hit("createdafter:2026-03-01", &zoneless, west),
            "no instant is invented"
        );
        let zoned = file("a", &[("dcterms:created", "2026-03-01T00:30:00Z")], 1, 0);
        assert!(
            !hit("createdafter:2026-03-01", &zoned, west),
            "an instant moves with the zone"
        );
        let undated = file("a", &[], 1, 0);
        assert!(!hit("createdafter:2000-01-01", &undated, UTC));
        assert!(!hit("createdbefore:2100-01-01", &undated, UTC));
    }

    #[test]
    fn folders_match_by_name_and_type_and_relevance_ranks_names_first() {
        let folder = FolderRow {
            node: "n".into(),
            node_kind: super::super::drive::FolderKind::Collection,
            name: "Client decks".into(),
        };
        assert!(folder_matches(
            &parse("client type:folder").expect("q"),
            &folder,
            UTC
        ));
        assert!(!folder_matches(
            &parse("client size>1").expect("q"),
            &folder,
            UTC
        ));
        assert!(folder_matches(
            &parse("client -size>1").expect("q"),
            &folder,
            UTC
        ));
        let named = file("deck.pdf", &[], 1, 0);
        let titled = file("x.pdf", &[("dcterms:title", "Deck")], 1, 0);
        let described = file("y.pdf", &[("dcterms:description", "a deck")], 1, 0);
        let query = parse("deck").expect("q");
        assert_eq!(file_score(&query, &named), 3);
        assert_eq!(
            file_score(&query, &titled),
            3,
            "title and any dcterms value"
        );
        assert_eq!(file_score(&query, &described), 1);
        assert_eq!(folder_score(&parse("client").expect("q"), &folder), 3);
    }
}
