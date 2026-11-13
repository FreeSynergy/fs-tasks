//! The store-backed task repository: module map.
//!
//! Pattern: Facade (declarations) — `TaskRepo` is the one entry point; the files behind it are
//! one concept each (build, CRUD, edit, queries, iCalendar transfer, the trait impls).

mod backend;
mod boards;
mod build;
mod claim;
mod consts;
mod crud;
mod deps;
mod edit;
mod graph_load;
mod handle;
mod items;
mod known;
mod page;
mod queries;
mod ready;
mod scan;
mod span;
mod transfer;
mod unblocked;
mod unblocked_batch;
mod unread;
mod upgrade;

pub use consts::{CONTENT_TYPE, CONTEXT, TASK_TYPE};
pub use handle::TaskRepo;
pub use upgrade::IndexReport;
