//! A task to be read.
//!
//! Pattern: Adapter — a sheet of labelled values. What the first version cannot change is shown here
//! too, so the person sees that it is there; it stays on the task.

use fs_components_workbenches::tasks::{TaskField, TaskSection, TaskSheet};
use fs_tasks_client::Revised;

use super::adapter::revision_text;
use super::draft::is_series;
use super::marks::is_marked;
use super::rows::{due_text, label_list, priority_text};
use crate::words::{field, progress_title, say};

/// The sheet of a stored task.
pub(super) fn sheet_of(read: &Revised) -> TaskSheet {
    let entry = &read.entry;
    let task = &entry.task;
    let mut fields = vec![TaskField::new(
        field::STATUS,
        progress_title(task.effective_progress()),
    )];
    let mut add = |label: &str, value: String| {
        if !value.is_empty() {
            fields.push(TaskField::new(label, value));
        }
    };
    add(field::DUE, due_text(task));
    add(field::ZONE, task.time_zone.clone().unwrap_or_default());
    add(field::PRIORITY, priority_text(task));
    add(
        field::PERCENT,
        task.percent_complete
            .map(|p| p.to_string())
            .unwrap_or_default(),
    );
    add(
        field::EXTENDED,
        task.task_status().unwrap_or_default().to_string(),
    );
    add(field::LABELS_SHORT, label_list(task).join(", "));
    add(
        field::DESCRIPTION,
        task.description.clone().unwrap_or_default(),
    );
    if is_series(task) {
        add(field::REPEATS, field::YES.to_string());
    }
    if !task.participants.is_empty() {
        add(field::PEOPLE, task.participants.len().to_string());
    }
    if !task.related_to.is_empty() {
        add(field::RELATED, task.related_to.len().to_string());
    }
    let mut sheet = TaskSheet::new(
        task.display_title(),
        vec![TaskSection::new(field::DETAILS, fields)],
    );
    if is_marked(entry) {
        sheet.marks.push(field::SENSITIVE.to_string());
    }
    if is_series(task) {
        sheet.note = Some(say::SERIES_SHEET.to_string());
        sheet.move_refusal = Some(say::SERIES_MOVE.to_string());
    } else if task.progress.is_none() {
        sheet.note = Some(say::COMPUTED.to_string());
    }
    sheet.column = task.effective_progress().to_string();
    sheet.revision = Some(revision_text(read));
    sheet
}
