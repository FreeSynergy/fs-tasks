//! The backend of the Admin frontend over `TaskItems`: module map.
//!
//! Pattern: Facade (declarations) — see `task_backend.rs`.

mod change;
mod commands;
mod ids;
mod instants;
mod issues;
mod read;
mod task_backend;
mod write;

pub use ids::{object_id, record_id};
pub use task_backend::TaskBackend;
