//! `fs-tasks-app`: the frontend of the tasks program (ADR 0005, 0009, 0037).
//!
//! Arguments, the words, and an adapter from the program's contract to the
//! workbench's `TaskSource`. It imports no `blinc_*` crate: the window and the work surface come from
//! `fs-components`. The data comes from a running `fs-tasks-server` over its Unix socket
//! (`fs-tasks-client`); the app holds no store (see `DESIGN.md`).
//!
//! Pattern: Facade — this file only declares modules and re-exports the public API.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::must_use_candidate, clippy::module_name_repetitions)]

pub mod args;
mod source;
mod surface;
mod transfer;
mod words;

pub use source::{TaskAdapter, apply_draft, is_marked, is_series};
pub use surface::{TasksScreen, app_widget, screen_widget};
pub use transfer::{ExportReport, ImportSummary, export_all, import_file};
pub use words::labels;
