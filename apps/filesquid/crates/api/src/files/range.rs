//! The single-range handling of the ranged content read and the content route (API-S-018,
//! DE-S-005, RFC 9110 section 14). Pure: it parses a `Range` header and plans which bytes an
//! answer carries; it reads nothing.
//!
//! - `bytes=a-b`, `bytes=a-` and `bytes=-n` are honoured. A malformed header, another unit,
//!   or several ranges are ignored, as RFC 9110 allows, and the answer is the whole file.
//! - A range that starts at or past the end, or a zero-length suffix, is unsatisfiable:
//!   `416` with `Content-Range: bytes */<length>`.
//! - Every answer carries at most [`MAX_ANSWER_BYTES`](super::MAX_ANSWER_BYTES); a larger
//!   whole file, or a longer range, is answered `206` with its first bytes up to that cap.

/// One requested byte range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeRequest {
    /// No usable `Range` header: the whole file.
    Whole,
    /// `bytes=first-last` (`last` absent for `bytes=first-`).
    From {
        /// The first byte.
        first: u64,
        /// The last byte, inclusive, when given.
        last: Option<u64>,
    },
    /// `bytes=-n`: the last `n` bytes.
    Suffix(u64),
}

/// Which bytes of a file of known length one answer carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangePlan {
    /// `200`: the whole file, `len` bytes.
    Whole {
        /// The file's length.
        len: u64,
    },
    /// `206`: `len` bytes from `start`.
    Part {
        /// The first byte.
        start: u64,
        /// How many bytes.
        len: u64,
    },
    /// `416`: nothing.
    Unsatisfiable,
}

/// The range set after a case-insensitive `bytes=` unit, or `None` for any other unit.
fn strip_unit(header: &str) -> Option<&str> {
    let unit = header.get(..6)?;
    unit.eq_ignore_ascii_case("bytes=").then(|| &header[6..])
}

impl RangeRequest {
    /// The range asked for by every `Range` field of `headers`, joined as one list, so
    /// repeated fields are several ranges and so [`RangeRequest::Whole`]. A field that is not
    /// visible ASCII is ignored the same way.
    pub fn from_headers(headers: &axum::http::HeaderMap) -> Self {
        let values: Option<Vec<&str>> = headers
            .get_all(axum::http::header::RANGE)
            .iter()
            .map(|v| v.to_str().ok())
            .collect();
        match values {
            Some(values) if !values.is_empty() => Self::parse(Some(&values.join(", "))),
            _ => Self::Whole,
        }
    }

    /// Parse the `Range` header value (several `Range` fields joined with `, `). Anything but
    /// a single `bytes` range is [`RangeRequest::Whole`]; the unit is case-insensitive
    /// (RFC 9110 section 14.1).
    pub fn parse(header: Option<&str>) -> Self {
        let Some(spec) = header.and_then(|h| strip_unit(h.trim())) else {
            return Self::Whole;
        };
        let Some((first, last)) = spec.split_once('-') else {
            return Self::Whole;
        };
        match (first, last) {
            ("", suffix) => suffix.parse().map_or(Self::Whole, Self::Suffix),
            (first, "") => first
                .parse()
                .map_or(Self::Whole, |first| Self::From { first, last: None }),
            (first, last) => Self::bounded(first, last),
        }
    }

    /// `bytes=first-last`, or [`RangeRequest::Whole`] when either is not a number or `last`
    /// is before `first`.
    fn bounded(first: &str, last: &str) -> Self {
        match (first.parse::<u64>(), last.parse::<u64>()) {
            (Ok(first), Ok(last)) if last >= first => Self::From {
                first,
                last: Some(last),
            },
            _ => Self::Whole,
        }
    }

