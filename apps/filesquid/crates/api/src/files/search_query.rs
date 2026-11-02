//! The search grammar (Task API-F2.3; API-S-012, SRS-F-063): a query of free text and
//! operators, parsed into [`Term`]s. Pure.
//!
//! A query is at most [`MAX_QUERY_BYTES`] bytes and [`MAX_QUERY_TERMS`] terms, separated by
//! white space. A term is a word, a quoted phrase (`"Client deck"`), or an operator `name:value`
//! whose value may be quoted (`creator:"Ana Example"`); `size>` and `size<` take their value
//! without a colon. A leading `-` excludes what the term matches. Operator names are compared
//! without case. Text containing a colon that is not an operator is quoted. Every value and
//! every name compared is first passed through [`fold`]: NFC and lower case.
//!
//! - **Answered here:** `type:` (`image`, `video`, `audio`, `text`, `document`, `folder`),
//!   `in:` (a collection's title), `title:`, `owner:me`, `creator:`, `tag:`, `is:deleted`,
//!   `before:`, `after:`, `createdbefore:`, `createdafter:` (`YYYY-MM-DD`),
//!   `source:`, `size>` and `size<` (an integer and an optional `B`, `KB`, `MB` or `GB`, in
//!   powers of 1024).
//! - **In the grammar, not answerable on this host:** `is:favorite`, `owner:` naming another
//!   user, and the `filesquid-server` operators `sharedwith:`, `to:`, `from:` and
//!   `pendingowner:`. They are refused as `unavailable` naming the operator (the disabled-module
//!   error of API-S-007), never ignored.
//! - **Unknown:** any other `name:` (`generator:` among them, since the `c2pa` module is not
//!   built) is refused as `invalid_request` naming it, never matched as an empty filter.

use super::error::FileError;
use crate::ErrorCode;
use icu_normalizer::ComposingNormalizerBorrowed;
use time::Date;

/// Longest query, in bytes.
pub const MAX_QUERY_BYTES: usize = 1_024;

/// Most terms in one query.
pub const MAX_QUERY_TERMS: usize = 32;

/// Operators of the grammar that need records this host does not keep.
const UNAVAILABLE_OPERATORS: [&str; 4] = ["sharedwith", "to", "from", "pendingowner"];

/// A file class of `type:`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeClass {
    /// `image/*`.
    Image,
    /// `video/*`.
    Video,
    /// `audio/*`.
    Audio,
    /// `text/*`.
    Text,
    /// PDF and office documents.
    Document,
    /// A folder row (a collection or virtual collection).
    Folder,
}

/// What one term asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Clause {
    /// Free text or a quoted phrase, folded ([`fold`]).
    Text(String),
    /// `type:`.
    Type(TypeClass),
    /// `in:`, folded.
    In(String),
    /// `title:`, folded.
    Title(String),
    /// `owner:me`.
    OwnerMe,
    /// `creator:`, folded.
    Creator(String),
    /// `tag:`, folded.
    Tag(String),
    /// `is:deleted`.
    Deleted,
    /// `before:`: the head version is older than this date.
    Before(Date),
    /// `after:`: the head version is from this date or later.
    After(Date),
    /// `createdbefore:`: captured before this date.
    CreatedBefore(Date),
    /// `createdafter:`: captured on or after this date.
    CreatedAfter(Date),
    /// `source:`, folded.
    Source(String),
    /// `size>`: larger than this many bytes.
    LargerThan(u64),
    /// `size<`: smaller than this many bytes.
    SmallerThan(u64),
}

/// One term: a clause, or its exclusion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Term {
    /// What the term matches.
    pub clause: Clause,
    /// True for a leading `-`: the term excludes what it matches.
    pub negated: bool,
}

/// A parsed query.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    /// The terms, in order.
    pub terms: Vec<Term>,
}

impl Query {
    /// True when a term asks for deleted files.
    pub fn wants_deleted(&self) -> bool {
        self.terms
            .iter()
            .any(|t| !t.negated && t.clause == Clause::Deleted)
    }

    /// The folded free-text and phrase terms that are not excluded.
    pub fn text(&self) -> impl Iterator<Item = &str> {
        self.terms
            .iter()
            .filter_map(|t| match (&t.clause, t.negated) {
                (Clause::Text(text), false) => Some(text.as_str()),
                _ => None,
            })
    }
}

