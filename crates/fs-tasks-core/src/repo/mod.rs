//! The store-backed task repository: module map.
//!
//! Pattern: Facade (declarations) — `TaskRepo` is the one entry point; the files behind it are
//! one concept each (build, CRUD, edit, queries, iCalendar transfer, the trait impls).

mod backend;
mod build;
mod consts;
mod crud;
mod edit;
mod handle;
mod items;
mod page;
mod queries;
mod span;
mod transfer;
mod unread;

pub use consts::{CONTENT_TYPE, CONTEXT, TASK_TYPE};
pub use handle::TaskRepo;
