//! The time span of a task for the index and the envelope.
//!
//! Pattern: Value object + pure function — `start` / `due` / `estimatedDuration` in UTC.

use chrono::{DateTime, Utc};

use super::handle::TaskRepo;
use crate::error::Result;
use crate::model::Task;
use crate::time::{Dur, Zone, parse_local};

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
            s.as_deref()
                .map(|x| parse_local(x).map(|l| zone.to_utc(l)))
                .transpose()
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
