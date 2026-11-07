//! A form written back onto a task.
//!
//! Pattern: Command — only the fields that differ from what the form started with are applied.
//! Everything else on the task stays untouched: properties the form does not show, `extra`,
//! participants, relations, the recurrence rule and its overrides. A field the form does not know, or a
//! change to the due date or the status of a series, is refused rather than guessed at; nothing is
//! changed then. An unchanged status is never written, so a status the core only computes stays that.

use fs_components_workbenches::SourceError;
use fs_components_workbenches::tasks::{DraftField, TaskDraft};
use fs_tasks_core::time::parse_local;
use fs_tasks_core::validate::PROGRESS;
use fs_tasks_core::{Task, now_utc};

use super::draft::{DESCRIPTION, DUE, LABELS, PRIORITY, PROGRESS_FIELD, TITLE, is_series};
use super::labels::read_labels;
use crate::words::say;

/// Put what changed in `draft` onto `task`.
///
/// # Errors
/// [`SourceError`] with a reason for the person; `task` is then as it was.
pub fn apply_draft(task: &mut Task, draft: &TaskDraft) -> Result<(), SourceError> {
    let mut changed = task.clone();
    for field in draft.fields.iter().filter(|f| f.changed()) {
        let text = field.value.trim();
        match field.key.as_str() {
            TITLE if text.is_empty() => return Err(SourceError::failed(say::NEEDS_TITLE)),
            TITLE => changed.title = Some(text.to_string()),
            DESCRIPTION => changed.description = (!text.is_empty()).then(|| field.value.clone()),
            PRIORITY => changed.priority = priority(text)?,
            LABELS => set_labels(&mut changed, field),
            DUE | PROGRESS_FIELD if is_series(task) => {
                return Err(SourceError::failed(say::SERIES_REFUSED));
            }
            DUE => changed.due = due(text)?,
            PROGRESS_FIELD => set_progress(&mut changed, text)?,
            other => {
                return Err(SourceError::failed(format!(
                    "the form has an unknown field \"{other}\""
                )));
            }
        }
    }
    *task = changed;
    Ok(())
}

/// The person chose this status: it is written, with the time it was chosen.
pub(super) fn set_progress(task: &mut Task, progress: &str) -> Result<(), SourceError> {
    if !PROGRESS.contains(&progress) {
        return Err(SourceError::failed(say::NO_COLUMN));
    }
    task.progress = Some(progress.to_string());
    task.progress_updated = Some(now_utc());
    Ok(())
}

fn priority(text: &str) -> Result<Option<i32>, SourceError> {
    if text.is_empty() {
        return Ok(None);
    }
    match text.parse() {
        Ok(number @ 0..=9) => Ok(Some(number)),
        _ => Err(SourceError::failed(say::BAD_PRIORITY)),
    }
}

/// `2026-11-30 17:00`, with seconds if wanted, as a `LocalDateTime`. A date alone is refused: the task
/// has no way yet to say "that day, at no particular time".
fn due(text: &str) -> Result<Option<String>, SourceError> {
    if text.is_empty() {
        return Ok(None);
    }
    let written = text.replacen(' ', "T", 1);
    let full = if written.len() == "2026-11-30T17:00".len() {
        format!("{written}:00")
    } else {
        written
    };
    parse_local(&full).map_err(|_| SourceError::failed(say::BAD_DUE))?;
    Ok(Some(full))
}

/// Only labels the person took out or put in change; a keyword switched off is not shown and stays.
fn set_labels(task: &mut Task, field: &DraftField) {
    let (before, now) = (read_labels(&field.original), read_labels(&field.value));
    for gone in before.difference(&now) {
        task.keywords.remove(gone);
    }
    for new in now.difference(&before) {
        task.label(new);
    }
}
