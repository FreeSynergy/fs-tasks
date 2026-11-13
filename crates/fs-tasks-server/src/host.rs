//! Serving the socket from inside another program (the window), on a thread of its own.
//!
//! Pattern: Facade — the store can be opened by one process only (the file is locked), so the window that holds it
//! also answers the command line: workers (`fs-tasks claim`, `next` ...) talk to the window's own repository, with
//! the same write turns as the window's edits, and the board shows what they do.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use fs_interfaces::rpc::{bind_socket, serve};
use fs_tasks_core::TaskRepo;
use tokio::sync::oneshot;

use crate::TasksServer;

/// The server on its thread; dropping it stops the server (and waits two seconds for it, no longer: a client that
/// holds its connection open must not keep the window from closing).
pub struct Hosted {
    stop: Option<oneshot::Sender<()>>,
    done: Receiver<()>,
}

impl Drop for Hosted {
    fn drop(&mut self) {
        drop(self.stop.take());
        let _ = self.done.recv_timeout(Duration::from_secs(2));
    }
}

/// Serve `repo` at `socket` until the returned guard is dropped.
///
/// # Errors
/// The socket could not be made or is served by somebody else; the text says why.
pub fn host(repo: Arc<TaskRepo>, socket: PathBuf) -> Result<Hosted, String> {
    let (ready, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let (stop, stopped) = oneshot::channel::<()>();
    let (finished, done) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(r) => r,
            Err(e) => {
                let _ = ready.send(Err(e.to_string()));
                return;
            }
        };
        tokio::task::LocalSet::new().block_on(&runtime, async move {
            if let Some(dir) = socket.parent()
                && let Err(e) = std::fs::create_dir_all(dir)
            {
                let _ = ready.send(Err(format!("{}: {e}", dir.display())));
                return;
            }
            let root = TasksServer::new(repo.clone(), repo).into_client();
            match bind_socket(&socket).await {
                Ok(bound) => {
                    let _ = ready.send(Ok(()));
                    let _ = serve(bound, root.client, async {
                        let _ = stopped.await;
                    })
                    .await;
                }
                Err(e) => {
                    let _ = ready.send(Err(format!("{}: {e}", socket.display())));
                }
            }
        });
        let _ = finished.send(());
    });
    match ready_rx.recv() {
        Ok(Ok(())) => Ok(Hosted {
            stop: Some(stop),
            done,
        }),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("the server thread ended at once".into()),
    }
}
