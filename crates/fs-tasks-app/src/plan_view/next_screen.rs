//! Screen B: the day list over the picture of the store, and the surface that holds both screens.
//!
//! Pattern: Adapter (a `Surface`) + Facade (`PlanSurface` picks the screen by the shared mode). State lives here:
//! the list chosen, the lines opened; the widgets only draw and report. Ticking a box finishes (or reopens) the
//! task through the adapter; Enter in the entry line makes a task and renews the picture.

use std::sync::{Arc, Mutex, PoisonError};

use fs_components_app::{Element, Surface};
use fs_components_widgets::plan::{Entry, Events, ListScreen};
use fs_components_workbenches::tasks::TaskSource;

use super::board_screen::BoardScreen;
use super::cache::Cache;
use super::mode::Mode;
use super::next_data::{heading, sections, side};
use crate::source::{TaskAdapter, row_of};

struct State {
    chosen: String,
    opened: Vec<String>,
}

/// The day list.
pub struct NextScreen {
    cache: Arc<Cache>,
    state: Arc<Mutex<State>>,
    entry: Entry,
    events: Events,
}

fn held(s: &Mutex<State>) -> std::sync::MutexGuard<'_, State> {
    s.lock().unwrap_or_else(PoisonError::into_inner)
}

impl NextScreen {
    /// The list over `cache`; changes go through `adapter`; `mode` is shared with the board.
    pub fn new(adapter: Arc<TaskAdapter>, cache: Arc<Cache>, mode: Arc<Mode>) -> Self {
        let state = Arc::new(Mutex::new(State {
            chosen: "today".into(),
            opened: Vec::new(),
        }));
        let mut events = Events::default();
        let (s, m) = (Arc::clone(&state), mode);
        events.pick = Some(Arc::new(move |key| {
            if key == "view:board" {
                m.set_list(false);
            } else {
                held(&s).chosen = key.to_string();
            }
        }));
        let s = Arc::clone(&state);
        events.open = Some(Arc::new(move |key| {
            let mut st = held(&s);
            match st.opened.iter().position(|k| k == key) {
                Some(i) => {
                    st.opened.remove(i);
                }
                None => st.opened.push(key.to_string()),
            }
        }));
        let (a, c) = (Arc::clone(&adapter), Arc::clone(&cache));
        events.tick = Some(Arc::new(move |key| {
            let Some(entry) = c.current().and_then(|p| p.entry(key).cloned()) else {
                return;
            };
            let finished = !crate::source::is_open(&entry.task);
            let to = if finished {
                "needs-action"
            } else {
                "completed"
            };
            if a.move_to(key, to, row_of(&entry).revision.as_deref())
                .is_ok()
            {
                c.changed();
            }
        }));
        let (a, c, s) = (adapter, Arc::clone(&cache), Arc::clone(&state));
        events.enter = Some(Arc::new(move |place, text| {
            if let Ok(mut draft) = a.blank() {
                let today = c.current().map(|p| p.today);
                for f in &mut draft.fields {
                    match f.key.as_str() {
                        "title" => f.value = text.to_string(),
                        "due" if place == "today" => {
                            f.value = today
                                .map(|d| format!("{} 17:00", d.text()))
                                .unwrap_or_default();
                        }
                        "labels" => {
                            f.value = place.strip_prefix("label:").unwrap_or("").to_string();
                        }
                        _ => {}
                    }
                }
                if a.create(&draft).is_ok() {
                    c.changed();
                }
            }
            let _ = &s;
        }));
        Self {
            cache,
            state,
            entry: Entry::new("New to-do"),
            events,
        }
    }
}

impl Surface for NextScreen {
    fn build(&self, width: f32, height: f32) -> Box<dyn Element> {
        let Some(snapshot) = self.cache.current() else {
            let why = self
                .cache
                .problem()
                .unwrap_or_else(|| "Reading the tasks and what blocks them ...".into());
            return Box::new(fs_components_widgets::plan::message(&why, width, height));
        };
        let (chosen, opened) = {
            let st = held(&self.state);
            (st.chosen.clone(), st.opened.clone())
        };
        let (title, note) = heading(&chosen);
        let (items, secs) = (
            side(&snapshot, &chosen),
            sections(&snapshot, &chosen, &opened),
        );
        let screen = ListScreen {
            side: &items,
            title: &title,
            note: Some(&note),
            sections: &secs,
            place: &chosen,
            entry: &self.entry,
            empty: "Nothing here.",
            events: &self.events,
        };
        Box::new(screen.build("fs-plan-list", width, height))
    }

    fn styles(&self) -> String {
        fs_components_widgets::styles()
    }

    fn status_line(&self) -> bool {
        false
    }
}

/// Both screens, one shown at a time.
pub struct PlanSurface {
    board: BoardScreen,
    list: NextScreen,
    mode: Arc<Mode>,
}

impl PlanSurface {
    /// The two screens over one cache; `list` says which one starts.
    pub fn new(adapter: Arc<TaskAdapter>, cache: Arc<Cache>, list: bool) -> Self {
        let mode = Arc::new(Mode::default());
        mode.set_list(list);
        Self {
            board: BoardScreen::new(Arc::clone(&adapter), Arc::clone(&cache), Arc::clone(&mode)),
            list: NextScreen::new(adapter, cache, Arc::clone(&mode)),
            mode,
        }
    }

    /// Choose the card of the board whose title holds `part` (a picture of the open panel).
    pub fn select_title(&self, part: &str) {
        self.board.select_title(part);
    }
}

impl Surface for PlanSurface {
    fn build(&self, width: f32, height: f32) -> Box<dyn Element> {
        if self.mode.is_list() {
            self.list.build(width, height)
        } else {
            self.board.build(width, height)
        }
    }

    fn styles(&self) -> String {
        fs_components_widgets::styles()
    }

    fn status_line(&self) -> bool {
        false
    }
}
