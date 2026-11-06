//! The program as a screen of the shell.
//!
//! Pattern: Adapter — the workbench as a `Surface` of the application shell; the only screen, so the
//! shell draws no navigation. Command — a menu entry is an id, and one handler turns it into a change of
//! the workbench's state. Only what works now is offered (ADR 0056).

use std::sync::Arc;

use fs_components_app::{AppError, AppWidget, Element, Menu, MenuEntry, Screen, Surface};
use fs_components_workbenches::tasks::{TaskSource, TaskView, TasksWidget, Variant};

use crate::words::{TITLE, labels, menu};

const NEW: &str = "task-new";
const LIST: &str = "view-list";
const STATUS: &str = "view-status";

/// The tasks workbench as a surface.
pub struct TasksScreen(pub TasksWidget);

impl Surface for TasksScreen {
    fn build(&self, width: f32, height: f32) -> Box<dyn Element> {
        Box::new(self.0.build(width, height))
    }

    fn styles(&self) -> String {
        self.0.styles()
    }
}

/// The shell around `widget`.
///
/// # Errors
/// [`AppError`] if the shell cannot be made.
pub fn screen_widget(widget: TasksWidget) -> Result<AppWidget, AppError> {
    let screen = Screen::new("tasks", TITLE, Arc::new(TasksScreen(widget.clone())));
    let app = AppWidget::new(TITLE, vec![screen])?;
    app.set_menus(menus_of(&widget), move |id| run(&widget, id))?;
    Ok(app)
}

/// What `widget` can do from a menu: the compact variant has no second view and changes nothing, and a
/// source that cannot create is not offered New.
fn menus_of(widget: &TasksWidget) -> Vec<Menu> {
    let (variant, can_create) = widget.with_state(|s| (s.variant(), s.can_create()));
    let mut menus = Vec::new();
    if can_create {
        menus.push(Menu::new(menu::TASK, vec![MenuEntry::new(NEW, menu::NEW)]));
    }
    if variant == Variant::Full {
        menus.push(Menu::new(
            menu::VIEW,
            vec![
                MenuEntry::new(LIST, menu::LIST),
                MenuEntry::new(STATUS, menu::STATUS),
            ],
        ));
    }
    menus
}

/// One menu entry: runs on the interface thread with no lock of the shell held.
fn run(widget: &TasksWidget, id: &str) {
    match id {
        NEW => {
            widget.with_state(|s| {
                s.begin_create();
            });
        }
        LIST => widget.with_state(|s| s.set_view(TaskView::List)),
        STATUS => widget.with_state(|s| s.set_view(TaskView::Status)),
        _ => return,
    }
    widget.refresh();
}

/// The shell around the workbench over `source`.
///
/// # Errors
/// [`AppError`] if the shell cannot be made.
pub fn app_widget(source: Arc<dyn TaskSource>) -> Result<AppWidget, AppError> {
    screen_widget(TasksWidget::new(source, labels()))
}
