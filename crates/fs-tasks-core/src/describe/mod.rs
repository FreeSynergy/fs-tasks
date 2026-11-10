//! The type description of `Task` (ADR 0066): fields, commands and the table of what the core does.
//!
//! Pattern: Facade (declarations) — callers use `describe::{support, fields}` and `Task::describe()`
//! (`fs_types::DescribesType`).

mod commands;
pub mod fields;
mod locked;
pub mod support;
mod task_type;

pub use task_type::STANDARD;
