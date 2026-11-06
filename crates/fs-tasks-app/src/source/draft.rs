//! A task as a form.
//!
//! Pattern: Factory — the fields the first version can change, each holding what the task says now, so
//! a save writes only what the person changed. A task that repeats has no due date and no status in its
//! form: those belong to its occurrences.

use fs_components_workbenches::tasks::{DraftField, FieldKind, FieldOption, TaskDraft};
use fs_tasks_core::validate::PROGRESS;
use fs_tasks_core::{Task, TaskEntry};

use super::adapter::revision_of;
use super::marks::is_marked;
use super::rows::{due_text, label_list, priority_text};
use crate::words::{field, progress_title, say};

pub(super) const TITLE: &str = "title";
pub(super) const DESCRIPTION: &str = "description";
pub(super) const DUE: &str = "due";
pub(super) const PRIORITY: &str = "priority";
pub(super) const PROGRESS_FIELD: &str = "progress";
pub(super) const LABELS: &str = "labels";

/// Does this task repeat, or is it one occurrence of a task that does?
pub fn is_series(task: &Task) -> bool {
    task.recurrence_rule.is_some()
        || !task.recurrence_overrides.is_empty()
        || task.recurrence_id.is_some()
}

fn progress_kind() -> FieldKind {
    FieldKind::Choice(
        PROGRESS
            .iter()
            .map(|p| FieldOption::new(*p, progress_title(p)))
            .collect(),
    )
}

fn fields_of(task: &Task) -> Vec<DraftField> {
    let description = task.description.clone().unwrap_or_default();
    let mut fields = vec![
        DraftField::new(TITLE, field::TITLE, task.title.clone().unwrap_or_default()),
        DraftField::new(DESCRIPTION, field::DESCRIPTION, description).kind(FieldKind::LongText),
    ];
    if !is_series(task) {
        fields.push(DraftField::new(DUE, field::DUE, due_text(task)));
    }
    fields.push(DraftField::new(
        PRIORITY,
        field::PRIORITY,
        priority_text(task),
    ));
    if !is_series(task) {
        fields.push(
            DraftField::new(PROGRESS_FIELD, field::STATUS, task.effective_progress())
                .kind(progress_kind()),
        );
    }
    fields.push(DraftField::new(
        LABELS,
        field::LABELS,
        label_list(task).join(", "),
    ));
    fields
}

/// The form of a stored task.
pub(super) fn draft_of(entry: &TaskEntry) -> TaskDraft {
    let task = &entry.task;
    let mut draft = TaskDraft::new(Some(revision_of(entry)), fields_of(task));
    if is_marked(entry) {
        draft.marks.push(field::SENSITIVE.to_string());
    }
    if is_series(task) {
        draft.note = Some(say::SERIES_FORM.to_string());
    }
    draft
}

/// The form of a task that does not exist yet.
pub(super) fn blank_draft() -> TaskDraft {
    TaskDraft::new(None, fields_of(&Task::new("")))
}
