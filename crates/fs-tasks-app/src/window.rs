//! The window: the standard shell (ADR 0065) around the Admin frontend of `Task` (ADR 0066).
//!
//! Pattern: Facade — one function puts the type description, the backend and the About page together. No render code:
//! everything that draws is `fs-admin-frontend` and `fs-components-*`.

use std::sync::Arc;

use fs_admin_frontend::view::{AboutInfo, AdminSurface};
use fs_admin_frontend::window::{TypeView, admin_window};
use fs_admin_frontend::{Backend, Session, SystemClock};
use fs_components_app::{AppError, AppWidget};
use fs_components_fields::{Look, Say};
use fs_components_styles::StyleId;
use fs_tasks_core::{DescribesType, Task};

use crate::frontend_link;
use crate::words::words;

/// Fields the form of `change` does not offer (see `backend/commands.rs`).
const READ_ONLY_FIELDS: [&str; 4] = ["participants", "relatedTo", "recurrenceRules", "alerts"];

/// Where help is.
pub const HELP_URL: &str = "https://github.com/FreeSynergy/fs-tasks#readme";

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

/// The window over this backend.
///
/// # Errors
/// [`AppError`] if the shell refuses the screens.
pub fn task_window(backend: Arc<dyn Backend>) -> Result<AppWidget, AppError> {
    let say: Arc<dyn Say> = words();
    let described = Task::describe();
    let label = described.label.clone();
    let description = frontend_link::description(&described, &READ_ONLY_FIELDS);
    let session = Session::new(description, backend, Arc::new(SystemClock::new()));
    let surface = AdminSurface::new(
        session,
        Look::new(StyleId::Admin, false),
        Arc::clone(&say),
        None,
    );
    let view = TypeView {
        id: "Task".into(),
        label: format!("type.{label}"),
        surface: Arc::new(surface),
    };
    admin_window(
        "FreeSynergy.Tasks",
        &say,
        vec![view],
        about(),
        HELP_URL.into(),
    )
}
