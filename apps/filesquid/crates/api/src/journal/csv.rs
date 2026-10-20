//! The CSV form of the journal export (API-S-031): RFC 4180, one row per entry, opaque IDs
//! only. Rows end with CRLF; a field that holds a comma, a double quote, a carriage return
//! or a line feed is enclosed in double quotes, with each double quote doubled.

use std::borrow::Cow;

use super::{format_rfc3339_ms, JournalEntry};

/// The header row: the column names, in row order, without the line end.
pub const CSV_HEADER: &str = "id,time,host,process_id,actor,operation,outcome,level,target_ids,\
duration_ms,error_code,operation_id,object_ref,client_event_id";

/// The line end of every row (RFC 4180).
pub const CSV_LINE_END: &str = "\r\n";

/// `value` as one RFC 4180 field: unchanged unless it holds a comma, a double quote, CR or
/// LF, else quoted with every double quote doubled.
pub fn csv_field(value: &str) -> Cow<'_, str> {
    if !value.contains([',', '"', '\r', '\n']) {
        return Cow::Borrowed(value);
    }
    Cow::Owned(format!("\"{}\"", value.replace('"', "\"\"")))
}

/// One row of `entry`, in [`CSV_HEADER`] order, without the line end. Absent values are
/// empty fields; the targets are one field, separated by spaces (an opaque ID holds none).
pub fn csv_row(entry: &JournalEntry) -> String {
    let text = |v: &Option<String>| v.clone().unwrap_or_default();
    let number = |v: Option<String>| v.unwrap_or_default();
    let fields = [
        number(entry.id.map(|id| id.to_string())),
        format_rfc3339_ms(entry.time_ms),
        entry.host.clone(),
        text(&entry.process_id),
        entry.actor.clone(),
        entry.operation.clone(),
        entry.outcome.as_str().to_string(),
        entry.level.as_str().to_string(),
        entry.target_ids.join(" "),
        number(entry.duration_ms.map(|d| d.to_string())),
        text(&entry.error_code),
        text(&entry.operation_id),
        text(&entry.object_ref),
        text(&entry.client_event_id),
    ];
    fields
        .iter()
        .map(|f| csv_field(f))
        .collect::<Vec<_>>()
        .join(",")
}
