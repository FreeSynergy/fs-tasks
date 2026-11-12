//! A task as the card of the board and as the panel of a chosen card.
//!
//! Pattern: Adapter — the picture's task, its readiness and today in; the widget's plain data out. The words and
//! the tones are decided here (a late due date is danger, today is accent), not in the widget.

use fs_components_widgets::plan::{Chip, Detail, Panel, PlanCard, Tone};
use fs_tasks_core::{Task, TaskEntry};

use super::look::{Standing, Urgency, people, standing, urgency, why};
use super::snapshot::Snapshot;
use crate::day::Day;
use crate::source::label_list as label_names;
use crate::words::progress_title;

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// "Nov 13", the day of a due time.
fn short_day(task: &Task) -> Option<String> {
    let due = task.due.as_deref()?;
    let day = Day::parse(due.get(..10)?).ok()?;
    let (_, month, d) = day.ymd();
    Some(format!(
        "{} {d}",
        MONTHS[usize::try_from(month - 1).ok()?.min(11)]
    ))
}

/// The due date as a chip: late is danger, today is accent, later is plain.
#[must_use]
pub fn due_chip(task: &Task, today: Day) -> Option<Chip> {
    let day = short_day(task)?;
    Some(match urgency(task, today)? {
        Urgency::Overdue => Chip::new(format!("Overdue {day}"), Tone::Danger),
        Urgency::Today => Chip::new("Today", Tone::Accent),
        Urgency::Later => Chip::new(day, Tone::Neutral),
    })
}

/// The card of `entry`.
#[must_use]
pub fn card(snapshot: &Snapshot, entry: &TaskEntry, selected: bool) -> PlanCard {
    let task = &entry.task;
    let key = entry.id.to_string();
    let mut card = PlanCard::new(&key, &task.display_title());
    card.selected = selected;
    card.people = people(task);
    card.due = due_chip(task, snapshot.today);
    card.chips = label_names(task)
        .into_iter()
        .map(|l| Chip::new(l, Tone::Neutral))
        .collect();
    if let Some((mark, reason)) = snapshot.readiness(&key).and_then(|r| why(snapshot, r)) {
        if standing(snapshot, entry) == Standing::Blocked {
            let tone = if mark == "CYCLE" {
                Tone::Danger
            } else {
                Tone::Warning
            };
            card.chips.insert(0, Chip::new(mark, tone));
            card.note = Some(reason);
            card.muted = true;
        }
    }
    card
}

/// The panel of `entry`.
#[must_use]
pub fn panel(snapshot: &Snapshot, entry: &TaskEntry) -> Panel {
    let task = &entry.task;
    let key = entry.id.to_string();
    let mut details = vec![Detail::new(
        "Status",
        progress_title(task.effective_progress()),
    )];
    let mut add = |label: &str, value: String| {
        if !value.is_empty() {
            details.push(Detail::new(label, &value));
        }
    };
    add("Due", super::look::due_phrase(task, snapshot.today));
    add("Assigned", people(task).join(", "));
    add(
        "Priority",
        task.priority.map(|p| p.to_string()).unwrap_or_default(),
    );
    add("Labels", label_names(task).join(", "));
    add(
        "Standing",
        match standing(snapshot, entry) {
            Standing::Today => "Ready, due today or earlier",
            Standing::Ready => "Ready",
            Standing::Blocked => "Blocked",
            Standing::Finished => "Finished",
        }
        .to_string(),
    );
    Panel {
        title: task.display_title(),
        note: snapshot
            .readiness(&key)
            .and_then(|r| why(snapshot, r))
            .map(|(_, t)| t),
        details,
    }
}
