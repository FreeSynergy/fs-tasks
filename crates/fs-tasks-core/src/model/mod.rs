//! Typed JSCalendar 2.0 `Task` model: module map.
//!
//! Pattern: Facade (declarations) — callers use `model::{Task, Participant, Relation, ...}`. What
//! every JSCalendar object shares comes from `fs-jscalendar`; `Task` itself is ours (ADR 0050).
//! Unknown and vendor properties stay verbatim in `extra`, so round trips lose nothing.

mod actor;
mod board;
mod exec;
mod gap;
mod ops;
mod relation;
mod task;
mod work;

pub use actor::{
    ACTOR_KEY, ACTOR_TYPE_KEY, AI_KEY, ActorType, MAX_ACTOR_IRI, MAX_ACTOR_NAME, MAX_CLAIM_HOURS,
    actor_of, actor_type_of, check_actor, is_non_human,
};
pub use board::{Board, BoardTask, Column, ColumnLayout, Layout};
pub use exec::{
    ATTEMPTS_KEY, CLAIM_KEY, Claim, ERROR_KEY, LOOP_KEY, LoopPass, MAX_EXEC_BYTES, RESULT_KEY,
    RUN_KEY, TEMPLATE_KEY, exec_issues,
};
pub use fs_jscalendar::model::{
    Extra, LEGACY_KEY, Map, NDay, PARTICIPATION_STATUSES, Participant, RecurrenceRule, VENDOR,
    VERSION,
};
pub use gap::{GAP_KEY, MAX_GAP_DAYS, parse_gap};
pub use ops::TASK_STATUS_KEY;
pub use relation::Relation;
pub use task::Task;
pub use work::{
    CHECK_KEY, CHECKED_KEY, CREATED_BY_KEY, MAX_NOTE_BYTES, NOTE_KEY, ORIGIN_IMPORTED, ORIGIN_KEY,
    ORIGIN_LOCAL, work_issues,
};
