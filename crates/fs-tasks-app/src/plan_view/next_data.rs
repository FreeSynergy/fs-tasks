//! What the day list shows: the lists of the side, their counts, and the sections of the list chosen.
//!
//! Pattern: Specification + Factory — pure functions of the picture; the six lists are cut by readiness and due date
//! and by nothing else, so they can be tested without a window. The names are those of a quiet to-do list; what each
//! one holds here is said in its note.

use std::collections::BTreeMap;

use fs_components_widgets::plan::{Detail, Section, SideItem, TaskLine, Tone};
use fs_tasks_core::TaskEntry;

use super::cards::due_chip;
use super::look::{Standing, Urgency, people, standing, urgency, why};
use super::snapshot::Snapshot;
use crate::source::label_list;

/// The six lists, in order: key, title, drawing, what it holds.
pub const LISTS: [(&str, &str, &str, &str); 6] = [
    (
        "inbox",
        "Inbox",
        "mail-inbox",
        "Open tasks with no due date, no label and nobody holding them: not sorted yet.",
    ),
    (
        "today",
        "Today",
        "starred",
        "Open tasks that nothing blocks and that are due today or were due before.",
    ),
    (
        "upcoming",
        "Upcoming",
        "x-office-calendar",
        "Open tasks that nothing blocks and that are due later, by day.",
    ),
    (
        "anytime",
        "Anytime",
        "view-list",
        "Open tasks that nothing blocks and that have no due date.",
    ),
    (
        "someday",
        "Someday",
        "folder",
        "Tasks that cannot start yet: they wait for other tasks, a task behind them was stopped or is not here, or they are in a cycle.",
    ),
    (
        "logbook",
        "Logbook",
        "emblem-ok",
        "Finished tasks: completed, cancelled or failed.",
    ),
];

fn is_inbox(e: &TaskEntry, st: Standing) -> bool {
    st != Standing::Finished
        && e.task.due.is_none()
        && label_list(&e.task).is_empty()
        && people(&e.task).is_empty()
}

fn in_list(snapshot: &Snapshot, e: &TaskEntry, list: &str) -> bool {
    let st = standing(snapshot, e);
    let due = urgency(&e.task, snapshot.today);
    match list {
        "inbox" => is_inbox(e, st),
        "today" => st == Standing::Today,
        "upcoming" => st == Standing::Ready && due == Some(Urgency::Later),
        "anytime" => st == Standing::Ready && due.is_none(),
        "someday" => st == Standing::Blocked,
        "logbook" => st == Standing::Finished,
        other => other.strip_prefix("label:").is_some_and(|l| {
            label_list(&e.task).iter().any(|x| x == l) && st != Standing::Finished
        }),
    }
}

/// The lists of the side with their counts, then the projects (labels); `chosen` is the key of the list shown.
#[must_use]
pub fn side(snapshot: &Snapshot, chosen: &str) -> Vec<SideItem> {
    let mut items: Vec<SideItem> = LISTS
        .iter()
        .map(|(key, title, icon, _)| {
            let n = snapshot
                .entries
                .iter()
                .filter(|e| in_list(snapshot, e, key))
                .count();
            SideItem::new(key, title, (*key != "logbook" && n > 0).then_some(n)).icon(icon)
        })
        .collect();
    let mut labels: BTreeMap<String, usize> = BTreeMap::new();
    for e in &snapshot.entries {
        if standing(snapshot, e) != Standing::Finished {
            for l in label_list(&e.task) {
                *labels.entry(l).or_default() += 1;
            }
        }
    }
    items.push(SideItem::heading("Projects"));
    items.extend(
        labels
            .into_iter()
            .map(|(l, n)| SideItem::new(&format!("label:{l}"), &l, Some(n))),
    );
    items.push(SideItem::heading("View"));
    items.push(SideItem::new("view:board", "Board", None).icon("view-grid"));
    for i in &mut items {
        i.selected = i.key == chosen;
    }
    items
}

