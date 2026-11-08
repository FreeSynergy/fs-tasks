//! `fs-tasks`: the command line of tasks.
//!
//! Pattern: Command — `args` turns words into a [`Command`] (pure), `run` carries one out against a
//! `tasks::Client` and answers the text to print. The client is whatever the caller connected: the
//! binary connects to the socket, the tests to a server in the same process. Nothing here touches
//! the store; the CLI is a second user of `tasks.capnp` (the first is the MCP tools), so a mistake in
//! the contract shows here too.

pub mod args;
mod run;
mod show;

pub use args::{Command, HELP, parse};
pub use run::run;
