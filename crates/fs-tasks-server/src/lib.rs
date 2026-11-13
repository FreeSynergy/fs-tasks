//! `fs-tasks-server`: the contract `tasks.capnp` over a Unix socket, in front of any provider of
//! `TaskItems` and `TaskFeed`. It only translates: no task logic lives here.
//!
//! Interface status: draft (ADR 0045).
//!
//! Pattern: Facade — this file only declares modules and re-exports.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
// The generated `Server` traits are async for every method, also those that never wait.
#![allow(clippy::missing_errors_doc, clippy::unused_async_trait_impl)]

mod cursor;
mod edit_if;
mod errors;
mod facts;
mod feed;
mod host;
mod items_service;
mod paging;
mod program;
mod tasks_service;
mod wire;
mod wire_caps;

pub use host::{Hosted, host};
pub use program::serve_command;
pub use tasks_service::TasksServer;
