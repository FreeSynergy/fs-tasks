//! The window as a command: what `fs-tasks` and `fs-tasks app` run (ADR 0063).
//!
//! Pattern: Facade consumer — parse arguments, get the tasks (from the server, see `open`), import, then export or
//! show. The runtime lives as long as the window: the workbench's calls run on it.

use std::sync::Arc;

use crate::args::{Args, HELP, Plan, parse};
use crate::day::Day;
use crate::frontend::{Frontend, Preference};
use crate::frontends::{FrontendOptions, Frontends};
use crate::open::{data_dir, open};
use crate::plan_view::{Board, Cache, Layout, Next, PlanSource};
use crate::sample::by_name as sample_by_name;
use crate::{TaskAdapter, export_all, import_file, labels, labels_next, screen_widget};
use fs_components_app::{Status, WindowOptions, launch};
use fs_components_workbenches::tasks::{TaskView, TasksWidget};
use fs_tasks_core::TaskItems;
use fs_tasks_server::Changes;

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

/// Where the choice of the frontend is kept. A picture or an export neither reads nor writes it.
fn settings_file(args: &Args) -> Option<std::path::PathBuf> {
    if args.snapshot.is_some() || args.export.is_some() {
        return None;
    }
    data_dir().map(|dir| dir.join("settings.json"))
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
    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let opened = open(&options, &runtime)?;
    let items = opened.items;
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
    show(&options, items, handle, status, today, opened.changes)
}

fn show(
    args: &Args,
    items: Arc<dyn TaskItems>,
    handle: tokio::runtime::Handle,
    status: Option<Status>,
    today: Day,
    live: Option<Arc<Changes>>,
) -> Result<(), String> {
    let app = match args.plan {
        Some(plan @ (Plan::OldBoard | Plan::OldNext)) => {
            let widget = plan_widget(plan, items, handle, today, args.snapshot.is_some());
            if let Some(view) = args.view {
                widget.with_state(|s| s.set_view(view));
            }
            screen_widget(widget).map_err(|e| e.to_string())?
        }
        _ => frontends_window(args, items, handle, today, live)?,
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
        Plan::Classic | Plan::Board | Plan::Next => unreachable!("drawn by frontends_window"),
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

/// The window with its three frontends. It starts with the frontend `--plan` names (this time only), else with the one
/// remembered, else with the default; a choice made in its menu is remembered.
fn frontends_window(
    args: &Args,
    items: Arc<dyn TaskItems>,
    handle: tokio::runtime::Handle,
    today: Day,
    live: Option<Arc<Changes>>,
) -> Result<fs_components_app::AppWidget, String> {
    let remember = settings_file(args).map(Preference::new);
    let start = args
        .plan
        .and_then(Plan::frontend)
        .or_else(|| remember.as_ref().map(Preference::load))
        .unwrap_or(Frontend::DEFAULT);
    let options = FrontendOptions {
        today,
        wait: args.snapshot.is_some(),
        list: args.list.clone(),
        open: args.open.clone(),
        view: args.view,
        remember,
        live,
    };
    let frontends = Frontends::new(items, handle, start, options).map_err(|e| e.to_string())?;
    Ok(frontends.app().clone())
}
