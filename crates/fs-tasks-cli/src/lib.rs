//! `fs-tasks`: the command line of tasks.
//!
//! Pattern: Command — `args` turns words into a [`Command`] (pure), `run` carries one out against a
//! `tasks::Client` and answers the text to print. The client is whatever the caller connected: the
//! binary connects to the socket (and starts the local server if none is there), the tests to a server
//! in the same process. Nothing here touches the store; the CLI is a second user of `tasks.capnp` (the
//! first is the MCP tools), so a mistake in the contract shows here too. The worker commands (`claim`,
//! `next`, `status`, `check` ...) are the protocol of `docs/worker-protocol.md`.

pub mod args;
mod args_work;
mod check;
mod edit;
mod exec;
mod fail;
mod make;
pub mod policy;
mod program;
mod run;
mod secrets;
mod show;
mod start;
mod work;

pub use args::{Command, HELP, NewTask, Query, To, Work, parse};
pub use exec::{Outcome, execute};
pub use fail::Failure;
pub use policy::Policy;
pub use program::cli_command;
pub use run::{run, run_with};
pub use secrets::refuse_secret;
