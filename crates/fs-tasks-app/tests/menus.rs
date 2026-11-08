//! The menus: only real commands, and an entry really changes the workbench's state.
//!
//! Pattern: Command tests over the real core, through the shell's own way of choosing an entry.

mod common;

use common::World;
use fs_components_workbenches::tasks::{TaskView, TasksWidget};
use fs_tasks_app::{labels, screen_widget};

fn labels_of(app: &fs_components_app::AppWidget) -> Vec<String> {
    app.with_shell(|shell| {
        shell
            .menus()
            .iter()
            .flat_map(|menu| {
                let lines = menu.entries.iter().map(|e| e.label.clone());
                std::iter::once(menu.label.clone()).chain(lines)
            })
            .collect()
    })
}

#[test]
fn view_entries_change_the_view_through_the_workbench_state() {
    let w = World::new();
    let widget = TasksWidget::new(w.adapter.clone(), labels());
    let app = screen_widget(widget.clone()).unwrap();
    assert_eq!(widget.context().view, TaskView::List);
    assert!(app.choose("view-status"));
    assert_eq!(widget.context().view, TaskView::Status);
    assert!(app.choose("view-list"));
    assert_eq!(widget.context().view, TaskView::List);
}

#[test]
fn new_starts_a_new_task_and_an_edit_in_progress_is_not_thrown_away() {
    let w = World::new();
    let key = w.plain("Plain");
    let widget = TasksWidget::new(w.adapter.clone(), labels());
    let app = screen_widget(widget.clone()).unwrap();
    assert!(app.choose("task-new"));
    assert_eq!(
        widget.with_state(|s| s.editing().map(|e| e.key.clone())),
        Some(None)
    );
    widget.with_state(|s| {
        s.cancel_edit();
        s.select(&key);
        s.begin_edit();
        s.set_field("title", "Typed");
    });
    assert!(app.choose("task-new"));
    let draft = widget.with_state(|s| {
        s.editing()
            .unwrap()
            .draft
            .value("title")
            .map(str::to_string)
    });
    assert_eq!(draft.as_deref(), Some("Typed"), "unsaved changes block it");
    assert!(widget.with_state(|s| s.notice().is_some()));
}

#[test]
fn the_entries_are_new_list_and_status_and_nothing_else() {
    let w = World::new();
    let app = fs_tasks_app::app_widget(w.adapter.clone()).unwrap();
    assert_eq!(labels_of(&app), ["Task", "New", "View", "List", "Status"]);
    assert!(!app.choose("task-delete"), "no other entry");
}

#[test]
fn the_compact_variant_has_no_menu_that_could_do_nothing() {
    let w = World::new();
    let app = screen_widget(TasksWidget::compact(w.adapter.clone(), labels())).unwrap();
    assert_eq!(labels_of(&app), [] as [std::string::String; 0]);
}