/// `text` as search compares it: Unicode NFC, then each character mapped to its lower case
/// (std's full lower-case mapping, which differs from simple case folding only for a few
/// characters, such as `İ`), then NFC again, since a lower-case mapping can decompose.
pub fn fold(text: &str) -> String {
    let nfc = ComposingNormalizerBorrowed::new_nfc();
    let lower: String = nfc
        .normalize(text)
        .chars()
        .flat_map(char::to_lowercase)
        .collect();
    nfc.normalize(&lower).into_owned()
}

/// Parse `query`.
///
/// # Errors
/// `invalid_request` for a query over its bounds, an unclosed quote, an unknown operator or
/// a malformed value; `unavailable` for an operator this host cannot answer.
pub fn parse(query: &str) -> Result<Query, FileError> {
    if query.len() > MAX_QUERY_BYTES {
        return Err(FileError::invalid(format!(
            "A search query is at most {MAX_QUERY_BYTES} bytes."
        )));
    }
    let tokens = tokenize(query)?;
    if tokens.len() > MAX_QUERY_TERMS {
        return Err(FileError::invalid(format!(
            "A search query has at most {MAX_QUERY_TERMS} terms."
        )));
    }
    let terms = tokens
        .into_iter()
        .map(|token| term(&token))
        .collect::<Result<_, _>>()?;
    Ok(Query { terms })
}

/// One token: its text with quotes removed, and whether any of it was quoted.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Token {
    text: String,
    quoted_from: Option<usize>,
}

/// The tokens of `query`: runs of non-space characters, where a quoted part may hold spaces.
fn tokenize(query: &str) -> Result<Vec<Token>, FileError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted_from = None;
    let mut in_quotes = false;
    for c in query.chars() {
        match (c, in_quotes) {
            ('"', _) => {
                in_quotes = !in_quotes;
                quoted_from.get_or_insert(current.len());
            }
            (c, false) if c.is_whitespace() => flush(&mut tokens, &mut current, &mut quoted_from),
            (c, _) => current.push(c),
        }
    }
    if in_quotes {
        return Err(FileError::invalid(
            "A quote in the search query is not closed.",
        ));
    }
    flush(&mut tokens, &mut current, &mut quoted_from);
    Ok(tokens)
}

/// End the token being built, if it holds anything.
fn flush(tokens: &mut Vec<Token>, current: &mut String, quoted_from: &mut Option<usize>) {
    if !current.is_empty() || quoted_from.is_some() {
        tokens.push(Token {
            text: std::mem::take(current),
            quoted_from: quoted_from.take(),
        });
    }
}

/// The term `token` names.
fn term(token: &Token) -> Result<Term, FileError> {
    let negated = token.text.starts_with('-') && token.quoted_from != Some(0);
    let body = match negated {
        true => &token.text[1..],
        false => token.text.as_str(),
    };
    let quoted_at = token
        .quoted_from
        .map(|at| at.saturating_sub(usize::from(negated)));
    let clause = match operator_of(body, quoted_at) {
        Some((name, value)) => operator(&fold(name), value)?,
        None => text_clause(body)?,
    };
    Ok(Term { clause, negated })
}

/// The operator name and value of `body`, when it is an operator: letters before a `:` (or
/// `size` before `>` or `<`) that are not inside quotes.
fn operator_of(body: &str, quoted_at: Option<usize>) -> Option<(&str, &str)> {
    let split = body.find([':', '>', '<'])?;
    let unquoted = quoted_at.is_none_or(|at| split < at);
    let name = &body[..split];
    let is_name = !name.is_empty() && name.chars().all(|c| c.is_ascii_alphabetic());
    let size = matches!(&body[split..split + 1], ">" | "<");
    match (unquoted && is_name, size) {
        (true, false) => Some((name, &body[split + 1..])),
        (true, true) if fold(name) == "size" => Some((&body[..split + 1], &body[split + 1..])),
        _ => None,
    }
}

/// A free-text or phrase clause.
fn text_clause(text: &str) -> Result<Clause, FileError> {
    match text.trim().is_empty() {
        true => Err(FileError::invalid("A search term is empty.")),
        false => Ok(Clause::Text(fold(text))),
    }
}

