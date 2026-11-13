//! The window as a command: what `fs-tasks` and `fs-tasks app` run (ADR 0063).
//!
//! Pattern: Facade consumer — parse arguments, open the store, import, then export or show. The runtime
//! lives as long as the window: the workbench's calls run on it.

use std::sync::Arc;

use crate::args::{Args, HELP, Plan, parse};
use crate::day::Day;
use crate::face::{Face, Preference};
use crate::faces::{FaceOptions, Faces};
use crate::plan_view::{Board, Cache, Layout, Next, PlanSource};
use crate::sample::by_name as sample_by_name;
use crate::{TaskAdapter, export_all, import_file, labels, labels_next, screen_widget};
use fs_components_app::{Status, WindowOptions, launch};
use fs_components_workbenches::tasks::{TaskView, TasksWidget};
use fs_objects::{Store, TursoStore};
use fs_tasks_core::{TaskItems, TaskRepo};

/// Who acts on this store. Provisional: there is no sign-in yet.
const ACTOR: &str = "https://localhost/actors/owner";

/// Open the window (or import, export, take a picture) with `argv`, the words after `app`; the exit code.
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

/// Where the tasks are kept. `--store` wins. A window that is really used (no picture, no export) keeps them in the
/// data folder (`$XDG_DATA_HOME/freesynergy/tasks/objects.db`, the file of `fs-tasks serve`): a program started from the
/// store never loses data by default.
/// Pictures and exports use memory, as before, so they touch nothing.
fn store_file(args: &crate::args::Args) -> Result<Option<std::path::PathBuf>, String> {
    if let Some(file) = &args.store {
        return Ok(Some(file.clone()));
    }
    if args.snapshot.is_some() || args.export.is_some() {
        return Ok(None);
    }
    let dir =
        data_dir().ok_or("cannot find a data folder (HOME is not set); give --store <file>")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    adopt_legacy_store(&dir)?;
    Ok(Some(dir.join(SHARED_STORE)))
}

/// The store the window, `fs-tasks serve` and the command line share: the file the server opens by default.
const SHARED_STORE: &str = "objects.db";

