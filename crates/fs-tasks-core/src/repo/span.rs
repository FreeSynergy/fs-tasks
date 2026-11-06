//! The time span of a task for the index and the envelope.
//!
//! Pattern: Value object + pure function — `start` / `due` / `estimatedDuration` in UTC.

use chrono::{DateTime, Datelike, Utc};

use super::consts::{FIRST_INDEX_TIME, LAST_INDEX_TIME};
use super::handle::TaskRepo;
use crate::error::Result;
use crate::model::Task;
use crate::time::{Dur, Zone, fmt_utc, parse_local};

/// The index value of an instant. A local time near the edge of years 0 to 9999 can leave them in UTC,
/// and then its text would no longer sort: `-0001-...` falls out of every range. So it stays at the
/// edge. The task keeps its real time, only the order in the index is at stake.
pub(super) fn index_time(t: DateTime<Utc>) -> String {
    match t.year() {
        ..0 => FIRST_INDEX_TIME.into(),
        0..10_000 => fmt_utc(t),
        _ => LAST_INDEX_TIME.into(),
    }
}

/// `(start, end, due)`, all UTC.
pub(super) struct Span {
    pub(super) start: Option<DateTime<Utc>>,
    pub(super) end: Option<DateTime<Utc>>,
    pub(super) due: Option<DateTime<Utc>>,
}

impl TaskRepo {
    /// Time span for the index. The end is the due date, else start + estimated duration, else the start.
    pub(super) fn time_index(&self, t: &Task) -> Result<Span> {
        let zone = match t.time_zone.as_deref() {
            Some(z) => Zone::parse(z)?,
            None => self.floating.clone(),
        };
        let local = |s: &Option<String>| -> Result<Option<DateTime<Utc>>> {
            Ok(s.as_deref()
                .map(|x| parse_local(x).map(|l| zone.to_utc(l)))
                .transpose()?)
        };
        let (start, due) = (local(&t.start)?, local(&t.due)?);
        let est = t
            .estimated_duration
            .as_deref()
            .map(Dur::parse)
            .transpose()?;
        let end = due.or_else(|| {
            start
                .zip(est)
                .map(|(s, d)| d.end_utc(&Zone::Utc, s.naive_utc()))
        });
        Ok(Span {
            start: start.or(due),
            end: end.or(start),
            due,
        })
    }
}