/// The clause of operator `name` (folded) with `value`.
fn operator(name: &str, value: &str) -> Result<Clause, FileError> {
    let folded = fold(value);
    match name {
        "type" => type_class(&folded).map(Clause::Type),
        "in" => non_empty(name, folded).map(Clause::In),
        "title" => non_empty(name, folded).map(Clause::Title),
        "owner" => owner(&folded),
        "creator" => non_empty(name, folded).map(Clause::Creator),
        "tag" => non_empty(name, folded).map(Clause::Tag),
        "is" => is_clause(&folded),
        "before" => date(name, value).map(Clause::Before),
        "after" => date(name, value).map(Clause::After),
        "createdbefore" => date(name, value).map(Clause::CreatedBefore),
        "createdafter" => date(name, value).map(Clause::CreatedAfter),
        "source" => non_empty(name, folded).map(Clause::Source),
        "size>" => bytes(value).map(Clause::LargerThan),
        "size<" => bytes(value).map(Clause::SmallerThan),
        other if UNAVAILABLE_OPERATORS.contains(&other) => Err(unavailable(other)),
        other => Err(FileError::invalid(format!(
            "The search operator {other}: is unknown."
        ))),
    }
}

/// `value`, or `invalid_request` naming `name` when it is empty.
fn non_empty(name: &str, value: String) -> Result<String, FileError> {
    match value.trim().is_empty() {
        true => Err(FileError::invalid(format!(
            "The search operator {name}: needs a value."
        ))),
        false => Ok(value),
    }
}

/// The class `type:` names.
fn type_class(value: &str) -> Result<TypeClass, FileError> {
    match value {
        "image" => Ok(TypeClass::Image),
        "video" => Ok(TypeClass::Video),
        "audio" => Ok(TypeClass::Audio),
        "text" => Ok(TypeClass::Text),
        "document" => Ok(TypeClass::Document),
        "folder" => Ok(TypeClass::Folder),
        _ => Err(FileError::invalid(
            "type: takes image, video, audio, text, document or folder.",
        )),
    }
}

/// The clause of `owner:`: `me` here; another user needs accounts this host does not keep.
fn owner(value: &str) -> Result<Clause, FileError> {
    match value.trim() {
        "me" => Ok(Clause::OwnerMe),
        "" => Err(FileError::invalid(
            "The search operator owner: needs a value.",
        )),
        _ => Err(unavailable("owner:")),
    }
}

/// The clause of `is:`.
fn is_clause(value: &str) -> Result<Clause, FileError> {
    match value {
        "deleted" => Ok(Clause::Deleted),
        "favorite" => Err(unavailable("is:favorite")),
        other => Err(FileError::invalid(format!(
            "The search operator is:{other} is unknown; is: takes deleted or favorite."
        ))),
    }
}

/// The calendar date `value` of operator `name`.
fn date(name: &str, value: &str) -> Result<Date, FileError> {
    parse_date(value).ok_or_else(|| {
        FileError::invalid(format!(
            "The search operator {name}: takes a date as YYYY-MM-DD."
        ))
    })
}

/// The calendar date `YYYY-MM-DD`, or `None`.
pub fn parse_date(value: &str) -> Option<Date> {
    let format = time::macros::format_description!("[year]-[month]-[day]");
    Date::parse(value, &format).ok()
}

/// The bytes `value` of `size>` or `size<` names: an integer and an optional unit.
fn bytes(value: &str) -> Result<u64, FileError> {
    let refused =
        || FileError::invalid("size> and size< take an integer and an optional B, KB, MB or GB.");
    let split = value
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(value.len());
    let number: u64 = value[..split].parse().map_err(|_| refused())?;
    let scale: u64 = match fold(&value[split..]).as_str() {
        "" | "b" => 1,
        "kb" => 1 << 10,
        "mb" => 1 << 20,
        "gb" => 1 << 30,
        _ => return Err(refused()),
    };
    number.checked_mul(scale).ok_or_else(refused)
}

