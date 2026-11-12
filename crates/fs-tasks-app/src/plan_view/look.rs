//! How a task stands, and the words for it: which group it belongs to, why it waits, when it is due.
//!
//! Pattern: Specification — small pure functions of a task, its readiness and today; both views use the same
//! rules, so a task is "blocked" in the same words on a card and in a list.

use std::fmt::Write;

use fs_tasks_core::plan::Readiness;
use fs_tasks_core::{Task, TaskEntry};

use super::snapshot::Snapshot;
use crate::day::Day;
use crate::source::is_open;

/// Where a task stands for the person who asks "what next?".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    /// Open, nothing keeps it from starting, and it is due today or was due before.
    Today,
    /// Open and nothing keeps it from starting.
    Ready,
    /// Open and something keeps it from starting (waiting, a stopped task behind it, an unknown task, a cycle).
    Blocked,
    /// Completed, cancelled or failed.
    Finished,
}

/// How a due date stands against today.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Urgency {
    /// Before today.
    Overdue,
    /// Today.
    Today,
    /// Later.
    Later,
}

/// The date part of the due text, `YYYY-MM-DD`.
fn due_day(task: &Task) -> Option<&str> {
    task.due.as_deref().and_then(|d| d.get(..10))
}

/// How the due date stands, if there is one.
#[must_use]
pub fn urgency(task: &Task, today: Day) -> Option<Urgency> {
    let day = due_day(task)?;
    let today = today.text();
    Some(match day.cmp(today.as_str()) {
        std::cmp::Ordering::Less => Urgency::Overdue,
        std::cmp::Ordering::Equal => Urgency::Today,
        std::cmp::Ordering::Greater => Urgency::Later,
    })
}

/// The group of `entry`.
#[must_use]
pub fn standing(snapshot: &Snapshot, entry: &TaskEntry) -> Standing {
    if !is_open(&entry.task) {
        return Standing::Finished;
    }
    match snapshot.readiness(&entry.id.to_string()) {
        None | Some(Readiness::Ready) => {
            if matches!(
                urgency(&entry.task, snapshot.today),
                Some(Urgency::Overdue | Urgency::Today)
            ) {
                Standing::Today
            } else {
                Standing::Ready
            }
        }
        Some(Readiness::Done) => Standing::Finished,
        Some(_) => Standing::Blocked,
    }
}

/// At most three names, then "and N more".
fn names(snapshot: &Snapshot, uids: &[String]) -> String {
    let shown: Vec<&str> = uids.iter().take(3).map(|u| snapshot.name_of(u)).collect();
    let mut text = shown.join(", ");
    if uids.len() > 3 {
        let _ = write!(text, " and {} more", uids.len() - 3);
    }
    text
}

/// The short marking and the sentence that say why a task cannot start; `None` if nothing keeps it.
#[must_use]
pub fn why(snapshot: &Snapshot, readiness: &Readiness) -> Option<(&'static str, String)> {
    match readiness {
        Readiness::Ready | Readiness::Done => None,
        Readiness::Blocked(uids) => {
            Some(("BLOCKED", format!("waits for {}", names(snapshot, uids))))
        }
        Readiness::DependencyCancelled(uids) => Some((
            "BLOCKED: STOPPED",
            format!("waits for a stopped task: {}", names(snapshot, uids)),
        )),
        Readiness::Unknown(uids) => Some((
            "BLOCKED: UNKNOWN",
            format!(
                "waits for a task that is not here: {}",
                names(snapshot, uids)
            ),
        )),
        Readiness::Conflict(uids) => Some((
            "CYCLE",
            format!("is in a cycle with {}", names(snapshot, uids)),
        )),
    }
}

/// The due date as a phrase: "today 17:00", "overdue, 2026-11-10", "2026-11-20 17:00"; empty if none.
#[must_use]
pub fn due_phrase(task: &Task, today: Day) -> String {
    let Some(due) = task.due.as_deref() else {
        return String::new();
    };
    let time = due.get(11..16).unwrap_or("");
    match urgency(task, today) {
        Some(Urgency::Today) => format!("today {time}").trim_end().to_string(),
        Some(Urgency::Overdue) => format!("overdue, {}", &due[..10]),
        _ => format!("{} {time}", &due[..10]).trim_end().to_string(),
    }
}

/// Who holds the task: the names of its participants, in order.
#[must_use]
pub fn people(task: &Task) -> Vec<String> {
    task.participants
        .values()
        .filter_map(|p| p.name.clone().filter(|n| !n.is_empty()))
        .collect()
}
