//! iCalendar (RFC 5545) import / export of tasks: module map.
//!
//! Pattern: Adapter (declarations) — makes typed tasks from the shared conversion in `fs-jscalendar`.

mod convert;

pub use convert::{ICS_MEDIA_TYPE, IcsPart, parse_ics, split_ics, to_ics};
