//! Where the window's tasks come from: the server (the motor) over the socket, or a store of its own.
//!
//! Pattern: Strategy — one motor per device holds the data (`fs-tasks serve`); the window is a client of it, like the
//! command line and the agents, and starts it if none runs. Only an explicit `--store` file, a picture or an export
//! without `--connect` keep the data in the process (memory or that file), so they touch nothing else.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs_interfaces::rpc::default_socket;
use fs_objects::{Store, TursoStore};
use fs_tasks_core::{TaskItems, TaskRepo};
use fs_tasks_server::{Changes, RemoteItems};

use crate::args::Args;

/// Who acts on a store the window holds itself. Provisional: there is no sign-in yet.
const ACTOR: &str = "https://localhost/actors/owner";

/// The store the window, `fs-tasks serve` and the command line share: the file the server opens by default.
const SHARED_STORE: &str = "objects.db";

/// The tasks and, for a window that is a client, the change notices of the server.
pub struct Opened {
    /// The provider the window reads and writes.
    pub items: Arc<dyn TaskItems>,
    /// Tells when anybody changed the tasks; `None` for a store held in the process.
    pub changes: Option<Arc<Changes>>,
}

/// Whether the window is a client of the server: with `--connect`, or by default (no `--store`, no picture, no export).
fn is_client(args: &Args) -> bool {
    args.connect.is_some()
        || (args.store.is_none() && args.snapshot.is_none() && args.export.is_none())
}

/// The data folder of the program: `$XDG_DATA_HOME/freesynergy/tasks`.
pub(crate) fn data_dir() -> Option<PathBuf> {
    crate::frontend::data_dir_in(std::env::var_os("XDG_DATA_HOME"), std::env::var_os("HOME"))
}

/// Up to 0.0.7 the window kept its tasks in `app.db`; the server keeps them in `objects.db`. A window that finds
/// only the old file moves it (with its side files) to the shared name, so nothing is lost and nothing is split.
fn adopt_legacy_store(dir: &Path) -> Result<(), String> {
    if dir.join(SHARED_STORE).exists() || !dir.join("app.db").exists() {
        return Ok(());
    }
    let named = |e: std::io::Error| format!("{}: {e}", dir.display());
    for entry in std::fs::read_dir(dir).map_err(named)? {
        let from = entry.map_err(named)?.path();
        let name = from.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if let Some(rest) = name.strip_prefix("app.db") {
            std::fs::rename(&from, dir.join(format!("{SHARED_STORE}{rest}"))).map_err(named)?;
        }
    }
    Ok(())
}

/// Connect to the server (starting it if none answers). The window never opens the data file in this case.
fn client(args: &Args) -> Result<Opened, String> {
    let socket = args
        .connect
        .clone()
        .or_else(|| default_socket("tasks"))
        .ok_or("no socket: give --connect <socket> or set XDG_RUNTIME_DIR")?;
    if args.connect.is_none()
        && let Some(dir) = data_dir()
    {
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        adopt_legacy_store(&dir)?;
    }
    let remote = RemoteItems::connect(socket, None).map_err(|e| format!("tasks server: {e}"))?;
    let changes = remote.changes();
    Ok(Opened {
        items: Arc::new(remote),
        changes: Some(changes),
    })
}

/// A store in this process: `--store`, or memory (a picture, an export).
fn own(args: &Args, runtime: &tokio::runtime::Runtime) -> Result<Opened, String> {
    let store: Arc<dyn Store> = runtime
        .block_on(async {
            match &args.store {
                Some(file) => TursoStore::open(file).await,
                None => TursoStore::open_in_memory().await,
            }
        })
        .map(|s| Arc::new(s) as Arc<dyn Store>)
        .map_err(|e| {
            let hint = if e.to_string().contains("locked") {
                " (another fs-tasks holds it, e.g. `fs-tasks serve`: use --connect <socket> to use that one)"
            } else {
                ""
            };
            format!("cannot open the store: {e}{hint}")
        })?;
    let repo = Arc::new(TaskRepo::new(store, ACTOR));
    // Tasks stored by 0.0.4 or before have no dependency fields in their index (and 0.6.0 ones no order key): complete
    // them as the server does, so the window sees the same readiness; what cannot be repaired is named.
    let report = runtime
        .block_on(repo.upgrade_index())
        .map_err(|e| format!("cannot complete the index: {e}"))?;
    if report.rewritten > 0 {
        eprintln!("fs-tasks: index completed for {} tasks", report.rewritten);
    }
    for failed in &report.failed {
        eprintln!("fs-tasks: task not repaired: {failed}");
    }
    Ok(Opened {
        items: repo,
        changes: None,
    })
}

/// The tasks for `args`.
///
/// # Errors
/// The text says why the server or the store could not be reached.
pub fn open(args: &Args, runtime: &tokio::runtime::Runtime) -> Result<Opened, String> {
    if is_client(args) {
        client(args)
    } else {
        own(args, runtime)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_old_app_db_moves_to_the_shared_name_with_its_side_files_and_never_over_a_new_one() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["app.db", "app.db-wal", "settings.json"] {
            std::fs::write(dir.path().join(name), name).unwrap();
        }
        adopt_legacy_store(dir.path()).unwrap();
        let read = |n: &str| std::fs::read_to_string(dir.path().join(n)).ok();
        assert_eq!(read("objects.db").as_deref(), Some("app.db"));
        assert_eq!(read("objects.db-wal").as_deref(), Some("app.db-wal"));
        assert!(read("app.db").is_none() && read("settings.json").is_some());
        // With a shared store in place an old file is left alone: nothing is overwritten.
        std::fs::write(dir.path().join("app.db"), "old").unwrap();
        adopt_legacy_store(dir.path()).unwrap();
        assert_eq!(read("objects.db").as_deref(), Some("app.db"));
        assert_eq!(read("app.db").as_deref(), Some("old"));
    }

    #[test]
    fn the_window_is_a_client_unless_it_keeps_a_store_of_its_own() {
        let plain = Args::default();
        assert!(is_client(&plain));
        let with = |f: fn(&mut Args)| {
            let mut a = Args::default();
            f(&mut a);
            a
        };
        assert!(!is_client(&with(|a| a.store = Some("x.db".into()))));
        assert!(!is_client(&with(|a| a.snapshot = Some("x.png".into()))));
        assert!(!is_client(&with(|a| a.export = Some("x.ics".into()))));
        // `--connect` wins: a picture of what the server holds.
        assert!(is_client(&with(|a| {
            a.snapshot = Some("x.png".into());
            a.connect = Some("s.sock".into());
        })));
    }
}
