//! `fs-tasks-client`: the tasks program as a client of its own contract `tasks.capnp`.
//!
//! [`RpcTasks`] implements [`fs_tasks_core::TaskItems`] over a Unix socket, so a frontend depends on
//! the trait and talks to a running `fs-tasks-server`, never to the core of the program (ADR 0021).
//! What a method does not offer answers the `Unsupported` of the provider rule (ADR 0045);
//! `DESIGN.md` lists which.
//!
//! Interface status: draft (ADR 0045), as the contract.
//!
//! Pattern: Facade — this file only declares modules and re-exports the public API.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
// A closure reads better than the full path of a generated reader type.
#![allow(
    clippy::module_name_repetitions,
    clippy::redundant_closure_for_method_calls
)]

mod peer;

pub use peer::RpcTasks;
