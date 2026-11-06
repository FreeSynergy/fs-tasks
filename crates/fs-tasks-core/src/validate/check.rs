//! Entry points of the validation: `issues` and `validate`.
//!
//! Pattern: Specification (composite) — the task-specific rules here, the shared ones in `shared`,
//! the recurrence rule in `rule`; `validate` turns a non-empty list into `Error::Invalid`.

use super::ctx::Ctx;
use super::shared::shared;
use crate::error::{Error, Issue, Result};
use crate::model::{Task, VERSION};
use crate::time::{Dur, parse_local, parse_utc};

/// The allowed `progress` values.
pub const PROGRESS: [&str; 5] = [
    "needs-action",
    "in-process",
    "completed",
    "failed",
    "cancelled",
];

/// All problems found in the task; empty = valid.
pub fn issues(t: &Task) -> Vec<Issue> {
    let mut c = Ctx(vec![]);
    if t.type_.as_deref() != Some(Task::TYPE) {
        c.bad("/@type", "must be \"Task\"");
    }
    if t.version.as_deref() != Some(VERSION) {
        c.bad(
            "/version",
            "required, must be \"2.0\" (upgrade RFC 8984 data with `upgrade`)",
        );
    }
    if t.uid.trim().is_empty() {
        c.bad("/uid", "required");
    }
    c.one_of("/progress", t.progress.as_deref(), &PROGRESS);
    if t.percent_complete.is_some_and(|p| p > 100) {
        c.bad("/percentComplete", "0..=100");
    }
    if let Some(s) = &t.due {
        if parse_local(s).is_err() {
            c.bad("/due", format!("not a LocalDateTime: {s:?}"));
        }
    }
    if let Some(d) = &t.estimated_duration {
        if Dur::parse(d).is_err() {
            c.bad("/estimatedDuration", format!("not a duration: {d:?}"));
        }
    }
    if let Some(s) = &t.progress_updated {
        if parse_utc(s).is_err() {
            c.bad("/progressUpdated", "not a UTCDateTime");
        }
    }
    shared(&mut c, t);
    c.0
}

/// Validate; `Err(Error::Invalid)` lists every problem.
pub fn validate(t: &Task) -> Result<()> {
    let i = issues(t);
    if i.is_empty() {
        Ok(())
    } else {
        Err(Error::Invalid(i))
    }
}
