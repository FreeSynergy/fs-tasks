//! Variant B, "what next": the open tasks in the groups Today, Ready and Blocked, one line each.
//!
//! Pattern: Strategy (`Layout`) — the pattern of a plain list under headings with a count (Things 3: Today,
//! This Evening; a title line, a small line under it, a mark on the right) in this program's own words. A
//! heading is a row that opens nothing; the groups are cut by the core's readiness and today's date.

use fs_components_workbenches::tasks::{
    DraftField, RowQuery, TaskCapabilities, TaskColumn, TaskDraft, TaskField, TaskRow, TaskSection,
    TaskSheet,
};

use super::layout::Layout;
use super::look::{Standing, due_phrase, people, standing, why};
use super::snapshot::Snapshot;
use crate::source::{matches, row_of};

/// The list.
pub struct Next;

const GROUPS: [(Standing, &str, &str, &str); 3] = [
    (
        Standing::Today,
        "group:today",
        "TODAY",
        "Open tasks that nothing blocks and that are due today or were due before.",
    ),
    (
        Standing::Ready,
        "group:ready",
        "READY",
        "Open tasks that nothing blocks and that are due later or have no due date.",
    ),
    (
        Standing::Blocked,
        "group:blocked",
        "BLOCKED",
        "Open tasks that cannot start yet: they wait for other tasks, a task behind them was stopped or is not here, or they are in a cycle.",
    ),
];

/// One line of the list: a heading with the size of its group, or a task (an index into the picture).
enum Item {
    Heading(usize, usize),
    Task(usize),
}

fn items(snapshot: &Snapshot, term: &str) -> Vec<Item> {
    let mut kept: [Vec<usize>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for (i, entry) in snapshot.entries.iter().enumerate() {
        if !matches(&entry.task, term) {
            continue;
        }
        match standing(snapshot, entry) {
            Standing::Today => kept[0].push(i),
            Standing::Ready => kept[1].push(i),
            Standing::Blocked => kept[2].push(i),
            Standing::Finished => {}
        }
    }
    let mut out = Vec::new();
    for (group, tasks) in kept.into_iter().enumerate() {
        if tasks.is_empty() {
            continue;
        }
        out.push(Item::Heading(group, tasks.len()));
        out.extend(tasks.into_iter().map(Item::Task));
    }
    out
}

fn heading(group: usize, size: usize) -> TaskRow {
    let (_, key, name, _) = GROUPS[group];
    let title = format!("{name}  ({size})");
    let mut row = TaskRow::new(key, title.clone(), "group");
    row.cells = vec![title, String::new(), String::new()];
    row.only_when_chosen = true;
    row
}

fn line(snapshot: &Snapshot, index: usize) -> TaskRow {
    let entry = &snapshot.entries[index];
    let mut row = row_of(entry);
    let key = entry.id.to_string();
    let reason = snapshot.readiness(&key).and_then(|r| why(snapshot, r));
    let when = match &reason {
        Some((mark, text)) if standing(snapshot, entry) == Standing::Blocked => {
            row.marks.push((*mark).to_string());
            text.clone()
        }
        _ => due_phrase(&entry.task, snapshot.today),
    };
    row.cells = vec![row.title.clone(), when, people(&entry.task).join(", ")];
    row
}

impl Layout for Next {
    fn columns(&self) -> Vec<TaskColumn> {
        super::board::Board.columns()
    }

    fn rows(
        &self,
        snapshot: &Snapshot,
        query: &RowQuery,
        offset: usize,
        limit: usize,
    ) -> Vec<TaskRow> {
        items(snapshot, &query.term.to_lowercase())
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|item| match item {
                Item::Heading(group, size) => heading(group, size),
                Item::Task(index) => line(snapshot, index),
            })
            .collect()
    }

    fn own_sheet(&self, key: &str, snapshot: &Snapshot) -> Option<TaskSheet> {
        let (standing_of, _, name, rule) = GROUPS.iter().find(|g| g.1 == key)?;
        let size = snapshot
            .entries
            .iter()
            .filter(|e| standing(snapshot, e) == *standing_of)
            .count();
        let fields = vec![
            TaskField::new("Rule", *rule),
            TaskField::new("Tasks", size.to_string()),
        ];
        Some(TaskSheet::new(
            *name,
            vec![TaskSection::new("This group", fields)],
        ))
    }

    /// The short form: the title alone.
    fn blank(&self) -> Option<TaskDraft> {
        Some(TaskDraft::new(
            None,
            vec![DraftField::new("title", "New task", "")],
        ))
    }

    fn capabilities(&self, mut can: TaskCapabilities) -> TaskCapabilities {
        can.move_tasks = false;
        can
    }
}
