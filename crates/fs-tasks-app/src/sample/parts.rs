//! One task of a sample, written as an iCalendar part.
//!
//! Pattern: Builder — fields set by name, written by `push`.

use std::fmt::Write;

use crate::day::Day;

pub(super) const HEAD: &str =
    "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//FreeSynergy//tasks sample//EN\r\n";

/// One task to write.
#[derive(Default)]
pub(super) struct Part<'a> {
    pub(super) uid: &'a str,
    pub(super) title: &'a str,
    pub(super) status: &'a str,
    pub(super) due: Option<(Day, &'a str)>,
    pub(super) who: Option<&'a str>,
    pub(super) priority: Option<u8>,
    pub(super) labels: &'a [&'a str],
    pub(super) waits_for: &'a [&'a str],
}

pub(super) fn push(out: &mut String, p: &Part) {
    let _ = write!(
        out,
        "BEGIN:VTODO\r\nUID:{}\r\nDTSTAMP:20261101T000000Z\r\nSUMMARY:{}\r\nSTATUS:{}\r\n",
        p.uid, p.title, p.status
    );
    if let Some((day, time)) = p.due {
        let _ = write!(out, "DUE:{}T{time}00Z\r\n", day.compact());
    }
    if let Some(who) = p.who {
        let _ = write!(
            out,
            "ATTENDEE;CN={who};ROLE=REQ-PARTICIPANT:mailto:{}@example.org\r\n",
            who.to_lowercase().replace(' ', ".")
        );
    }
    if let Some(priority) = p.priority {
        let _ = write!(out, "PRIORITY:{priority}\r\n");
    }
    if !p.labels.is_empty() {
        let _ = write!(out, "CATEGORIES:{}\r\n", p.labels.join(","));
    }
    for uid in p.waits_for {
        let _ = write!(out, "RELATED-TO;RELTYPE=DEPENDS-ON:{uid}\r\n");
    }
    out.push_str("END:VTODO\r\n");
}
