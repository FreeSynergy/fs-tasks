//! The window with its three faces, chosen from the menu "Face" and remembered.
//!
//! Pattern: State + Adapter — one screen of the shell whose surface draws the current face (the classic workbench, the
//! board or the day list); a menu entry is an id, one handler changes the face, puts the menus of that face in place and
//! remembers the choice. The plan faces (and their reading of the store) are made the first time one is shown, so a
//! person who stays with the classic window pays nothing for them.

use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use fs_components_app::{AppError, AppWidget, Element, Menu, MenuEntry, Screen, Surface};
use fs_components_workbenches::tasks::{TaskView, TasksWidget};
use fs_tasks_core::TaskItems;
use tokio::runtime::Handle;

use crate::day::Day;
use crate::face::{Face, Preference};
use crate::plan_view::{Cache, PlanSurface};
use crate::source::TaskAdapter;
use crate::surface::{menus_of, run};
use crate::words::{TITLE, labels, menu};

const PREFIX: &str = "face-";
const FIRST_READING: Duration = Duration::from_secs(600);

/// What the window needs besides the store.
pub struct FaceOptions {
    /// The day that is today.
    pub today: Day,
    /// A picture is drawn once: wait for the first reading of the store.
    pub wait: bool,
    /// The day list to show first (a picture).
    pub list: Option<String>,
    /// The card or line whose title holds this text is opened (a picture).
    pub open: Option<String>,
    /// The view of the classic face to start with.
    pub view: Option<TaskView>,
    /// Where a choice made in the window is remembered; `None` remembers nothing.
    pub remember: Option<Preference>,
}

struct Inner {
    adapter: Arc<TaskAdapter>,
    items: Arc<dyn TaskItems>,
    handle: Handle,
    options: FaceOptions,
    classic: TasksWidget,
    plan: OnceLock<Arc<PlanSurface>>,
    face: Mutex<Face>,
}

impl Inner {
    fn face(&self) -> Face {
        *self.face.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The plan faces over one reading of the store, made on first use.
    fn plan(&self) -> Arc<PlanSurface> {
        Arc::clone(self.plan.get_or_init(|| {
            let cache = Cache::start(
                Arc::clone(&self.items),
                self.handle.clone(),
                self.options.today,
            );
            if self.options.wait {
                cache.wait(FIRST_READING);
            }
            let screen = Arc::new(PlanSurface::new(
                Arc::clone(&self.adapter),
                Arc::clone(&cache),
                self.face() == Face::Day,
            ));
            if let Some(list) = &self.options.list {
                screen.show_list(list);
            }
            if let Some(part) = &self.options.open {
                cache.wait(FIRST_READING);
                screen.select_title(part);
            }
            cache.on_change(fs_components_widgets::plan::redraw);
            screen
        }))
    }

    /// Make `face` the one that is drawn.
    fn show(&self, face: Face) {
        *self.face.lock().unwrap_or_else(PoisonError::into_inner) = face;
        if face != Face::Classic {
            self.plan().set_list(face == Face::Day);
        }
    }
}

struct FaceSurface(Arc<Inner>);

impl Surface for FaceSurface {
    fn build(&self, width: f32, height: f32) -> Box<dyn Element> {
        if self.0.face() == Face::Classic {
            Box::new(self.0.classic.build(width, height))
        } else {
            self.0.plan().build(width, height)
        }
    }

    fn styles(&self) -> String {
        format!(
            "{}{}",
            self.0.classic.styles(),
            fs_components_widgets::styles()
        )
    }

    fn status_line(&self) -> bool {
        self.0.face() == Face::Classic
    }
}

/// The window of the program with its faces.
pub struct Faces {
    app: AppWidget,
    inner: Arc<Inner>,
}

impl Faces {
    /// The window over `items`, showing `start`.
    ///
    /// # Errors
    /// [`AppError`] if the shell cannot be made.
    pub fn new(
        items: Arc<dyn TaskItems>,
        handle: Handle,
        start: Face,
        options: FaceOptions,
    ) -> Result<Self, AppError> {
        let adapter = Arc::new(TaskAdapter::new(Arc::clone(&items), handle.clone()));
        let classic = TasksWidget::new(adapter.clone(), labels());
        if let Some(view) = options.view {
            classic.with_state(|s| s.set_view(view));
        }
        let inner = Arc::new(Inner {
            adapter,
            items,
            handle,
            options,
            classic,
            plan: OnceLock::new(),
            face: Mutex::new(start),
        });
        inner.show(start);
        let screen = Screen::new("tasks", TITLE, Arc::new(FaceSurface(Arc::clone(&inner))));
        let app = AppWidget::new(TITLE, vec![screen])?;
        install(&app, &inner)?;
        Ok(Self { app, inner })
    }

    /// The shell to draw or launch.
    pub fn app(&self) -> &AppWidget {
        &self.app
    }

    /// The face that is shown now.
    pub fn face(&self) -> Face {
        self.inner.face()
    }
}

/// The menus of the face that is shown, and the handler that goes with them.
fn install(app: &AppWidget, inner: &Arc<Inner>) -> Result<(), AppError> {
    let face = inner.face();
    let mut menus = if face == Face::Classic {
        menus_of(&inner.classic)
    } else {
        Vec::new()
    };
    menus.push(Menu::new(
        menu::FACE,
        Face::ALL
            .into_iter()
            .map(|f| {
                let entry = MenuEntry::new(format!("{PREFIX}{}", f.id()), f.label());
                if f == face { entry.disabled() } else { entry }
            })
            .collect(),
    ));
    let (shell, state) = (app.clone(), Arc::clone(inner));
    app.set_menus(menus, move |id| choose(&shell, &state, id))
}

/// One menu entry: a face, or a command of the classic face.
fn choose(app: &AppWidget, inner: &Arc<Inner>, id: &str) {
    let Some(face) = id.strip_prefix(PREFIX).and_then(Face::from_id) else {
        if inner.face() == Face::Classic {
            run(&inner.classic, id);
        }
        return;
    };
    if face == inner.face() {
        return;
    }
    inner.show(face);
    if let Some(memory) = &inner.options.remember
        && let Err(why) = memory.save(face)
    {
        eprintln!("fs-tasks: cannot remember the face: {why}");
    }
    if let Err(why) = install(app, inner) {
        eprintln!("fs-tasks: cannot change the menus: {why}");
    }
}