    /// Plan the answer for a file of `len` bytes, carrying at most `cap` bytes (`cap` > 0).
    pub fn plan(self, len: u64, cap: u64) -> RangePlan {
        match self {
            Self::Whole if len <= cap => RangePlan::Whole { len },
            Self::Whole => RangePlan::Part { start: 0, len: cap },
            Self::From { first, .. } if first >= len => RangePlan::Unsatisfiable,
            Self::From { first, last } => {
                let end = last.map_or(len - 1, |last| last.min(len - 1));
                let len = (end - first + 1).min(cap);
                RangePlan::Part { start: first, len }
            }
            Self::Suffix(0) => RangePlan::Unsatisfiable,
            Self::Suffix(_) if len == 0 => RangePlan::Unsatisfiable,
            Self::Suffix(n) => {
                let start = len - n.min(len);
                RangePlan::Part {
                    start,
                    len: (len - start).min(cap),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_byte_ranges_parse() {
        let parse = |h| RangeRequest::parse(Some(h));
        assert_eq!(
            parse("bytes=0-9"),
            RangeRequest::From {
                first: 0,
                last: Some(9)
            }
        );
        assert_eq!(
            parse("bytes=10-"),
            RangeRequest::From {
                first: 10,
                last: None
            }
        );
        assert_eq!(parse("bytes=-5"), RangeRequest::Suffix(5));
        assert_eq!(
            parse("BYTES=-5"),
            RangeRequest::Suffix(5),
            "the unit is case-insensitive"
        );
        assert_eq!(
            parse("Bytes=0-9"),
            RangeRequest::From {
                first: 0,
                last: Some(9)
            }
        );
        for ignored in [
            "bytes=0-1, bytes=4-5",
            "bytes=0-1, 4-5",
            "bytes",
            "bytes=0-1,4-5",
            "items=0-1",
            "bytes=x-1",
            "bytes=5-1",
            "bytes=",
            "0-1",
            "bytes=0 - 1",
            "bytes= 0-1",
        ] {
            assert_eq!(parse(ignored), RangeRequest::Whole, "{ignored}");
        }
        assert_eq!(RangeRequest::parse(None), RangeRequest::Whole);
    }

    #[test]
    fn repeated_range_fields_are_several_ranges() {
        use axum::http::{header::RANGE, HeaderMap, HeaderValue};
        let mut headers = HeaderMap::new();
        assert_eq!(RangeRequest::from_headers(&headers), RangeRequest::Whole);
        headers.append(RANGE, HeaderValue::from_static("bytes=0-1"));
        let one = RangeRequest::From {
            first: 0,
            last: Some(1),
        };
        assert_eq!(RangeRequest::from_headers(&headers), one);
        headers.append(RANGE, HeaderValue::from_static("bytes=4-5"));
        assert_eq!(RangeRequest::from_headers(&headers), RangeRequest::Whole);
    }

    #[test]
    fn plans_cap_every_answer_and_find_unsatisfiable_ranges() {
        let cap = 10;
        assert_eq!(
            RangeRequest::Whole.plan(8, cap),
            RangePlan::Whole { len: 8 }
        );
        assert_eq!(
            RangeRequest::Whole.plan(30, cap),
            RangePlan::Part { start: 0, len: 10 }
        );
        let open = RangeRequest::From {
            first: 25,
            last: None,
        };
        assert_eq!(open.plan(30, cap), RangePlan::Part { start: 25, len: 5 });
        let past = RangeRequest::From {
            first: 30,
            last: Some(40),
        };
        assert_eq!(past.plan(30, cap), RangePlan::Unsatisfiable);
        assert_eq!(
            RangeRequest::Suffix(5).plan(30, cap),
            RangePlan::Part { start: 25, len: 5 }
        );
        assert_eq!(
            RangeRequest::Suffix(50).plan(30, cap),
            RangePlan::Part { start: 0, len: 10 }
        );
        assert_eq!(
            RangeRequest::Suffix(0).plan(30, cap),
            RangePlan::Unsatisfiable
        );
        assert_eq!(
            RangeRequest::Suffix(3).plan(0, cap),
            RangePlan::Unsatisfiable
        );
        assert_eq!(
            RangeRequest::Whole.plan(0, cap),
            RangePlan::Whole { len: 0 }
        );
    }
}
