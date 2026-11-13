//! The window with its three frontends, chosen from the menu "Frontend" and remembered.
//!
//! Pattern: State + Adapter — one screen of the shell whose surface draws the current frontend (the classic workbench, the
//! board or the day list); a menu entry is an id, one handler changes the frontend, puts the menus of that frontend in place and
//! remembers the choice. The plan frontends (and their reading of the store) are made the first time one is shown, so a
//! person who stays with the classic window pays nothing for them.

use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use fs_components_app::{AppError, AppWidget, Element, Menu, MenuEntry, Screen, Surface};
use fs_components_workbenches::tasks::{TaskView, TasksWidget};
use fs_tasks_core::TaskItems;
use fs_tasks_server::Changes;
use tokio::runtime::Handle;

use crate::day::Day;
use crate::frontend::{Frontend, Preference};
use crate::plan_view::{Cache, PlanSurface};
use crate::source::TaskAdapter;
use crate::surface::{menus_of, run};
use crate::words::{TITLE, labels, menu};

const PREFIX: &str = "frontend-";
const FIRST_READING: Duration = Duration::from_secs(600);

/// What the window needs besides the store.
pub struct FrontendOptions {
    /// The day that is today.
    pub today: Day,
    /// A picture is drawn once: wait for the first reading of the store.
    pub wait: bool,
    /// The day list to show first (a picture).
    pub list: Option<String>,
    /// The card or line whose title holds this text is opened (a picture).
    pub open: Option<String>,
    /// The view of the classic frontend to start with.
    pub view: Option<TaskView>,
    /// Where a choice made in the window is remembered; `None` remembers nothing.
    pub remember: Option<Preference>,
    /// Change notices of the server (the window is a client): both frontends read again when anybody changes a task.
    pub live: Option<Arc<Changes>>,
}

struct Inner {
    adapter: Arc<TaskAdapter>,
    items: Arc<dyn TaskItems>,
    handle: Handle,
    options: FrontendOptions,
    classic: TasksWidget,
    plan: OnceLock<Arc<PlanSurface>>,
    cache: OnceLock<Arc<Cache>>,
    frontend: Mutex<Frontend>,
}

impl Inner {
    fn frontend(&self) -> Frontend {
        *self.frontend.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The plan frontends over one reading of the store, made on first use.
    fn plan(&self) -> Arc<PlanSurface> {
        Arc::clone(self.plan.get_or_init(|| {
            let cache = Cache::start(
                Arc::clone(&self.items),
                self.handle.clone(),
                self.options.today,
            );
            let _ = self.cache.set(Arc::clone(&cache));
            if self.options.wait {
                cache.wait(FIRST_READING);
            }
            let screen = Arc::new(PlanSurface::new(
                Arc::clone(&self.adapter),
                Arc::clone(&cache),
                self.frontend() == Frontend::Day,
            ));
            if let Some(list) = &self.options.list {
                screen.show_list(list);
            }
            if let Some(part) = &self.options.open {
                cache.wait(FIRST_READING);
                screen.select_title(part);
            }
            cache.on_change(fs_components_widgets::plan::redraw);
            if let Some(live) = &self.options.live {
                // A weak handle: a strong one would make a cycle (changes, callback, cache, items) and the connection thread would never end.
                let cache = Arc::downgrade(&cache);
                live.on_change(move || {
                    if let Some(cache) = cache.upgrade() {
                        cache.reload_in_background();
                    }
                });
            }
            screen
        }))
    }

    /// Make `frontend` the one that is drawn.
    fn show(&self, frontend: Frontend) {
        *self.frontend.lock().unwrap_or_else(PoisonError::into_inner) = frontend;
        if frontend != Frontend::Classic {
            self.plan().set_list(frontend == Frontend::Day);
        }
    }
}

struct FrontendSurface(Arc<Inner>);

impl Surface for FrontendSurface {
    fn build(&self, width: f32, height: f32) -> Box<dyn Element> {
        if self.0.frontend() == Frontend::Classic {
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
        self.0.frontend() == Frontend::Classic
    }
}

/// The window of the program with its frontends.
pub struct Frontends {
    app: AppWidget,
    inner: Arc<Inner>,
}

impl Frontends {
    /// The window over `items`, showing `start`.
    ///
    /// # Errors
    /// [`AppError`] if the shell cannot be made.
    pub fn new(
        items: Arc<dyn TaskItems>,
        handle: Handle,
        start: Frontend,
        options: FrontendOptions,
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
            cache: OnceLock::new(),
            frontend: Mutex::new(start),
        });
        inner.show(start);
        if let Some(live) = &inner.options.live {
            // Limit: `refresh` is only a redraw in fs-components v0.0.13, so the classic frontend reads its list again at its next action, not live (live: board and day list). Weak handle: the window may be gone.
            let weak = Arc::downgrade(&inner);
            live.on_change(move || {
                if let Some(inner) = weak.upgrade() {
                    inner.classic.refresh();
                }
            });
        }
        let screen = Screen::new(
            "tasks",
            TITLE,
            Arc::new(FrontendSurface(Arc::clone(&inner))),
        );
        let app = AppWidget::new(TITLE, vec![screen])?;
        install(&app, &inner)?;
        Ok(Self { app, inner })
    }

    /// The shell to draw or launch.
    pub fn app(&self) -> &AppWidget {
        &self.app
    }

    /// The reading of the store behind Board and Day list, once one of them has been shown (what they display).
    pub fn cache(&self) -> Option<Arc<Cache>> {
        self.inner.cache.get().cloned()
    }

    /// The frontend that is shown now.
    pub fn frontend(&self) -> Frontend {
        self.inner.frontend()
    }
}

/// The menus of the frontend that is shown, and the handler that goes with them.
fn install(app: &AppWidget, inner: &Arc<Inner>) -> Result<(), AppError> {
    let frontend = inner.frontend();
    let mut menus = if frontend == Frontend::Classic {
        menus_of(&inner.classic)
    } else {
        Vec::new()
    };
    menus.push(Menu::new(
        menu::FRONTEND,
        Frontend::ALL
            .into_iter()
            .map(|f| {
                let entry = MenuEntry::new(format!("{PREFIX}{}", f.id()), f.label());
                if f == frontend {
                    entry.disabled()
                } else {
                    entry
                }
            })
            .collect(),
    ));
    let (shell, state) = (app.clone(), Arc::clone(inner));
    app.set_menus(menus, move |id| choose(&shell, &state, id))
}

/// One menu entry: a frontend, or a command of the classic frontend.
fn choose(app: &AppWidget, inner: &Arc<Inner>, id: &str) {
    let Some(frontend) = id.strip_prefix(PREFIX).and_then(Frontend::from_id) else {
        if inner.frontend() == Frontend::Classic {
            run(&inner.classic, id);
        }
        return;
    };
    if frontend == inner.frontend() {
        return;
    }
    inner.show(frontend);
    if let Some(memory) = &inner.options.remember
        && let Err(why) = memory.save(frontend)
    {
        eprintln!("fs-tasks: cannot remember the frontend: {why}");
    }
    if let Err(why) = install(app, inner) {
        eprintln!("fs-tasks: cannot change the menus: {why}");
    }
}
