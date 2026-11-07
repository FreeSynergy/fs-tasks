//! Shared set-up of the tests: the real server on a real Unix socket over the real store, on a
//! thread of its own, and the client connected to it. Nothing here stands in for either.
//!
//! Pattern: Facade over the set-up; `ReadOnly` is a Decorator over the real repository that offers
//! only the reads, so the real server answers `Unsupported` for every write from the provider rule.
#![allow(dead_code)] // each test file uses a different subset

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use fs_interfaces::Capabilities;
use fs_interfaces::rpc::{bind_socket, serve};
use fs_store::{ObjectId, Store, TursoStore};
use fs_tasks_client::RpcTasks;
use fs_tasks_core::{Result, Task, TaskEntry, TaskFeed, TaskItems, TaskQuery, TaskRepo};
use fs_tasks_server::TasksServer;
use tokio::sync::oneshot;

pub const ME: &str = "https://local.invalid/users/owner";

/// A provider with the required reads only: every other method is the default `Unsupported`.
pub struct ReadOnly(pub Arc<TaskRepo>);

#[async_trait]
impl TaskItems for ReadOnly {
    fn capabilities(&self) -> Capabilities {
        let mut caps = Capabilities::new("fs-tasks-read-only");
        for id in ["task.read", "task.query"] {
            caps.supported(id);
        }
        for id in [
            "task.create",
            "task.edit",
            "task.delete",
            "task.import",
            "task.export",
        ] {
            caps.unsupported(id, "this provider does not offer it");
        }
        caps
    }
    async fn get(&self, id: ObjectId) -> Result<Option<TaskEntry>> {
        self.0.get(id).await
    }
    async fn get_by_uid(&self, uid: &str) -> Result<Option<TaskEntry>> {
        self.0.get_by_uid(uid).await
    }
    async fn list(&self) -> Result<Vec<TaskEntry>> {
        self.0.list().await
    }
    async fn query(&self, query: &TaskQuery) -> Result<Vec<TaskEntry>> {
        self.0.query(query).await
    }
}

/// A provider whose `list` answers only when two callers are in it at the same time: the proof that
/// the client lets calls overlap. A client that took turns would wait here for ever.
pub struct Meeting(pub Arc<TaskRepo>, pub tokio::sync::Barrier);

#[async_trait]
impl TaskItems for Meeting {
    fn capabilities(&self) -> Capabilities {
        self.0.capabilities()
    }
    async fn get(&self, id: ObjectId) -> Result<Option<TaskEntry>> {
        self.0.get(id).await
    }
    async fn get_by_uid(&self, uid: &str) -> Result<Option<TaskEntry>> {
        self.0.get_by_uid(uid).await
    }
    async fn list(&self) -> Result<Vec<TaskEntry>> {
        self.1.wait().await;
        self.0.list().await
    }
    async fn query(&self, query: &TaskQuery) -> Result<Vec<TaskEntry>> {
        self.0.query(query).await
    }
}

#[derive(Clone, Copy)]
enum Mode {
    Full,
    ReadOnly,
    Meeting,
}

/// A server running until this is dropped.
pub struct Served {
    pub socket: PathBuf,
    /// The provider behind the server, for set-up and for comparing (never what is under test).
    pub repo: Arc<TaskRepo>,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
    _dir: tempfile::TempDir,
}

impl Served {
    /// Over a fresh in-memory store, offering everything the repository does.
    pub async fn start() -> Self {
        Self::over(Mode::Full).await
    }

    /// The server's provider lets two `list` calls meet (see [`Meeting`]).
    pub async fn start_meeting() -> Self {
        Self::over(Mode::Meeting).await
    }

    /// The same, but the server's provider offers only the reads.
    pub async fn start_read_only() -> Self {
        Self::over(Mode::ReadOnly).await
    }

    async fn over(mode: Mode) -> Self {
        let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
        let repo = Arc::new(TaskRepo::new(store, ME));
        let items: Arc<dyn TaskItems> = match mode {
            Mode::Full => repo.clone(),
            Mode::ReadOnly => Arc::new(ReadOnly(repo.clone())),
            Mode::Meeting => Arc::new(Meeting(repo.clone(), tokio::sync::Barrier::new(2))),
        };
        Self::serving(items, repo).await
    }

    /// A server whose `TaskItems` is this provider and whose feed comes from `repo`: for a provider
    /// that does what the real one cannot show, such as never answering.
    pub async fn serving(items: Arc<dyn TaskItems>, repo: Arc<TaskRepo>) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("tasks.sock");
        let (stop, stopped) = oneshot::channel::<()>();
        let (ready, listening) = std::sync::mpsc::channel();
        let (path, provider) = (socket.clone(), repo.clone());
        let thread = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            tokio::task::LocalSet::new().block_on(&runtime, async move {
                let bound = bind_socket(&path).await.unwrap();
                let feed: Arc<dyn TaskFeed> = provider;
                let root = TasksServer::new(items, feed);
                ready.send(()).unwrap();
                let shutdown = async {
                    let _ = stopped.await;
                };
                serve(bound, root.into_client().client, shutdown)
                    .await
                    .unwrap();
            });
        });
        listening.recv().unwrap();
        Self {
            socket,
            repo,
            stop: Some(stop),
            thread: Some(thread),
            _dir: dir,
        }
    }

    /// A client of this server.
    pub fn client(&self) -> RpcTasks {
        RpcTasks::connect(&self.socket).unwrap()
    }

    /// Stop the server; clients stay and find it gone.
    pub fn stop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

impl Drop for Served {
    fn drop(&mut self) {
        self.stop();
    }
}

/// A plain task with this `uid` and title.
pub fn task(uid: &str, title: &str) -> Task {
    let mut task = Task::new(title);
    task.uid = uid.to_string();
    task
}

/// A task due on this day of December 2026, in Berlin.
pub fn due(uid: &str, title: &str, day: u32) -> Task {
    let mut task = task(uid, title);
    task.due = Some(format!("2026-12-{day:02}T10:00:00"));
    task.time_zone = Some("Europe/Berlin".into());
    task
}
