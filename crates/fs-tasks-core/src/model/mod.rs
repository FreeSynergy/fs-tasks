//! Typed JSCalendar 2.0 `Task` model: module map.
//!
//! Pattern: Facade (declarations) — callers use `model::{Task, Participant, Relation, ...}`. What
//! every JSCalendar object shares comes from `fs-jscalendar`; `Task` itself is ours (ADR 0050).
//! Unknown and vendor properties stay verbatim in `extra`, so round trips lose nothing.

mod gap;
mod ops;
mod relation;
mod task;

pub use fs_jscalendar::model::{
    Extra, LEGACY_KEY, Map, NDay, PARTICIPATION_STATUSES, Participant, RecurrenceRule, VENDOR,
    VERSION,
};
pub use gap::{GAP_KEY, MAX_GAP_DAYS, parse_gap};
pub use ops::TASK_STATUS_KEY;
pub use relation::Relation;
pub use task::Task;
