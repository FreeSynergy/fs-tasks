//! The connection: what a frontend holds.
//!
//! Pattern: Adapter — `TaskItems` of the core in front of the Cap'n Proto client of `tasks.capnp`.
//! Why: the frontend keeps depending on the trait it already uses, and only the provider behind it
//! changes. The thread of the connection is in `worker.rs`; the calls in `calls.rs`; the trait in
//! `items_impl.rs`. Interface first (ADR 0043): `connect` was written down before the tests.

use std::future::Future;
use std::io;
use std::path::Path;

use fs_interfaces::Capabilities;
use fs_store::Error as StoreError;
use fs_tasks_core::{Error, Result};
use tokio::sync::oneshot;

use fs_interfaces::rpc::client::Timeouts;

use crate::worker::{Job, Remote, Worker, start};

/// What the server can do and this client does not call: reported as not possible, with the reason,
/// so `capabilities()` never promises more than the methods of `RpcTasks` keep.
const NOT_OFFERED: [&str; 4] = [
    "task.assign",
    "relation.create",
    "label.assign",
    "task.subscribe",
];

/// The report of the server, cut to what this client implements.
fn offered(reported: &Capabilities) -> Capabilities {
    let mut caps = Capabilities::new(reported.provider());
    for entry in reported.entries() {
        if entry.supported && NOT_OFFERED.contains(&entry.capability.as_str()) {
            caps.unsupported(
                entry.capability.as_str(),
                "the server offers it, the client of the tasks program does not yet",
            );
        } else if entry.supported {
            caps.supported(entry.capability.as_str());
        } else {
            caps.unsupported(entry.capability.as_str(), entry.reason.as_str());
        }
    }
    caps
}

/// The tasks program as seen over its socket. Implements `TaskItems`.
///
/// Calls from any thread are fine; they queue for the one thread that owns the connection and
/// overlap there. Dropping it closes the connection.
pub struct RpcTasks {
    worker: Worker,
    capabilities: Capabilities,
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
        Self::connect_with(socket, Timeouts::default())
    }

    /// [`connect`](Self::connect) with other deadlines than the defaults (connect 5 s, call 30 s).
    ///
    /// # Errors
    /// As for [`connect`](Self::connect), and the server did not answer in time.
    pub fn connect_with(socket: &Path, _timeouts: Timeouts) -> io::Result<Self> {
        let (worker, reported) = start(socket.to_path_buf())?;
        Ok(Self {
            worker,
            capabilities: offered(&reported),
        })
    }

    /// What the server reported when this connection was made. A snapshot: the server is not asked
    /// again.
    pub(crate) fn report(&self) -> Capabilities {
        self.capabilities.clone()
    }

    /// Run `work` on the connection's thread and wait for its answer.
    pub(crate) async fn call<T, F, Fut>(&self, work: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(Remote) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T>> + 'static,
    {
        let gone = || {
            Error::Store(StoreError::Backend(
                "tasks server: the connection is gone".into(),
            ))
        };
        let (reply, answer) = oneshot::channel();
        let job: Job = Box::new(move |remote| {
            Box::pin(async move {
                let _ = reply.send(work(remote).await);
            })
        });
        self.worker.jobs.send(job).map_err(|_| gone())?;
        answer.await.map_err(|_| gone())?
    }
}