/// Up to 0.0.7 the window kept its tasks in `app.db`; the server keeps them in `objects.db`. A window that finds
/// only the old file moves it (with its side files) to the shared name, so nothing is lost and nothing is split.
fn adopt_legacy_store(dir: &std::path::Path) -> Result<(), String> {
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

/// The folder of the program's own files: `$XDG_DATA_HOME/freesynergy/tasks`.
fn data_dir() -> Option<std::path::PathBuf> {
    crate::face::data_dir_in(std::env::var_os("XDG_DATA_HOME"), std::env::var_os("HOME"))
}

/// Where the choice of the face is kept. A picture or an export neither reads nor writes it.
fn settings_file(args: &Args) -> Option<std::path::PathBuf> {
    if args.snapshot.is_some() || args.export.is_some() {
        return None;
    }
    data_dir().map(|dir| dir.join("settings.json"))
}

/// Serve the repository at the default socket, if there is one; a problem is said and the window goes on without.
fn answer_workers(repo: Arc<TaskRepo>) -> Option<fs_tasks_server::Hosted> {
    let socket = fs_interfaces::rpc::default_socket("tasks")?;
    match fs_tasks_server::host(repo, socket) {
        Ok(hosted) => Some(hosted),
        Err(e) => {
            eprintln!("fs-tasks: the command line cannot reach this window: {e}");
            None
        }
    }
}

fn run(argv: &[String]) -> Result<(), String> {
    // The typing test reads what is typed into the fields: the registry must be on BEFORE the first field is made, so
    // before the window is built. A debug-only feature, off in a release.
    #[cfg(feature = "test-access")]
    fs_components_widgets::test_access::start_from_env();
    let options = parse(argv)?;
    if options.help {
        println!("{HELP}");
        return Ok(());
    }
    let store_file = store_file(&options)?;
    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let store: Arc<dyn Store> = runtime
        .block_on(async {
            match &store_file {
                Some(file) => TursoStore::open(file).await,
                None => TursoStore::open_in_memory().await,
            }
        })
        .map(|s| Arc::new(s) as Arc<dyn Store>)
        .map_err(|e| {
            let hint = if e.to_string().contains("locked") {
                " (another fs-tasks holds it, e.g. `fs-tasks serve`: stop that one, or close the other window)"
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
    let items: Arc<dyn TaskItems> = repo.clone();
    // The window holds the store (one process only can), so it also answers the command line: workers see and change
    // the tasks the board shows. Only for the shared store; a picture, an export or a file given by --store stays alone.
    let _answering = (options.store.is_none() && store_file.is_some())
        .then(|| answer_workers(repo))
        .flatten();
    let handle = runtime.handle().clone();

    let today = options.today.unwrap_or_else(Day::today);
    if let Some(name) = &options.sample {
        let text = sample_by_name(name, today)?;
        let report = handle
            .block_on(items.import_ics(&text))
            .map_err(|e| format!("cannot import the sample: {e}"))?;
        eprintln!(
            "fs-tasks: sample {name}: {} tasks made, {} failed",
            report.created.len(),
            report.failed.len()
        );
    }
    let mut status = None;
    if let Some(file) = &options.import {
        let summary = import_file(items.as_ref(), &handle, file)?;
        eprintln!("{}", summary.message());
        status = Some(if summary.is_clean() {
            Status::Info(summary.message())
        } else {
            Status::Problem(summary.message())
        });
    }
    if let Some(file) = &options.export {
        let report = export_all(items.as_ref(), &handle, file, &options.export_sensitive)?;
        eprintln!("{}", report.message(file));
        if !report.failed.is_empty() {
            return Err(format!("{} could not be exported", report.failed.len()));
        }
        if options.snapshot.is_none() {
            return Ok(());
        }
    }
    show(&options, items, handle, status, today)
}

fn show(
    args: &Args,
    items: Arc<dyn TaskItems>,
    handle: tokio::runtime::Handle,
    status: Option<Status>,
    today: Day,
) -> Result<(), String> {
    let app = match args.plan {
        Some(plan @ (Plan::OldBoard | Plan::OldNext)) => {
            let widget = plan_widget(plan, items, handle, today, args.snapshot.is_some());
            if let Some(view) = args.view {
                widget.with_state(|s| s.set_view(view));
            }
            screen_widget(widget).map_err(|e| e.to_string())?
        }
        _ => faces_window(args, items, handle, today)?,
    };
    app.set_status(status);
    let (width, height) = args.size;
    if let Some(file) = &args.snapshot {
        app.picture(width, height, file)
            .map_err(|e| e.to_string())?;
        eprintln!("wrote {}", file.display());
        return Ok(());
    }
    let mut options = WindowOptions::default();
    options.width = width;
    options.height = height;
    launch(options, app).map_err(|e| e.to_string())
}

/// The widget of a plan view. A picture waits for the first reading of the store; a window opens at once and
/// fills when it arrives (the cache tells the window, which reads the list again).
fn plan_widget(
    plan: Plan,
    items: Arc<dyn TaskItems>,
    handle: tokio::runtime::Handle,
    today: Day,
    wait: bool,
) -> TasksWidget {
    let adapter = Arc::new(TaskAdapter::new(Arc::clone(&items), handle.clone()));
    let cache = Cache::start(items, handle, today);
    if wait {
        let started = std::time::Instant::now();
        let picture = cache.wait(std::time::Duration::from_secs(600));
        let count = picture.map_or(0, |p| p.entries.len());
        eprintln!(
            "fs-tasks: picture of {count} tasks with their readiness in {:?}",
            started.elapsed()
        );
    }
    let (layout, words, view): (Box<dyn Layout>, _, _) = match plan {
        Plan::OldBoard => (Box::new(Board), labels(), TaskView::Status),
        Plan::OldNext => (Box::new(Next), labels_next(), TaskView::List),
        Plan::Classic | Plan::Board | Plan::Next => unreachable!("drawn by faces_window"),
    };
    let source = PlanSource::new(adapter, Arc::clone(&cache), layout);
    let widget = TasksWidget::new(Arc::new(source), words);
    widget.with_state(|s| s.set_view(view));
    let again = widget.clone();
    cache.on_change(move || {
        again.with_state(|s| {
            let term = s.term().to_string();
            s.search(&term);
        });
        again.refresh();
    });
    widget
}

/// The window with its three faces. It starts with the face `--plan` names (this time only), else with the one
/// remembered, else with the default; a choice made in its menu is remembered.
fn faces_window(
    args: &Args,
    items: Arc<dyn TaskItems>,
    handle: tokio::runtime::Handle,
    today: Day,
) -> Result<fs_components_app::AppWidget, String> {
    let remember = settings_file(args).map(Preference::new);
    let start = args
        .plan
        .and_then(Plan::face)
        .or_else(|| remember.as_ref().map(Preference::load))
        .unwrap_or(Face::DEFAULT);
    let options = FaceOptions {
        today,
        wait: args.snapshot.is_some(),
        list: args.list.clone(),
        open: args.open.clone(),
        view: args.view,
        remember,
    };
    let faces = Faces::new(items, handle, start, options).map_err(|e| e.to_string())?;
    Ok(faces.app().clone())
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
}
