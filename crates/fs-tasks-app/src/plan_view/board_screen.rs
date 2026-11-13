//! Screen A: the board as the plan widgets draw it, over the picture of the store.
//!
//! Pattern: Adapter — the screen is a `Surface`; its state (the chosen card, the column being added to, the label
//! shown) lives here, the widgets only draw and report. Reading is from the cache, so drawing never waits for the
//! core; creating goes through the adapter's form and renews the picture.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use fs_components_app::{Element, Surface};
use fs_components_widgets::plan::{BoardView, Column, Entry, Events, SideItem, Tab};
use fs_components_workbenches::tasks::TaskSource;
use fs_tasks_core::validate::PROGRESS;

use super::cache::Cache;
use super::cards::{card, panel};
use super::snapshot::Snapshot;
use crate::source::{TaskAdapter, label_list};
use crate::words::progress_title;

#[derive(Default)]
struct State {
    selected: Option<String>,
    creating: Option<String>,
    label: Option<String>,
}

/// The board screen.
pub struct BoardScreen {
    adapter: Arc<TaskAdapter>,
    cache: Arc<Cache>,
    state: Arc<Mutex<State>>,
    entry: Entry,
    events: Events,
}

fn held(s: &Mutex<State>) -> std::sync::MutexGuard<'_, State> {
    s.lock().unwrap_or_else(PoisonError::into_inner)
}

impl BoardScreen {
    /// The board over `cache`; creating a task goes through `adapter`.
    pub fn new(adapter: Arc<TaskAdapter>, cache: Arc<Cache>, mode: Arc<super::mode::Mode>) -> Self {
        let state = Arc::new(Mutex::new(State::default()));
        let mut events = Events::default();
        let s = Arc::clone(&state);
        events.pick = Some(Arc::new(move |key| {
            let mut st = held(&s);
            if let Some(label) = key.strip_prefix("label:") {
                st.label = Some(label.to_string());
            } else if key == "all" {
                st.label = None;
            } else {
                st.selected = (st.selected.as_deref() != Some(key)).then(|| key.to_string());
            }
        }));
        events.tab = Some(Arc::new(move |key| {
            if key == "list" {
                mode.set_list(true);
            }
        }));
        let (a, c) = (Arc::clone(&adapter), Arc::clone(&cache));
        events.act = Some(Arc::new(move |key, to| {
            let Some(entry) = c.current().and_then(|p| p.entry(key).cloned()) else {
                return;
            };
            if a.move_to(key, to, crate::source::row_of(&entry).revision.as_deref())
                .is_ok()
            {
                c.changed();
            }
        }));
        let s = Arc::clone(&state);
        events.close = Some(Arc::new(move || held(&s).selected = None));
        let s = Arc::clone(&state);
        events.create = Some(Arc::new(move |col| {
            held(&s).creating = Some(col.to_string());
        }));
        let (a, c) = (Arc::clone(&adapter), Arc::clone(&cache));
        events.enter = Some(Arc::new(move |col, text| {
            if let Ok(mut draft) = a.blank() {
                for f in &mut draft.fields {
                    match f.key.as_str() {
                        "title" => f.value = text.to_string(),
                        "progress" => f.value = col.to_string(),
                        _ => {}
                    }
                }
                if a.create(&draft).is_ok() {
                    c.changed();
                }
            }
        }));
        Self {
            adapter,
            cache,
            state,
            entry: Entry::new("What needs to be done?"),
            events,
        }
    }

    /// Choose the task whose title holds `part` (for a picture of the open panel).
    pub fn select_title(&self, part: &str) {
        let part = part.to_lowercase();
        let found = self.cache.current().and_then(|s| {
            s.entries
                .iter()
                .find(|e| e.task.display_title().to_lowercase().contains(&part))
                .map(|e| e.id.to_string())
        });
        held(&self.state).selected = found;
    }

    fn projects(snapshot: &Snapshot, shown: Option<&str>) -> Vec<SideItem> {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for e in &snapshot.entries {
            for l in label_list(&e.task) {
                *counts.entry(l).or_default() += 1;
            }
        }
        let mut items = vec![
            SideItem::heading("Projects (labels)"),
            SideItem::new("all", "All tasks", Some(snapshot.entries.len())),
        ];
        items.extend(
            counts
                .into_iter()
                .map(|(l, n)| SideItem::new(&format!("label:{l}"), &l, Some(n))),
        );
        for i in &mut items {
            i.selected = match shown {
                None => i.key == "all",
                Some(l) => i.key == format!("label:{l}"),
            };
        }
        items
    }
}

impl Surface for BoardScreen {
    fn build(&self, width: f32, height: f32) -> Box<dyn Element> {
        let Some(snapshot) = self.cache.current() else {
            let why = self
                .cache
                .problem()
                .unwrap_or_else(|| "Reading the tasks and what blocks them ...".into());
            return Box::new(fs_components_widgets::plan::message(&why, width, height));
        };
        let (selected, creating, label) = {
            let st = held(&self.state);
            (st.selected.clone(), st.creating.clone(), st.label.clone())
        };
        let mut columns: Vec<Column> = PROGRESS
            .iter()
            .map(|p| Column::new(p, progress_title(p)))
            .collect();
        for e in &snapshot.entries {
            if label
                .as_ref()
                .is_some_and(|l| !label_list(&e.task).contains(l))
            {
                continue;
            }
            let id = e.id.to_string();
            let at = PROGRESS
                .iter()
                .position(|p| *p == e.task.effective_progress());
            if let Some(i) = at {
                columns[i]
                    .cards
                    .push(card(&snapshot, e, selected.as_deref() == Some(id.as_str())));
            }
        }
        let panel = selected
            .as_deref()
            .and_then(|k| snapshot.entry(k))
            .map(|e| panel(&snapshot, e));
        let tabs = [
            Tab::new("board", "Board"),
            Tab::new("list", "List"),
            Tab::new("calendar", "Calendar").locked(),
            Tab::new("timeline", "Timeline").locked(),
        ];
        let projects = Self::projects(&snapshot, label.as_deref());
        let view = BoardView {
            tabs: &tabs,
            active: "board",
            soon: "Not built yet",
            projects: &projects,
            columns: &columns,
            panel: panel.as_ref(),
            creating: creating.as_deref(),
            entry: &self.entry,
            create: "Create",
            close: "Close",
            events: &self.events,
        };
        let _ = &self.adapter;
        Box::new(view.build("fs-plan-board", width, height))
    }

    fn styles(&self) -> String {
        fs_components_widgets::styles()
    }

    fn status_line(&self) -> bool {
        false
    }
}
