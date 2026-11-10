//! The timeline: a bar from `start` to `due`, a milestone where only one of them is set, an arrow for `depends-on`.
//!
//! Pattern: Adapter (`TimelineSource`) over `TaskItems`. Read only: the timeline has no operation of its own yet.

use std::collections::BTreeMap;
use std::sync::Arc;

use fs_components_timeline::{Dependency, Item, TimelineSource, Tone};
use fs_tasks_core::{Relation, TaskEntry, TaskItems};
use tokio::runtime::Handle;

use super::dates::day_of;
use crate::backend::record_id;

/// The tasks of one provider as a timeline.
pub struct TaskTimeline {
    items: Arc<dyn TaskItems>,
    runtime: Handle,
}

impl TaskTimeline {
    /// A timeline over `items`.
    pub fn new(items: Arc<dyn TaskItems>, runtime: Handle) -> Self {
        Self { items, runtime }
    }

    fn entries(&self) -> Vec<TaskEntry> {
        self.runtime.block_on(self.items.list()).unwrap_or_default()
    }

    fn item(entry: &TaskEntry) -> Option<Item> {
        let t = &entry.task;
        let (key, title) = (record_id(entry.id), t.display_title());
        let item = match (day_of(t.start.as_deref()), day_of(t.due.as_deref())) {
            (Some(s), Some(d)) => {
                let done = match t.effective_progress() {
                    "completed" => 100,
                    _ => t.percent_complete.unwrap_or(0),
                };
                Item::bar(&key, &title, s, d, done)
            }
            (Some(day), None) | (None, Some(day)) => Item::milestone(&key, &title, day),
            (None, None) => return None,
        };
        Some(item.tone(match t.effective_progress() {
            "completed" => Tone::Success,
            "failed" => Tone::Error,
            "cancelled" => Tone::Neutral,
            _ => Tone::Accent,
        }))
    }
}

impl TimelineSource for TaskTimeline {
    fn items(&self) -> Vec<Item> {
        self.entries().iter().filter_map(Self::item).collect()
    }

    fn dependencies(&self) -> Vec<Dependency> {
        let entries = self.entries();
        let by_uid: BTreeMap<&str, String> = entries
            .iter()
            .map(|e| (e.task.uid.as_str(), record_id(e.id)))
            .collect();
        entries
            .iter()
            .flat_map(|e| {
                let to = record_id(e.id);
                let by_uid = &by_uid;
                e.task
                    .relations()
                    .into_iter()
                    .filter_map(move |(uid, rel)| {
                        let first = by_uid.get(uid.as_str())?;
                        matches!(rel, Relation::DependsOn | Relation::FinishToStart).then(|| {
                            Dependency {
                                from: first.clone(),
                                to: to.clone(),
                            }
                        })
                    })
            })
            .collect()
    }
}
