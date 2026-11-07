//! The thread that owns the Cap'n Proto connection.
//!
//! Pattern: Command — a [`Job`] is a call as a command: a closure that gets the capability and answers
//! through its own channel, and runs on the connection's thread. Why: a capnp-rpc client is `!Send`,
//! so it lives on one thread with a current-thread runtime and a `LocalSet`; callers on any thread
//! send the command, the thread runs each as a local task, so calls overlap. Dropping the [`Worker`] ends the loop, closes the connection and joins.

use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::mpsc;
use std::thread::JoinHandle;

use fs_interfaces::Capabilities;
use fs_interfaces::rpc::connect;
use fs_interfaces::tasks_capnp::{items, tasks};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

/// The capability of the server a job may call.
#[derive(Clone)]
pub(crate) struct Remote {
    pub(crate) items: items::Client,
}

/// A piece of work for the connection's thread.
pub(crate) type Job = Box<dyn FnOnce(Remote) -> Pin<Box<dyn Future<Output = ()>>> + Send>;

/// The thread, and the way to reach it.
pub(crate) struct Worker {
    pub(crate) jobs: UnboundedSender<Job>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        // Close the queue first by swapping the sender out, then wait for the thread.
        let (closed, _) = unbounded_channel();
        drop(std::mem::replace(&mut self.jobs, closed));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn other(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!("tasks server: {e}"))
}

/// What the thread learned on connecting.
async fn handshake(socket: &Path) -> io::Result<(Remote, Capabilities, impl Sized)> {
    let connection = connect::<tasks::Client>(socket).await?;
    let root = connection.client.clone();
    let report = root
        .support_report_request()
        .send()
        .promise
        .await
        .map_err(other)?;
    let capabilities =
        Capabilities::read(report.get().and_then(|r| r.get_report()).map_err(other)?)
            .map_err(other)?;
    let items = root.items_request().send().promise.await.map_err(other)?;
    let items = items.get().and_then(|r| r.get_items()).map_err(other)?;
    Ok((Remote { items }, capabilities, connection))
}

/// Start the thread and wait until it is connected.
pub(crate) fn start(socket: PathBuf) -> io::Result<(Worker, Capabilities)> {
    let (jobs, mut queue) = unbounded_channel::<Job>();
    let (ready, answer) = mpsc::channel::<io::Result<Capabilities>>();
    let thread = std::thread::Builder::new()
        .name("fs-tasks-client".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(e) => return drop(ready.send(Err(e))),
            };
            let local = tokio::task::LocalSet::new();
            local.block_on(&runtime, async {
                let (remote, capabilities, connection) = match handshake(&socket).await {
                    Ok(done) => done,
                    Err(e) => return drop(ready.send(Err(e))),
                };
                let _ = ready.send(Ok(capabilities));
                while let Some(job) = queue.recv().await {
                    tokio::task::spawn_local(job(remote.clone()));
                }
                drop(remote);
                drop(connection);
            });
            // Let the disconnect and the jobs still under way finish.
            runtime.block_on(local);
        })?;
    let capabilities = answer
        .recv()
        .map_err(|_| other("the connection thread ended"))??;
    Ok((
        Worker {
            jobs,
            thread: Some(thread),
        },
        capabilities,
    ))
}
