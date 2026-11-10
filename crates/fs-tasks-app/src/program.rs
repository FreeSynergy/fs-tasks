//! The window as a command: what `fs-tasks` and `fs-tasks app` run (ADR 0063).
//!
//! Pattern: Facade consumer — parse arguments, open the store, show the window. The runtime
//! lives as long as the window: the workbench's calls run on it.

use std::sync::Arc;

use crate::args::{HELP, parse};
use crate::task_window;
use fs_components_app::{WindowOptions, launch};
use fs_objects::{Store, TursoStore};
use fs_tasks_core::{TaskItems, TaskRepo};

/// Who acts on this store. Provisional: there is no sign-in yet.
const ACTOR: &str = "https://localhost/actors/owner";

/// Open the window  with `argv`, the words after `app`; the exit code.
#[must_use]
pub fn app_command(argv: &[String]) -> i32 {
    match run(argv) {
        Ok(()) => 0,
        Err(message) => {
            eprintln!("fs-tasks: {message}");
            1
        }
    }
}

/// Where the tasks are kept. `--store` wins, otherwise the data folder (`$XDG_DATA_HOME/freesynergy/tasks/app.db`): a program started
/// from the store never loses data by default.
fn store_file(args: &crate::args::Args) -> Result<std::path::PathBuf, String> {
    if let Some(file) = &args.store {
        return Ok(file.clone());
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share"))
        })
        .ok_or("cannot find a data folder (HOME is not set); give --store <file>")?;
    let dir = base.join("freesynergy").join("tasks");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir.join("app.db"))
}

fn run(argv: &[String]) -> Result<(), String> {
    let options = parse(argv)?;
    if options.help {
        println!("{HELP}");
        return Ok(());
    }
    let store_file = store_file(&options)?;
    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let store: Arc<dyn Store> = runtime
        .block_on(TursoStore::open(&store_file))
        .map(|s| Arc::new(s) as Arc<dyn Store>)
        .map_err(|e| format!("cannot open the store: {e}"))?;
    let items: Arc<dyn TaskItems> = Arc::new(TaskRepo::new(store, ACTOR));
    let app = task_window(items, runtime.handle().clone()).map_err(|e| e.to_string())?;
    let mut window = WindowOptions::default();
    window.width = options.size.0;
    window.height = options.size.1;
    launch(window, app).map_err(|e| e.to_string())
}
