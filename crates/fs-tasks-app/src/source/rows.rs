//! A task as a row or a card, and how its values read.
//!
//! Pattern: Adapter — the task in, the workbench's neutral row out. The column is the task's effective
//! progress as the core computes it (RFC 8984 §5.2.5), so nothing is shown that the task does not say.

use fs_components_workbenches::tasks::TaskRow;
use fs_tasks_core::{Task, TaskEntry};

use super::adapter::revision_of;
use super::draft::is_series;
use super::marks::is_marked;
use crate::words::{field, progress_title, say};

/// The due date as the person reads it; seconds only when there are some.
pub(super) fn due_text(task: &Task) -> String {
    let Some(due) = task.due.as_deref() else {
        return String::new();
    };
    let text = due.replacen('T', " ", 1);
    text.strip_suffix(":00").unwrap_or(&text).to_string()
}

/// The labels in a stable order: those switched on.
pub(super) fn label_list(task: &Task) -> Vec<String> {
    task.keywords
        .iter()
        .filter(|(_, on)| **on)
        .map(|(k, _)| k.clone())
        .collect()
}

pub(super) fn priority_text(task: &Task) -> String {
    task.priority.map(|p| p.to_string()).unwrap_or_default()
}

/// Does `term` (in lower case) occur in what the person reads of this task?
pub(crate) fn matches(task: &Task, term: &str) -> bool {
    let has = |text: &str| text.to_lowercase().contains(term);
    term.is_empty()
        || has(&task.display_title())
        || task.description.as_deref().is_some_and(has)
        || task.keywords.keys().any(|k| has(k))
}

/// Is it still to be done?
pub(crate) fn is_open(task: &Task) -> bool {
    matches!(task.effective_progress(), "needs-action" | "in-process")
}

/// The row of a stored task.
pub(crate) fn row_of(entry: &TaskEntry) -> TaskRow {
    let task = &entry.task;
    let (title, column) = (task.display_title(), task.effective_progress());
    let (due, priority) = (due_text(task), priority_text(task));
    let mut row = TaskRow::new(entry.id.to_string(), title.clone(), column);
    if is_marked(entry) {
        row = row.guarded(field::SENSITIVE);
    }
    row.cells = vec![
        title,
        due.clone(),
        priority.clone(),
        progress_title(column).to_string(),
    ];
    if !due.is_empty() {
        row.lines.push(format!("{}: {due}", field::DUE));
    }
    if !priority.is_empty() {
        row.lines.push(format!("{}: {priority}", field::PRIORITY));
    }
    row.tags = label_list(task);
    if is_series(task) {
        row.move_refusal = Some(say::SERIES_MOVE.to_string());
    }
    row.revision = Some(revision_of(entry));
    row
}
