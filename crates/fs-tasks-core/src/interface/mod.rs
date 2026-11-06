//! The public interface of the tasks program: module map.
//!
//! Pattern: Facade (declarations) — `TaskItems` and `TaskFeed` are the contract (ADR 0043,
//! 0050); the store-backed implementation is `repo::TaskRepo`.
//! Interface status: draft (ADR 0045); the Cap'n Proto form is `schema/tasks.capnp`.

mod feed;
mod items;
mod types;

pub use feed::{Registry, Subscription, TaskChange, TaskFeed, TaskListener};
pub use items::{CAPABILITIES, TaskItems};
pub use types::{ImportReport, TaskEntry, TaskQuery};
