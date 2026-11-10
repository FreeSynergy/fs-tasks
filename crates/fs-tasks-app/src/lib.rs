//! `fs-tasks-app`: the window of the tasks program (ADR 0005, 0009, 0037, 0065, 0066).
//!
//! The Admin frontend is generated from the type description of `Task` (`fs-tasks-core`); this crate gives it the
//! records and commands ([`TaskBackend`] over `TaskItems`), the words, and the window shell. It draws nothing and
//! imports no `blinc_*` crate. The core is called in process (see `DESIGN.md`).
//!
//! Pattern: Facade — this file only declares modules and re-exports the public API.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::must_use_candidate, clippy::module_name_repetitions)]

pub mod args;
mod backend;
mod frontend_link;
mod program;
mod window;
pub mod words;

pub use backend::{TaskBackend, object_id, record_id};
pub use program::app_command;
pub use window::{HELP_URL, about, task_window};
