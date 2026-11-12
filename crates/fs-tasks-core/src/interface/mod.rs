//! The public interface of the tasks program: module map.
//!
//! Pattern: Facade (declarations) — `TaskItems` and `TaskFeed` are the contract (ADR 0043,
//! 0050); the store-backed implementation is `repo::TaskRepo`.
//! Interface status: draft (ADR 0045); the Cap'n Proto form is `tasks.capnp` in fs-interfaces.

mod comments;
mod feed;
mod items;
mod page;
mod types;

pub use comments::{COMMENT_CAPABILITIES, Comment, Comments};
pub use feed::{Registry, Subscription, TaskChange, TaskFeed, TaskListener};
pub use items::{CAPABILITIES, TaskItems};
pub use page::{Place, Placed, TaskPage};
pub use types::{ImportReport, TaskEntry, TaskQuery};
