//! The status board: one column per `progress` value, one card per task; a move changes `progress`.
//!
//! Pattern: Adapter (`BoardSource`, `BoardEvents`) over `TaskItems` for reading and the backend for writing, so the
//! revision check and the rules of `change` are the same as in the Admin view. Which controls are locked comes from the
//! description's support table: a move is the command `change`; add, rename and limit of a column are `columnCreate`,
//! `columnRename`, `columnLimit` (not built: they stay locked and unlock by themselves).

use std::sync::Arc;

use fs_admin_frontend::{Backend, BackendError, Values};
use fs_components_fields::Say;
use fs_components_kanban::{
    Answer, BoardEvents, BoardSource, Card, Column, Due, DueState, Label, Operation, Tone,
};
use fs_tasks_core::describe::support::is_supported;
use fs_tasks_core::validate::PROGRESS;
use fs_tasks_core::{TaskEntry, TaskItems};
use tokio::runtime::Handle;

use super::dates::{day_of, today};
use crate::backend::record_id;

/// The tasks of one provider as a board.
pub struct TaskBoard {
    items: Arc<dyn TaskItems>,
    runtime: Handle,
    backend: Arc<dyn Backend>,
    say: Arc<dyn Say>,
}

impl TaskBoard {
    /// A board over `items`, moving through `backend`.
    pub fn new(
        items: Arc<dyn TaskItems>,
        runtime: Handle,
        backend: Arc<dyn Backend>,
        say: Arc<dyn Say>,
    ) -> Self {
        Self {
            items,
            runtime,
            backend,
            say,
        }
    }

    fn card(entry: &TaskEntry) -> Card {
        let task = &entry.task;
        let mut card = Card::new(&record_id(entry.id), &task.display_title());
        card.labels = task
            .keywords
            .iter()
            .filter(|(_, on)| **on)
            .map(|(k, _)| Label {
                text: k.clone(),
                tone: Tone::Accent,
            })
            .collect();
        card.assignee = task
            .participants
            .values()
            .find_map(|p| p.name.clone().or_else(|| p.email.clone()));
        if let (Some(text), Some(day)) = (task.due.as_deref(), day_of(task.due.as_deref())) {
            let state = match task.effective_progress() {
                "completed" => DueState::Done,
                _ if day < today() => DueState::Overdue,
                _ if day.0 - today().0 <= 2 => DueState::Soon,
                _ => DueState::Open,
            };
            card.due = Some(Due {
                text: text.chars().take(10).collect(),
                state,
            });
        }
        card
    }
}

impl BoardSource for TaskBoard {
    fn columns(&self) -> Vec<Column> {
        let entries = self.runtime.block_on(self.items.list()).unwrap_or_default();
        PROGRESS
            .iter()
            .map(|p| {
                let mut column = Column::new(p, &self.say.say(&format!("board.column.{p}")));
                column.cards = entries
                    .iter()
                    .filter(|e| e.task.effective_progress() == *p)
                    .map(Self::card)
                    .collect();
                column
            })
            .collect()
    }

    fn supports(&self, op: Operation) -> bool {
        is_supported(match op {
            Operation::Move => "change",
            Operation::AddColumn => "columnCreate",
            Operation::Rename => "columnRename",
            Operation::Limit => "columnLimit",
        })
    }
}

impl BoardEvents for TaskBoard {
    fn move_card(&self, key: &str, from: &str, to: &str) -> Answer {
        if from == to {
            return Answer::Accepted;
        }
        let input = Values::from([
            ("id".to_owned(), vec![key.to_owned()]),
            ("progress".to_owned(), vec![to.to_owned()]),
        ]);
        match self.backend.run("change", &input) {
            Ok(_) => Answer::Accepted,
            Err(BackendError::Rejected(_)) => Answer::Refused("rejected".into()),
            Err(BackendError::NotFound) => Answer::Refused("gone".into()),
            Err(BackendError::Unavailable(m)) if m.starts_with("changed") => {
                Answer::Refused("changed".into())
            }
            Err(BackendError::Unavailable(m)) if m.starts_with("unsupported") => {
                Answer::Refused("unsupported".into())
            }
            Err(BackendError::Unavailable(_)) => Answer::Refused("unavailable".into()),
        }
    }
}
