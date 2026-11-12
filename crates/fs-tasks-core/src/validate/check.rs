//! Entry points of the validation: `issues` and `validate`.
//!
//! Pattern: Specification (composite) — the task-specific rules here, the shared ones from
//! `fs-jscalendar`; `validate` turns a non-empty list into `Error::Invalid`.

use fs_jscalendar::validate::{Ctx, common, into_result, shared};
use fs_jscalendar::{Issue, quoted};

use crate::error::Result;
use crate::model::Task;
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
    let mut c = Ctx::new();
    common(
        &mut c,
        t.type_.as_deref(),
        Task::TYPE,
        t.version.as_deref(),
        &t.uid,
    );
    c.one_of("/progress", t.progress.as_deref(), &PROGRESS);
    if t.percent_complete.is_some_and(|p| p > 100) {
        c.bad("/percentComplete", "0..=100");
    }
    if let Some(s) = &t.due
        && parse_local(s).is_err()
    {
        c.bad("/due", format!("not a LocalDateTime: {}", quoted(s)));
    }
    if let Some(d) = &t.estimated_duration
        && Dur::parse(d).is_err()
    {
        c.bad(
            "/estimatedDuration",
            format!("not a duration: {}", quoted(d)),
        );
    }
    if let Some(s) = &t.progress_updated
        && parse_utc(s).is_err()
    {
        c.bad("/progressUpdated", "not a UTCDateTime");
    }
    for (path, what) in crate::model::exec_issues(t) {
        c.bad(&path, what);
    }
    shared(&mut c, t);
    c.into_issues()
}

/// Validate; `Err(Error::Invalid)` lists every problem.
pub fn validate(t: &Task) -> Result<()> {
    Ok(into_result(issues(t))?)
}
