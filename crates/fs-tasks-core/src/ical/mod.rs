//! iCalendar (RFC 5545) import / export of tasks: module map.
//!
//! Pattern: Adapter (declarations) — makes typed tasks from the shared conversion in `fs-jscalendar`.

mod convert;

pub use convert::{
    ICS_MEDIA_TYPE, IcsPart, parse_ics, put_comments, split_ics, take_comments, to_ics,
    unfolded_lines,
};
