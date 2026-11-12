//! The window as a command: what `fs-tasks` and `fs-tasks app` run (ADR 0063).
//!
//! Pattern: Facade consumer — parse arguments, open the store, import, then export or show. The runtime
//! lives as long as the window: the workbench's calls run on it.

use std::sync::Arc;

use crate::args::{Args, HELP, Plan, parse};
use crate::day::Day;
use crate::plan_view::{Board, BoardScreen, Cache, Layout, Next, PlanSource};
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
/// data folder (`$XDG_DATA_HOME/freesynergy/tasks/app.db`): a program started from the store never loses data by default.
/// Pictures and exports use memory, as before, so they touch nothing.
fn store_file(args: &crate::args::Args) -> Result<Option<std::path::PathBuf>, String> {
    if let Some(file) = &args.store {
        return Ok(Some(file.clone()));
    }
    if args.snapshot.is_some() || args.export.is_some() {
        return Ok(None);
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share"))
        })
        .ok_or("cannot find a data folder (HOME is not set); give --store <file>")?;
    let dir = base.join("freesynergy").join("tasks");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(Some(dir.join("app.db")))
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
        .block_on(async {
            match &store_file {
                Some(file) => TursoStore::open(file).await,
                None => TursoStore::open_in_memory().await,
            }
        })
        .map(|s| Arc::new(s) as Arc<dyn Store>)
        .map_err(|e| format!("cannot open the store: {e}"))?;
    let repo = TaskRepo::new(store, ACTOR);
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
    let items: Arc<dyn TaskItems> = Arc::new(repo);
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
    let widget = match args.plan {
        Some(Plan::Board2) => return show_board2(args, items, handle, today),
        Some(plan) => plan_widget(plan, items, handle, today, args.snapshot.is_some()),
        None => TasksWidget::new(Arc::new(TaskAdapter::new(items, handle)), labels()),
    };
    if let Some(view) = args.view {
        widget.with_state(|s| s.set_view(view));
    }
    let app = screen_widget(widget).map_err(|e| e.to_string())?;
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
        Plan::Board => (Box::new(Board), labels(), TaskView::Status),
        Plan::Next => (Box::new(Next), labels_next(), TaskView::List),
        Plan::Board2 => unreachable!("drawn by show_board2"),
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

/// The board drawn by the plan widgets: no workbench, the screen is the whole window.
fn show_board2(
    args: &Args,
    items: Arc<dyn TaskItems>,
    handle: tokio::runtime::Handle,
    today: Day,
) -> Result<(), String> {
    let adapter = Arc::new(TaskAdapter::new(Arc::clone(&items), handle.clone()));
    let cache = Cache::start(items, handle, today);
    if args.snapshot.is_some() {
        cache.wait(std::time::Duration::from_secs(600));
    }
    let screen = Arc::new(BoardScreen::new(adapter, Arc::clone(&cache)));
    if let Some(part) = &args.open {
        cache.wait(std::time::Duration::from_secs(600));
        screen.select_title(part);
    }
    let shell = fs_components_app::AppWidget::new(
        crate::words::TITLE,
        vec![fs_components_app::Screen::new(
            "tasks",
            crate::words::TITLE,
            screen,
        )],
    )
    .map_err(|e| e.to_string())?;
    cache.on_change(fs_components_widgets::plan::redraw);
    let (width, height) = args.size;
    if let Some(file) = &args.snapshot {
        shell
            .picture(width, height, file)
            .map_err(|e| e.to_string())?;
        eprintln!("wrote {}", file.display());
        return Ok(());
    }
    let mut options = WindowOptions::default();
    options.width = width;
    options.height = height;
    launch(options, shell).map_err(|e| e.to_string())
}
