//! `fs-tasks-app`: the frontend of the tasks program (ADR 0005, 0009, 0037).
//!
//! Arguments, opening the core, the words, and an adapter from the program's contract to the
//! workbench's `TaskSource`. It imports no `blinc_*` crate: the window and the work surface come from
//! `fs-components`. The core is called in process for now, not over RPC (see `DESIGN.md`).
//!
//! Pattern: Facade — this file only declares modules and re-exports the public API.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::must_use_candidate, clippy::module_name_repetitions)]

pub mod args;
mod day;
mod face;
mod faces;
pub mod plan_view;
mod program;
mod sample;
mod source;
mod surface;
mod transfer;
mod words;

pub use day::Day;
pub use face::{Face, Preference};
pub use faces::{FaceOptions, Faces};
pub use program::app_command;
pub use sample::{by_name as sample_by_name, chain as sample_chain, demo as sample_demo};
pub use source::{TaskAdapter, apply_draft, is_marked, is_series};
pub use surface::{TasksScreen, app_widget, screen_widget};
pub use transfer::{ExportReport, ImportSummary, export_all, import_file};
pub use words::{labels, labels_next};
