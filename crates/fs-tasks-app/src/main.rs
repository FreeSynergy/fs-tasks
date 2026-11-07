//! The binary: wiring only.
//!
//! Pattern: Facade consumer — parse arguments, connect to the server, import, then export or show. The
//! runtime lives as long as the window; the workbench's calls wait on it.

use std::sync::Arc;

use fs_components_app::{Status, WindowOptions, launch};
use fs_components_workbenches::tasks::TasksWidget;
use fs_tasks_app::args::{Args, HELP, parse};
use fs_tasks_app::{TaskAdapter, export_all, import_file, labels, screen_widget};
use fs_tasks_client::RpcTasks;
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
    let items: Arc<dyn TaskItems> = Arc::new(
        RpcTasks::connect(socket)
            .map_err(|e| format!("cannot reach the tasks server at {}: {e}", socket.display()))?,
    );
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
            return Err(format!("{} could not be exported", report.failed.len()));
        }
        if args.snapshot.is_none() {
            return Ok(());
        }
    }
    show(&args, items, handle, status)
}

fn show(
    args: &Args,
    items: Arc<dyn TaskItems>,
    handle: tokio::runtime::Handle,
    status: Option<Status>,
) -> Result<(), String> {
    let widget = TasksWidget::new(Arc::new(TaskAdapter::new(items, handle)), labels());
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
