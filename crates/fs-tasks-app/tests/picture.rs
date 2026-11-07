//! The whole program as a picture, through the shell's real path. No window is opened.
//!
//! Skipped with a message where there is no graphics adapter. Pictures are kept
//! where `FS_COMP_SHOTS` says. A hang here is a deadlock.

mod common;

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use common::{World, series};
use fs_components_app::AppError;
use fs_components_render::{Canvas, RenderError, draw_to_png};
use fs_components_workbenches::tasks::{TaskView, TasksWidget};
use fs_tasks_app::{labels, screen_widget};
use serde_json::json;

/// `draw_to_png` holds the process-wide lock; nothing else may set up Blinc's runtime meanwhile.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner)
}

fn out(name: &str) -> PathBuf {
    let dir = std::env::var_os("FS_COMP_SHOTS").map_or_else(
        || std::env::temp_dir().join(format!("fs-tasks-app-{}", std::process::id())),
        PathBuf::from,
    );
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

/// The picture's bytes; it stays only where `FS_COMP_SHOTS` asks for it.
fn read_and_clear(path: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap();
    if std::env::var_os("FS_COMP_SHOTS").is_none() {
        std::fs::remove_file(path).unwrap();
        let _ = std::fs::remove_dir(path.parent().unwrap());
    }
    bytes
}

fn world() -> World {
    let w = World::new();
    let task = |title: &str, due: Option<&str>, progress: &str, extra: serde_json::Value| {
        let uid = format!("uid-{}", title.replace(' ', "-"));
        let mut value = json!({"@type": "Task", "version": "2.0", "uid": uid, "title": title,
            "progress": progress});
        if let Some(due) = due {
            value["due"] = due.into();
        }
        for (k, v) in extra.as_object().unwrap() {
            value[k] = v.clone();
        }
        value
    };
    w.add_json(task(
        "Send the minutes",
        Some("2026-11-12T17:00:00"),
        "needs-action",
        json!({"priority": 1, "keywords": {"office": true, "minutes": true},
            "description": "To everyone who was there."}),
    ));
    w.add_json(task(
        "Budget planning with the very long title that cannot fit on one line of a card",
        Some("2026-11-20T12:00:00"),
        "in-process",
        json!({"timeZone": "Europe/Berlin", "percentComplete": 40}),
    ));
    w.add_json(task(
        "Appointment at the clinic",
        Some("2026-11-13T08:00:00"),
        "needs-action",
        json!({"freesynergy.net:sensitive": ["pd:HealthRecord"]}),
    ));
    w.add_json(task("Paid the invoice", None, "completed", json!({})));
    w.add_json(task("Move the server", None, "failed", json!({})));
    w.add_json(task("Old idea", None, "cancelled", json!({})));
    w.add_json(series());
    w
}

fn draw(app: &fs_components_app::AppWidget, name: &str, size: (u32, u32)) -> Option<Vec<u8>> {
    let path = out(name);
    match app.picture(size.0, size.1, &path) {
        Err(AppError::Picture(RenderError::NoGraphics(why))) => {
            eprintln!("skipped: {why}");
            None
        }
        other => {
            other.unwrap();
            Some(read_and_clear(&path))
        }
    }
}

#[test]
fn the_program_draws_through_picture_and_through_the_live_build_alike() {
    let _one = serial();
    let w = world();
    let app = fs_tasks_app::app_widget(w.adapter.clone()).unwrap();
    let Some(once) = draw(&app, "tasks-once.png", (1300, 800)) else {
        return;
    };
    let live = out("tasks-live.png");
    let canvas = Canvas::new(1300, 800, app.styles());
    draw_to_png(
        &canvas,
        |width, height| app.build(width, height),
        || {},
        &live,
    )
    .unwrap();
    assert_eq!(
        once,
        read_and_clear(&live),
        "the window's path draws the same"
    );
}

#[test]
fn the_views_the_form_and_a_marked_task_are_drawn() {
    let _one = serial();
    let w = world();
    let widget = TasksWidget::new(w.adapter.clone(), labels());
    let app = screen_widget(widget.clone()).unwrap();
    let Some(list) = draw(&app, "tasks-list.png", (1300, 800)) else {
        return;
    };
    widget.with_state(|s| s.set_view(TaskView::Status));
    let status = draw(&app, "tasks-status.png", (1300, 800)).unwrap();
    assert_ne!(list, status);
    let clinic = widget.with_state(|s| {
        s.rows()
            .iter()
            .find(|r| r.title.contains("clinic"))
            .unwrap()
            .key
            .clone()
    });
    widget.with_state(|s| s.select(&clinic));
    let marked = draw(&app, "tasks-marked.png", (1300, 800)).unwrap();
    assert_ne!(status, marked);
    widget.with_state(|s| s.begin_edit());
    let form = draw(&app, "tasks-form.png", (1300, 800)).unwrap();
    assert_ne!(marked, form);
    widget.with_state(|s| {
        s.cancel_edit();
        s.set_view(TaskView::List);
        s.begin_create();
    });
    let new_form = draw(&app, "tasks-new.png", (1300, 800)).unwrap();
    assert_ne!(form, new_form);
    widget.with_state(|s| {
        s.cancel_edit();
        s.set_view(TaskView::Status);
    });
    let narrow = draw(&app, "tasks-status-narrow.png", (420, 800)).unwrap();
    assert!(!narrow.is_empty());
    let compact = TasksWidget::compact(w.adapter.clone(), labels());
    let small = screen_widget(compact).unwrap();
    let next = draw(&small, "tasks-compact.png", (420, 600)).unwrap();
    assert!(!next.is_empty());
}

#[test]
fn the_smallest_window_is_drawn_in_every_state_with_its_actions() {
    let _one = serial();
    let w = world();
    let widget = TasksWidget::new(w.adapter.clone(), labels());
    let app = screen_widget(widget.clone()).unwrap();
    let size = fs_components_app::MIN_WINDOW;
    if draw(&app, "tasks-min-list.png", size).is_none() {
        return;
    }
    widget.with_state(|s| s.set_view(TaskView::Status));
    draw(&app, "tasks-min-status.png", size).unwrap();
    widget.with_state(|s| s.ask_delete());
    draw(&app, "tasks-min-delete.png", size).unwrap();
    widget.with_state(|s| {
        s.keep();
        s.begin_edit();
    });
    draw(&app, "tasks-min-form.png", size).unwrap();
}
