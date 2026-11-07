//! The binary: wiring only.
//!
//! Pattern: none — a thin binary: parse arguments, connect to the server, import, then export or show;
//! everything with behaviour is in the library. The
//! runtime lives as long as the window; the workbench's calls wait on it.

use std::sync::Arc;

use fs_components_app::{Status, WindowOptions, launch};
use fs_components_workbenches::tasks::TasksWidget;
use fs_tasks_app::args::{Args, parse};
use fs_tasks_app::{
    HELP, TaskAdapter, cannot_reach, export_all, import_file, labels, not_exported, screen_widget,
    wrote,
};
use fs_tasks_client::{RevisedTasks, RpcTasks, TaskPages};
use fs_tasks_core::TaskItems;

fn main() {
    if let Err(message) = run() {
        eprintln!("fs-tasks-app: {message}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = parse(&argv)?;
    if args.help {
        println!("{HELP}");
        return Ok(());
    }
    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let socket = args.socket_path()?;
    let tasks = Arc::new(
        RpcTasks::connect(socket)
            .map_err(|e| cannot_reach(&socket.display().to_string(), &e.to_string()))?,
    );
    let items: Arc<dyn TaskItems> = tasks.clone();
    let revised: Arc<dyn RevisedTasks> = tasks.clone();
    let pages: Arc<dyn TaskPages> = tasks;
    let handle = runtime.handle().clone();

    let mut status = None;
    if let Some(file) = &args.import {
        let summary = import_file(items.as_ref(), &handle, file)?;
        eprintln!("{}", summary.message());
        status = Some(if summary.is_clean() {
            Status::Info(summary.message())
        } else {
            Status::Problem(summary.message())
        });
    }
    if let Some(file) = &args.export {
        let report = export_all(items.as_ref(), &handle, file, &args.export_sensitive)?;
        eprintln!("{}", report.message(file));
        if !report.failed.is_empty() {
            return Err(not_exported(report.failed.len()));
        }
        if args.snapshot.is_none() {
            return Ok(());
        }
    }
    show(&args, (items, revised, pages), handle, status)
}

fn show(
    args: &Args,
    (items, revised, pages): (
        Arc<dyn TaskItems>,
        Arc<dyn RevisedTasks>,
        Arc<dyn TaskPages>,
    ),
    handle: tokio::runtime::Handle,
    status: Option<Status>,
) -> Result<(), String> {
    let widget = TasksWidget::new(
        Arc::new(TaskAdapter::new(items, revised, pages, handle)),
        labels(),
    );
    if let Some(view) = args.view {
        widget.with_state(|s| s.set_view(view));
    }
    let app = screen_widget(widget).map_err(|e| e.to_string())?;
    app.set_status(status);
    let (width, height) = args.size;
    if let Some(file) = &args.snapshot {
        app.picture(width, height, file)
            .map_err(|e| e.to_string())?;
        eprintln!("{}", wrote(&file.display().to_string()));
        return Ok(());
    }
    let mut options = WindowOptions::default();
    options.width = width;
    options.height = height;
    launch(options, app).map_err(|e| e.to_string())
}
