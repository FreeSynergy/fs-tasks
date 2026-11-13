//! The window's side of the socket: `TaskItems` over `tasks.capnp`, with the change feed.
//!
//! Pattern: Proxy + Observer — the Cap'n Proto client is not `Send`, so it lives on a thread of its own (current-thread
//! runtime and `LocalSet`); a call is a job sent to that thread, the answer comes back through a oneshot. The same
//! thread subscribes to the server's changes (`Tasks.subscribe`): each change notice calls the registered callbacks,
//! so a board can read again at once, from whoever made the change. The client starts the server if none answers
//! (`ensure_server`), so the window never holds the store itself.

use std::future::Future;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use fs_interfaces::common_capnp::change_listener;
use fs_interfaces::rpc::connect;
use fs_interfaces::tasks_capnp::{items as wire_items, tasks};
use fs_tasks_core::{Error, Result as CoreResult};
use tokio::sync::{mpsc, oneshot};

use crate::start::ensure_server;

type Callback = Arc<dyn Fn() + Send + Sync>;
type Job = Box<dyn FnOnce(wire_items::Client) + Send>;

/// Who wants to hear that something changed in the store (any client's change, also an own one).
#[derive(Default)]
pub struct Changes {
    callbacks: Mutex<Vec<Callback>>,
}

impl Changes {
    /// Call `f` (from a background thread) after every change notice of the server.
    pub fn on_change(&self, f: impl Fn() + Send + Sync + 'static) {
        self.callbacks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(Arc::new(f));
    }

    fn notify(&self) {
        let all = self
            .callbacks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        for f in all {
            f();
        }
    }
}

struct Listener(Arc<Changes>);

impl change_listener::Server for Listener {
    async fn change(self: Rc<Self>, _: change_listener::ChangeParams) -> Result<(), capnp::Error> {
        self.0.notify();
        Ok(())
    }

    async fn ended(
        self: Rc<Self>,
        _: change_listener::EndedParams,
        _: change_listener::EndedResults,
    ) -> Result<(), capnp::Error> {
        self.0.notify();
        Ok(())
    }
}

/// How long a call may take before it ends with a message (a hanging server must not freeze the window).
const CALL_LIMIT: Duration = Duration::from_secs(10);

/// The tasks of a server at a socket, as a [`TaskItems`](fs_tasks_core::TaskItems) provider.
pub struct RemoteItems {
    jobs: mpsc::UnboundedSender<Job>,
    changes: Arc<Changes>,
    limit: Duration,
    pub(super) capability_ids: Vec<String>,
}

impl RemoteItems {
    /// Connect to the server at `socket`, starting it (with `data` as its data folder) if nobody answers; subscribe to
    /// its changes.
    ///
    /// # Errors
    /// The text says why the server could not be started or reached.
    pub fn connect(socket: PathBuf, data: Option<PathBuf>) -> std::result::Result<Self, String> {
        let changes = Arc::new(Changes::default());
        let (jobs, mut inbox) = mpsc::unbounded_channel::<Job>();
        let (ready, ready_rx) = std::sync::mpsc::channel();
        let feed = Arc::clone(&changes);
        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(r) => r,
                Err(e) => return drop(ready.send(Err(e.to_string()))),
            };
            tokio::task::LocalSet::new().block_on(&runtime, async move {
                if let Err(e) = serve_jobs(&socket, data.as_deref(), feed, &mut inbox, &ready).await
                {
                    let _ = ready.send(Err(e));
                }
            });
        });
        match ready_rx.recv() {
            Ok(Ok(capability_ids)) => Ok(Self {
                jobs,
                changes,
                limit: CALL_LIMIT,
                capability_ids,
            }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("the connection thread ended at once".into()),
        }
    }

    /// The same connection with another time limit for a call (the default is 10 s).
    #[must_use]
    pub fn with_call_limit(mut self, limit: Duration) -> Self {
        self.limit = limit;
        self
    }

    /// The change notices of the server.
    #[must_use]
    pub fn changes(&self) -> Arc<Changes> {
        Arc::clone(&self.changes)
    }

    /// Run `f` on the connection thread with the `Items` client; its answer comes back here.
    pub(super) async fn call<T, F, Fut>(&self, f: F) -> CoreResult<T>
    where
        T: Send + 'static,
        F: FnOnce(wire_items::Client) -> Fut + Send + 'static,
        Fut: Future<Output = CoreResult<T>> + 'static,
    {
        let gone = || Error::Rejected("the connection to the tasks server is gone".into());
        let call_limit = self.limit;
        let (tx, rx) = oneshot::channel();
        let job: Job = Box::new(move |client| {
            tokio::task::spawn_local(async move {
                // A server that hangs must not freeze the window: the call ends with a message after `CALL_LIMIT`.
                let answer = tokio::time::timeout(call_limit, f(client)).await;
                let _ = tx.send(answer.unwrap_or_else(|_| {
                    Err(Error::Rejected(format!(
                        "the tasks server did not answer within {} s",
                        call_limit.as_secs_f32()
                    )))
                }));
            });
        });
        self.jobs.send(job).map_err(|_| gone())?;
        rx.await.map_err(|_| gone())?
    }
}

type Ready = std::sync::mpsc::Sender<std::result::Result<Vec<String>, String>>;

/// Connect, subscribe, say `ready` (with the capability ids), then run the jobs until the proxy is dropped. The
/// connection and the subscription stay alive for as long as this function runs.
async fn serve_jobs(
    socket: &std::path::Path,
    data: Option<&std::path::Path>,
    changes: Arc<Changes>,
    inbox: &mut mpsc::UnboundedReceiver<Job>,
    ready: &Ready,
) -> std::result::Result<(), String> {
    ensure_server(socket, data).await?;
    let said = |e: capnp::Error| format!("{}: {e}", socket.display());
    let connection = connect::<tasks::Client>(socket)
        .await
        .map_err(|e| format!("{}: {e}", socket.display()))?;
    let root = &connection.client;
    let items = root
        .items_request()
        .send()
        .promise
        .await
        .map_err(said)?
        .get()
        .and_then(tasks::items_results::Reader::get_items)
        .map_err(said)?;
    let answer = root
        .capabilities_request()
        .send()
        .promise
        .await
        .map_err(said)?;
    let ids = answer
        .get()
        .and_then(tasks::capabilities_results::Reader::get_capabilities)
        .and_then(fs_interfaces::common_capnp::capability_list::Reader::get_ids)
        .map_err(said)?
        .iter()
        .map(|t| Ok(t.map_err(said)?.to_string().unwrap_or_default()))
        .collect::<std::result::Result<Vec<_>, String>>()?;
    let mut subscribe = root.subscribe_request();
    subscribe
        .get()
        .set_listener(capnp_rpc::new_client(Listener(changes)));
    let subscription = subscribe
        .send()
        .promise
        .await
        .map_err(said)?
        .get()
        .and_then(tasks::subscribe_results::Reader::get_subscription)
        .map_err(said)?;
    let _keep = subscription;
    let _ = ready.send(Ok(ids));
    while let Some(job) = inbox.recv().await {
        job(items.clone());
    }
    Ok(())
}
