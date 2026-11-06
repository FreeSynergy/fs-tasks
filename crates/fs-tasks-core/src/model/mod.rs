//! Typed JSCalendar 2.0 `Task` model: module map.
//!
//! Pattern: Facade (declarations) — callers use `model::{Task, Participant, Relation, ...}`.
//! Unknown and vendor properties stay verbatim in `extra`, so round trips lose nothing.

#[macro_use]
mod macros;
mod consts;
mod ops;
mod participant;
mod relation;
mod rule;
mod task;

pub use consts::{Extra, LEGACY_KEY, Map, TASK_STATUS_KEY, VENDOR, VERSION};
pub use participant::{PARTICIPATION_STATUSES, Participant};
pub use relation::Relation;
pub use rule::{NDay, RecurrenceRule};
pub use task::Task;