/// The title and the note of the list shown.
#[must_use]
pub fn heading(chosen: &str) -> (String, String) {
    LISTS.iter().find(|l| l.0 == chosen).map_or_else(
        || {
            (
                chosen.strip_prefix("label:").unwrap_or(chosen).to_string(),
                "Open tasks of this project.".to_string(),
            )
        },
        |l| (l.1.to_string(), l.3.to_string()),
    )
}

fn line(snapshot: &Snapshot, e: &TaskEntry, opened: &[String]) -> TaskLine {
    let key = e.id.to_string();
    let mut l = TaskLine::new(&key, &e.task.display_title());
    let mut sub = label_list(&e.task);
    sub.extend(people(&e.task));
    l.sub = (!sub.is_empty()).then(|| sub.join("  \u{b7}  "));
    l.chip = due_chip(&e.task, snapshot.today);
    l.checked = standing(snapshot, e) == Standing::Finished;
    let reason = snapshot.readiness(&key).and_then(|r| why(snapshot, r));
    if standing(snapshot, e) == Standing::Blocked {
        l.muted = true;
        l.reason = reason.map(|(_, t)| t);
    }
    l.open = opened.contains(&key);
    if l.open {
        let t = &e.task;
        l.details = vec![Detail::new("Key", &format!("FS-{}", snapshot.number(&key)))];
        if let Some(d) = &t.description {
            l.details.push(Detail::new("Notes", d));
        }
        if let Some(r) = &l.reason {
            l.details.push(Detail::new("Waiting", r));
        }
        if let Some(p) = t.priority {
            l.details.push(Detail::new("Priority", &p.to_string()));
        }
    }
    l
}

/// The sections of the list `chosen`: the tasks in it, grouped the way that list reads best.
#[must_use]
pub fn sections(snapshot: &Snapshot, chosen: &str, opened: &[String]) -> Vec<Section> {
    let mine: Vec<&TaskEntry> = snapshot
        .entries
        .iter()
        .filter(|e| in_list(snapshot, e, chosen))
        .collect();
    let mut by_group: Vec<(String, Tone, Vec<TaskLine>)> = Vec::new();
    for e in mine {
        let (name, tone) = group_of(snapshot, e, chosen);
        let l = line(snapshot, e, opened);
        match by_group.iter_mut().find(|g| g.0 == name) {
            Some(g) => g.2.push(l),
            None => by_group.push((name, tone, vec![l])),
        }
    }
    by_group
        .into_iter()
        .map(|(n, t, lines)| Section::new(&n, t, lines))
        .collect()
}

fn group_of(snapshot: &Snapshot, e: &TaskEntry, chosen: &str) -> (String, Tone) {
    let st = standing(snapshot, e);
    match chosen {
        "today" if urgency(&e.task, snapshot.today) == Some(Urgency::Overdue) => {
            ("Overdue".into(), Tone::Danger)
        }
        "today" => ("Today".into(), Tone::Accent),
        "upcoming" => (
            due_chip(&e.task, snapshot.today).map_or_else(String::new, |c| c.text),
            Tone::Neutral,
        ),
        "someday" => match snapshot
            .readiness(&e.id.to_string())
            .and_then(|r| why(snapshot, r))
            .map(|(m, _)| m)
        {
            Some("CYCLE") => ("In a cycle".into(), Tone::Danger),
            Some("BLOCKED: STOPPED") => ("Behind a stopped task".into(), Tone::Warning),
            Some("BLOCKED: UNKNOWN") => ("Behind an unknown task".into(), Tone::Warning),
            _ => ("Waiting for other tasks".into(), Tone::Neutral),
        },
        "logbook" => (
            crate::words::progress_title(e.task.effective_progress()).to_string(),
            Tone::Success,
        ),
        _ if st == Standing::Blocked => ("Waiting".into(), Tone::Warning),
        _ if st == Standing::Today => ("Today".into(), Tone::Accent),
        _ => ("Ready".into(), Tone::Neutral),
    }
}
