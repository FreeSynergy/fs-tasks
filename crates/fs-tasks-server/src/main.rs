//! `fs-tasks-server`. Usage: `fs-tasks-server [--socket PATH] [--data DIR] [--actor IRI]`.
//!
//! Pattern: none (thin binary) — reads its options, opens the store, serves until a signal.

use std::sync::Arc;

use fs_interfaces::rpc::{ServerOptions, prepare_data_dir, run_until_signal};
use fs_store::{Store, TursoStore};
use fs_tasks_core::TaskRepo;
use fs_tasks_server::TasksServer;

fn main() {
    let options = ServerOptions::from_env_or_exit("tasks");
    let ServerOptions {
        socket,
        data,
        actor,
        instance,
    } = options;
    let served = run_until_signal(&socket, || async move {
        prepare_data_dir(&data)?;
        let store = TursoStore::open(data.join("objects.db"))
            .await
            .map_err(std::io::Error::other)?;
        let store: Arc<dyn Store> = Arc::new(store);
        let mut repo = TaskRepo::new(store, actor);
        if let Some(instance) = instance {
            repo = repo.with_instance(instance);
        }
        let repo = Arc::new(repo);
        Ok(TasksServer::new(repo.clone(), repo).into_client().client)
    });
    if let Err(e) = served {
        eprintln!("fs-tasks-server: {e}");
        std::process::exit(1);
    }
}
