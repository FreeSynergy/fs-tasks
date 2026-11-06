//! `fs-tasks-server`. Usage: `fs-tasks-server [--socket PATH] [--data DIR] [--actor IRI]`.
//!
//! Pattern: none (thin binary) — reads its locations, opens the store, serves until a signal.

use std::path::PathBuf;
use std::sync::Arc;

use fs_interfaces::rpc::{default_socket, run_until_signal};
use fs_store::{Store, TursoStore};
use fs_tasks_core::TaskRepo;
use fs_tasks_server::TasksServer;

const USAGE: &str = "fs-tasks-server [--socket PATH] [--data DIR] [--actor IRI]\nDefaults: $XDG_RUNTIME_DIR/freesynergy/tasks.sock, $XDG_DATA_HOME/freesynergy/tasks, the local owner";
const LOCAL_OWNER: &str = "https://local.invalid/users/owner";

fn default_data() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(base.join("freesynergy").join("tasks"))
}

fn fail(message: &str) -> ! {
    eprintln!("fs-tasks-server: {message}");
    std::process::exit(2);
}

fn main() {
    let mut socket = default_socket("tasks");
    let mut data = default_data();
    let mut actor = LOCAL_OWNER.to_string();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--socket" => socket = args.next().map(PathBuf::from),
            "--data" => data = args.next().map(PathBuf::from),
            "--actor" => actor = args.next().unwrap_or_else(|| fail("--actor needs a value")),
            "-h" | "--help" => {
                println!("{USAGE}");
                return;
            }
            other => fail(&format!("unknown argument {other}")),
        }
    }
    let socket = socket.unwrap_or_else(|| fail("no $XDG_RUNTIME_DIR: pass --socket"));
    let data = data.unwrap_or_else(|| fail("no $XDG_DATA_HOME or $HOME: pass --data"));
    let served = run_until_signal(&socket, || async move {
        std::fs::create_dir_all(&data)?;
        let store = TursoStore::open(data.join("objects.db"))
            .await
            .map_err(std::io::Error::other)?;
        let store: Arc<dyn Store> = Arc::new(store);
        let repo = Arc::new(TaskRepo::new(store, actor));
        Ok(TasksServer::new(repo.clone(), repo).into_client().client)
    });
    if let Err(e) = served {
        eprintln!("fs-tasks-server: {e}");
        std::process::exit(1);
    }
}
