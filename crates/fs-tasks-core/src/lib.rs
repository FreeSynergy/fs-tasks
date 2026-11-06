//! `fs-tasks-core`: program core of FreeSynergy tasks (no GUI).
//!
//! A task is a JSCalendar 2.0 `Task` inside an AS2 `Task` envelope stored through `fs-store`
//! (ADR 0014, 0019, 0020). `fs-tasks` owns the standard (ADR 0050): other programs, e.g.
//! `fs-calendar`, use tasks only through the [`TaskItems`] / [`TaskFeed`] traits; [`TaskRepo`]
//! is the store-backed implementation. See `DESIGN.md` for the pattern map.
//! Interface status: draft (ADR 0045).
//!
//! Pattern: Facade — this file only declares modules and re-exports the public API.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
// Pedantic lints that would only add noise to this crate's small, documented API:
// every fallible fn returns the crate `Result` (documented once on `Error`), results of pure
// getters are used by callers anyway, and the date arithmetic casts are range-checked by jiff.
#![allow(
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::module_name_repetitions
)]

// The Cap'n Proto contract and its generated code live in `fs-interfaces` (ADR 0045); re-exported here.
#[cfg(feature = "contract")]
pub use fs_interfaces::{common_capnp, tasks_capnp};
#[cfg(feature = "contract")]
pub mod contract;
mod error;
pub mod ical;
mod interface;
pub mod model;
mod repo;
pub mod validate;

pub use error::{Error, Result};
pub use fs_jscalendar::upgrade::{Upgraded, upgrade};
pub use fs_jscalendar::{Issue, now_utc, time, upgrade};
pub use interface::{
    CAPABILITIES, ImportReport, Registry, Subscription, TaskChange, TaskEntry, TaskFeed, TaskItems,
    TaskListener, TaskQuery,
};
pub use model::{Participant, RecurrenceRule, Relation, Task};
pub use repo::{CONTENT_TYPE, CONTEXT, TASK_TYPE, TaskRepo};
pub use time::{Zone, parse_utc as utc};
