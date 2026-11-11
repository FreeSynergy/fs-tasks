//! A schedule from durations and edges: earliest and latest times and the slack.
//!
//! Pattern: Template method (a forward and a backward pass over the topological order). Pure; the times are computed,
//! never stored. Calendar time only: working days and holidays are not known here (an availability source is a
//! later, separate step).

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};

use super::graph::{Edge, EdgeKind, cycles};

/// One task as the schedule needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// The task's `uid`.
    pub uid: String,
    /// How long it takes (zero for a milestone).
    pub duration: Duration,
    /// A start the task is fixed to (its own `start`), if it has one.
    pub fixed_start: Option<DateTime<Utc>>,
}

/// The planned times of a task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planned {
    /// The task's `uid`.
    pub uid: String,
    /// Earliest start.
    pub start: DateTime<Utc>,
    /// Earliest end (`start` + duration).
    pub end: DateTime<Utc>,
    /// How much the task may be late without moving the end of the plan (zero on the critical path).
    pub slack: Duration,
}

/// Why no schedule was made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScheduleError {
    /// These tasks (sorted `uid`s of the first group) are in a dependency cycle.
    Cycle(Vec<String>),
}

/// Plan `items` with `edges`. A task with no fixed start and no placed predecessor is not placed (it is left out of
/// the result, not guessed). Edges that name a task not in `items` are ignored.
///
/// # Errors
/// [`ScheduleError::Cycle`] when the edges contain a cycle.
pub fn schedule(items: &[Item], edges: &[Edge]) -> Result<Vec<Planned>, ScheduleError> {
    if let Some(group) = cycles(edges).into_iter().next() {
        return Err(ScheduleError::Cycle(group));
    }
    let by_uid: BTreeMap<&str, &Item> = items.iter().map(|i| (i.uid.as_str(), i)).collect();
    let edges: Vec<&Edge> = edges
        .iter()
        .filter(|e| by_uid.contains_key(e.from.as_str()) && by_uid.contains_key(e.to.as_str()))
        .collect();
    let order = topological(items, &edges);

    // forward: earliest start
    let mut early: BTreeMap<&str, DateTime<Utc>> = BTreeMap::new();
    for item in &order {
        let mut start = item.fixed_start;
        for e in edges.iter().filter(|e| e.to == item.uid) {
            let (Some(&p_start), Some(p)) =
                (early.get(e.from.as_str()), by_uid.get(e.from.as_str()))
            else {
                continue;
            };
            let p_end = p_start + p.duration;
            let bound = match e.kind {
                EdgeKind::FinishToStart => p_end,
                EdgeKind::StartToStart => p_start,
                EdgeKind::FinishToFinish => p_end - item.duration,
                EdgeKind::StartToFinish => p_start - item.duration,
            };
            start = Some(start.map_or(bound, |s| s.max(bound)));
        }
        if let Some(s) = start {
            early.insert(item.uid.as_str(), s);
        }
    }
    let Some(plan_end) = order
        .iter()
        .filter_map(|i| early.get(i.uid.as_str()).map(|s| *s + i.duration))
        .max()
    else {
        return Ok(Vec::new());
    };

    // backward: latest start that keeps every successor and the end of the plan
    let mut late: BTreeMap<&str, DateTime<Utc>> = BTreeMap::new();
    for item in order.iter().rev() {
        if !early.contains_key(item.uid.as_str()) {
            continue;
        }
        let mut latest = plan_end - item.duration;
        for e in edges.iter().filter(|e| e.from == item.uid) {
            let (Some(&s_late), Some(s)) = (late.get(e.to.as_str()), by_uid.get(e.to.as_str()))
            else {
                continue;
            };
            let bound = match e.kind {
                EdgeKind::FinishToStart => s_late - item.duration,
                EdgeKind::StartToStart => s_late,
                EdgeKind::FinishToFinish => s_late + s.duration - item.duration,
                EdgeKind::StartToFinish => s_late + s.duration,
            };
            latest = latest.min(bound);
        }
        late.insert(item.uid.as_str(), latest);
    }
    Ok(order
        .iter()
        .filter_map(|i| {
            let start = *early.get(i.uid.as_str())?;
            let slack = *late.get(i.uid.as_str())? - start;
            Some(Planned {
                uid: i.uid.clone(),
                start,
                end: start + i.duration,
                slack: slack.max(Duration::zero()),
            })
        })
        .collect())
}

/// The items in an order where every predecessor comes first (Kahn; ties by `uid`, so the result is the same each time).
fn topological<'a>(items: &'a [Item], edges: &[&Edge]) -> Vec<&'a Item> {
    let mut waiting: BTreeMap<&str, usize> = items.iter().map(|i| (i.uid.as_str(), 0)).collect();
    for e in edges {
        if let Some(n) = waiting.get_mut(e.to.as_str()) {
            *n += 1;
        }
    }
    let by_uid: BTreeMap<&str, &Item> = items.iter().map(|i| (i.uid.as_str(), i)).collect();
    let mut ready: Vec<&str> = waiting
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(u, _)| *u)
        .collect();
    ready.sort_by(|a, b| b.cmp(a));
    let mut out = Vec::with_capacity(items.len());
    while let Some(uid) = ready.pop() {
        out.push(by_uid[uid]);
        for e in edges.iter().filter(|e| e.from == uid) {
            if let Some(n) = waiting.get_mut(e.to.as_str()) {
                *n -= 1;
                if *n == 0 {
                    ready.push(e.to.as_str());
                    ready.sort_by(|a, b| b.cmp(a));
                }
            }
        }
    }
    out
}
