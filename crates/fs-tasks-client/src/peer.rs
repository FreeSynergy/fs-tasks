//! The connection: what a frontend holds.
//!
//! Pattern: Adapter — `TaskItems` of the core in front of the Cap'n Proto client of `tasks.capnp`.
//! Why: the frontend keeps depending on the trait it already uses, and only the provider behind it
//! changes. The trait is `fs_tasks_core::TaskItems`; this file is the interface of the adapter
//! (`connect`), written down before the tests and the calls (ADR 0043).

use std::io;
use std::path::Path;

/// The tasks program as seen over its socket. Implements `TaskItems`.
///
/// Dropping it closes the connection.
#[derive(Debug)]
pub struct RpcTasks {
    _private: (),
}

impl RpcTasks {
    /// Connect to the `fs-tasks-server` that listens on `socket` and ask what it can do.
    ///
    /// Blocks the calling thread until the server has answered; call it before the window opens, not
    /// from inside an async task of a single-threaded runtime.
    ///
    /// # Errors
    /// Nothing listens there, the server runs as another user, or it does not answer the contract.
    pub fn connect(socket: &Path) -> io::Result<Self> {
        Err(io::Error::other(format!(
            "not written yet: {}",
            socket.display()
        )))
    }
}