/// The refusal of an operator in the grammar that this host cannot answer.
fn unavailable(operator: &str) -> FileError {
    FileError::new(
        ErrorCode::Unavailable,
        format!("The search operator {operator} is not available on this host."),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The clauses of `query`.
    fn clauses(query: &str) -> Vec<(Clause, bool)> {
        parse(query)
            .expect("query")
            .terms
            .into_iter()
            .map(|t| (t.clause, t.negated))
            .collect()
    }

    /// The error code of parsing `query`.
    fn refused(query: &str) -> ErrorCode {
        parse(query).expect_err(query).code()
    }

    #[test]
    fn words_phrases_and_exclusions() {
        assert_eq!(
            clauses("\"Client deck\" -draft Budget"),
            [
                (Clause::Text("client deck".into()), false),
                (Clause::Text("draft".into()), true),
                (Clause::Text("budget".into()), false),
            ]
        );
        assert_eq!(
            clauses("-\"old copy\""),
            [(Clause::Text("old copy".into()), true)]
        );
        assert_eq!(clauses("\"-x\""), [(Clause::Text("-x".into()), false)]);
        assert_eq!(
            clauses("\"10:30\""),
            [(Clause::Text("10:30".into()), false)]
        );
        assert_eq!(refused("\"open"), ErrorCode::InvalidRequest);
    }

    #[test]
    fn every_answered_operator_parses() {
        let date = parse_date("2026-01-01").expect("date");
        assert_eq!(
            clauses(
                "type:IMAGE in:Campaign title:Budget owner:me creator:\"Ana Example\" tag:finance \
                 is:deleted before:2026-01-01 after:2026-01-01 \
                 createdbefore:2026-01-01 createdafter:2026-01-01 source:example.org \
                 size>100MB SIZE<2kb -type:video"
            ),
            [
                (Clause::Type(TypeClass::Image), false),
                (Clause::In("campaign".into()), false),
                (Clause::Title("budget".into()), false),
                (Clause::OwnerMe, false),
                (Clause::Creator("ana example".into()), false),
                (Clause::Tag("finance".into()), false),
                (Clause::Deleted, false),
                (Clause::Before(date), false),
                (Clause::After(date), false),
                (Clause::CreatedBefore(date), false),
                (Clause::CreatedAfter(date), false),
                (Clause::Source("example.org".into()), false),
                (Clause::LargerThan(100 << 20), false),
                (Clause::SmallerThan(2 << 10), false),
                (Clause::Type(TypeClass::Video), true),
            ]
        );
        assert_eq!(clauses("size>7"), [(Clause::LargerThan(7), false)]);
    }

    #[test]
    fn unknown_operators_and_bad_values_are_refused_and_never_ignored() {
        for query in [
            "colour:blue",
            "generator:\"Camera App\"",
            "type:spreadsheet",
            "is:starred",
            "before:2026-13-01",
            "after:yesterday",
            "size>10TB",
            "size>",
            "title:",
            "owner:\"   \"",
            "\"\"",
        ] {
            assert_eq!(refused(query), ErrorCode::InvalidRequest, "{query}");
        }
    }

    #[test]
    fn operators_this_host_cannot_answer_are_unavailable() {
        for query in [
            "is:favorite",
            "owner:ana",
            "sharedwith:public",
            "to:ben",
            "from:ben",
            "pendingowner:me",
        ] {
            assert_eq!(refused(query), ErrorCode::Unavailable, "{query}");
        }
    }

    #[test]
    fn queries_are_bounded() {
        assert_eq!(
            refused(&"a".repeat(MAX_QUERY_BYTES + 1)),
            ErrorCode::InvalidRequest
        );
        let many = vec!["x"; MAX_QUERY_TERMS + 1].join(" ");
        assert_eq!(refused(&many), ErrorCode::InvalidRequest);
        assert!(parse(&vec!["x"; MAX_QUERY_TERMS].join(" ")).is_ok());
    }

    #[test]
    fn folding_is_nfc_and_lower_case_without_diacritic_folding() {
        assert_eq!(fold("CAFÉ RÉSUMÉ"), "café résumé");
        assert_eq!(
            fold("Cafe\u{301}"),
            "caf\u{e9}",
            "a decomposed name matches its composed form"
        );
        assert_eq!(fold("CAF\u{c9}"), fold("Cafe\u{301}"));
        assert_ne!(fold("Café"), "cafe", "no diacritic folding");
    }
}
