//! The window: the standard shell (ADR 0065) around the Admin frontend of `Task` (ADR 0066), the status board and the
//! timeline.
//!
//! Pattern: Facade — one function puts the type description, the backend, the two views and the About page together.
//! No render code: everything that draws is `fs-admin-frontend` and `fs-components-*`. The menus are those of
//! `fs_admin_frontend::window` (same ids, same handler rules) with one View entry per screen.

use std::sync::Arc;

use fs_admin_frontend::view::{AboutInfo, AdminSurface};
use fs_admin_frontend::window::{ABOUT, HELP, QUIT, VIEW_PREFIX};
use fs_admin_frontend::{Session, SystemClock};
use fs_components_app::{AppError, AppWidget, Menu, MenuEntry, Screen, Surface};
use fs_components_fields::{Look, Say};
use fs_components_kanban::KanbanBoard;
use fs_components_styles::StyleId;
use fs_components_timeline::{Timeline, TimelineState};
use fs_components_widgets::open_external;
use fs_tasks_core::{DescribesType, Task, TaskItems};
use tokio::runtime::Handle;

use crate::views::{BoardSurface, TaskBoard, TaskTimeline, TimelineSurface};
use crate::words::words;
use crate::{TaskBackend, frontend_link};

/// Where help is.
pub const HELP_URL: &str = "https://github.com/FreeSynergy/fs-tasks#readme";

/// Fields the form of `change` does not offer (see `backend/commands.rs`).
const READ_ONLY_FIELDS: [&str; 4] = ["participants", "relatedTo", "recurrenceRules", "alerts"];

/// What About says: name, version (from the crate), licence, repository, where it came from.
pub fn about() -> AboutInfo {
    AboutInfo {
        name: "FreeSynergy.Tasks".into(),
        version: format!("v{}", env!("CARGO_PKG_VERSION")),
        origin: "source".into(),
        licence: env!("CARGO_PKG_LICENSE").into(),
        repository: env!("CARGO_PKG_REPOSITORY").into(),
    }
}

/// The screens, in the order of the View menu: id, label key.
const SCREENS: [(&str, &str); 3] = [
    ("Task", "type.task"),
    ("Board", "view.board"),
    ("Timeline", "view.timeline"),
];

fn menus(say: &dyn Say) -> Vec<Menu> {
    vec![
        Menu::new(
            say.say("menu.program"),
            vec![MenuEntry::new(QUIT, say.say("menu.quit"))],
        ),
        Menu::new(
            say.say("menu.view"),
            SCREENS
                .iter()
                .map(|(id, label)| MenuEntry::new(format!("{VIEW_PREFIX}{id}"), say.say(label)))
                .collect(),
        ),
        Menu::new(
            say.say("menu.help"),
            vec![
                MenuEntry::new(HELP, say.say("menu.help")),
                MenuEntry::new(ABOUT, say.say("menu.about")),
            ],
        ),
    ]
}

/// The window over this provider, driven with `runtime`.
///
/// # Errors
/// [`AppError`] if the shell refuses the screens or the menus.
pub fn task_window(items: Arc<dyn TaskItems>, runtime: Handle) -> Result<AppWidget, AppError> {
    let say: Arc<dyn Say> = words();
    let look = Look::new(StyleId::Admin, false);
    let backend = Arc::new(TaskBackend::new(Arc::clone(&items), runtime.clone()));

    let described = Task::describe();
    let description = frontend_link::description(&described, &READ_ONLY_FIELDS);
    let session = Session::new(description, backend.clone(), Arc::new(SystemClock::new()));
    let admin = Arc::new(AdminSurface::new(session, look, Arc::clone(&say), None));

    let board_data = Arc::new(TaskBoard::new(
        Arc::clone(&items),
        runtime.clone(),
        backend,
        Arc::clone(&say),
    ));
    let board = KanbanBoard::new(board_data.clone(), Arc::clone(&say), look).events(board_data);
    let today = crate::views::today();
    let timeline = Timeline::new(
        TimelineState::new(Some(today)),
        Arc::new(TaskTimeline::new(items, runtime)),
        Arc::clone(&say),
        look,
    );

    let surfaces: [Arc<dyn Surface>; 3] = [
        admin.clone(),
        Arc::new(BoardSurface(board)),
        Arc::new(TimelineSurface(timeline)),
    ];
    let screens = SCREENS
        .iter()
        .zip(surfaces)
        .map(|((id, label), surface)| Screen::new((*id).to_owned(), say.say(label), surface))
        .collect();
    let app = AppWidget::new("FreeSynergy.Tasks", screens)?;
    let shell = app.clone();
    app.set_menus(menus(&*say), move |id| match id {
        QUIT => {
            // Nothing is kept unsaved but an open form: with text in one the window stays and says why.
            if admin.has_unsaved() {
                shell.select("Task");
                admin.warn_unsaved();
            } else {
                std::process::exit(0);
            }
        }
        HELP => open_external(HELP_URL),
        ABOUT => {
            shell.select("Task");
            admin.set_about(Some(about()));
        }
        other => {
            if let Some(view) = other.strip_prefix(VIEW_PREFIX) {
                shell.select(view);
            }
        }
    })?;
    Ok(app)
}
