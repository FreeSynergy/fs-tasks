//! Variant A, the board: one column per status, a card per task that says when, who, and what keeps it from starting.
//!
//! Pattern: Strategy (`Layout`) — the pattern of a board of columns (a title, a count of the work, cards with a
//! due date, the people and badges) in this program's own words. A card moves by the status view's own ways; that
//! is an edit of the task for everyone, and the window says so.

use fs_components_workbenches::tasks::{RowQuery, TaskColumn, TaskRow};
use fs_tasks_core::TaskEntry;
use fs_tasks_core::validate::PROGRESS;

use super::layout::Layout;
use super::look::{Standing, Urgency, due_phrase, people, standing, urgency, why};
use super::snapshot::Snapshot;
use crate::source::{is_open, matches, row_of};
use crate::words::progress_title;

/// The board.
pub struct Board;

/// The card of one task: the row of the list, with what a card says about readiness and people.
fn card(snapshot: &Snapshot, entry: &TaskEntry) -> TaskRow {
    let task = &entry.task;
    let mut row = row_of(entry);
    row.lines.clear();
    let key = entry.id.to_string();
    let open = is_open(task);
    if open {
        match snapshot.readiness(&key).and_then(|r| why(snapshot, r)) {
            Some((mark, reason)) => {
                row.marks.push(mark.to_string());
                row.lines.push(reason);
            }
            None if standing(snapshot, entry) != Standing::Finished => {
                row.tags.insert(0, "ready".into());
            }
            None => {}
        }
        if urgency(task, snapshot.today) == Some(Urgency::Overdue) {
            row.marks.push("OVERDUE".into());
        }
    }
    let due = due_phrase(task, snapshot.today);
    if !due.is_empty() {
        row.lines.push(format!("Due: {due}"));
    }
    let who = people(task);
    if !who.is_empty() {
        row.lines.push(format!("Assigned: {}", who.join(", ")));
    }
    if let Some(p) = task.priority {
        row.lines.push(format!("Priority: {p}"));
    }
    row
}

impl Layout for Board {
    fn columns(&self) -> Vec<TaskColumn> {
        PROGRESS
            .iter()
            .map(|p| TaskColumn::new(*p, progress_title(p)))
            .collect()
    }

    fn rows(
        &self,
        snapshot: &Snapshot,
        query: &RowQuery,
        offset: usize,
        limit: usize,
    ) -> Vec<TaskRow> {
        let term = query.term.to_lowercase();
        snapshot
            .entries
            .iter()
            .filter(|e| matches(&e.task, &term))
            .filter(|e| !query.open_only || is_open(&e.task))
            .skip(offset)
            .take(limit)
            .map(|e| card(snapshot, e))
            .collect()
    }
}
