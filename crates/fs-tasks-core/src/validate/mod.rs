//! Validation of a Task against JSCalendar 2.0: module map.
//!
//! Pattern: Facade (declarations) — callers use `validate::{issues, validate}`.

mod check;
mod ctx;
mod rule;
mod shared;

pub use check::{PROGRESS, issues, validate};
