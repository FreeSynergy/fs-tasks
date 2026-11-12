//! Edges between tasks and the cycles in them.
//!
//! Pattern: Value object (`Edge`) + pure functions. **Direction (RFC 9253 section 4 and 5):** a temporal relation
//! (`finishtostart` ...) is held by the predecessor and refers to the successor; `depends-on` is held by the dependent
//! task and refers to the one it depends on. An [`Edge`] is always `from` predecessor `to` successor, whichever way it
//! was written, so the rest of the code does not have to remember the difference.

use std::collections::{BTreeMap, BTreeSet};

use chrono::Duration;

use crate::model::{Relation, Task};

/// How the successor is tied to its predecessor (the four temporal relations of RFC 9253; `depends-on` is a
/// finish-to-start edge: the dependent one waits until the other is finished).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EdgeKind {
    /// The successor cannot start until the predecessor finishes.
    FinishToStart,
    /// The successor cannot finish until the predecessor finishes.
    FinishToFinish,
    /// The successor cannot start until the predecessor starts.
    StartToStart,
    /// The successor cannot finish until the predecessor starts.
    StartToFinish,
}

impl EdgeKind {
    /// Does the edge keep the successor from STARTING (and so make it "blocked")?
    pub const fn blocks_start(self) -> bool {
        matches!(self, Self::FinishToStart | Self::StartToStart)
    }
}

/// `from` (predecessor) to `to` (successor), by `uid`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Edge {
    /// The predecessor.
    pub from: String,
    /// The successor.
    pub to: String,
    /// The tie.
    pub kind: EdgeKind,
    /// Lag (positive) or lead (negative) between the predecessor and the successor (RFC 9253 `GAP`); zero for none.
    pub gap: Duration,
}

impl Edge {
    /// The edge a relation held by `holder` towards `other` means, with `gap`; `None` for the relations that are no
    /// dependency (`first`, `next`, `child`, `parent`).
    pub fn of_relation(
        holder: &str,
        other: &str,
        relation: Relation,
        gap: Duration,
    ) -> Option<Self> {
        let (from, to, kind) = match relation {
            Relation::DependsOn => (other, holder, EdgeKind::FinishToStart),
            Relation::FinishToStart => (holder, other, EdgeKind::FinishToStart),
            Relation::FinishToFinish => (holder, other, EdgeKind::FinishToFinish),
            Relation::StartToStart => (holder, other, EdgeKind::StartToStart),
            Relation::StartToFinish => (holder, other, EdgeKind::StartToFinish),
            Relation::First | Relation::Next | Relation::Child | Relation::Parent => return None,
        };
        Some(Self {
            from: from.to_string(),
            to: to.to_string(),
            kind,
            gap,
        })
    }
}

/// All dependency edges of `tasks` (each task's own relations; an edge named twice is kept once), in order.
pub fn edges_of(tasks: &[Task]) -> Vec<Edge> {
    let all: BTreeSet<Edge> = tasks
        .iter()
        .flat_map(|t| {
            t.relations().into_iter().filter_map(|(other, r)| {
                Edge::of_relation(&t.uid, &other, r, t.relation_gap(&other))
            })
        })
        .collect();
    all.into_iter().collect()
}

fn successors(edges: &[Edge]) -> BTreeMap<&str, Vec<&str>> {
    let mut map: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for e in edges {
        map.entry(e.from.as_str()).or_default().push(e.to.as_str());
        map.entry(e.to.as_str()).or_default();
    }
    map
}

/// Every cycle, as the sorted `uid`s of its strongly connected group: a group of two or more tasks, or one task that
/// depends on itself. Groups are in the order of their first `uid`. Iterative (Tarjan), so a long chain cannot overflow
/// the stack.
pub fn cycles(edges: &[Edge]) -> Vec<Vec<String>> {
    let next = successors(edges);
    let mut index: BTreeMap<&str, usize> = BTreeMap::new();
    let mut low: BTreeMap<&str, usize> = BTreeMap::new();
    let mut on_stack: BTreeSet<&str> = BTreeSet::new();
    let mut stack: Vec<&str> = Vec::new();
    let mut found: Vec<Vec<String>> = Vec::new();
    let mut counter = 0;
    for &root in next.keys() {
        if index.contains_key(root) {
            continue;
        }
        // (node, position of the next successor to look at)
        let mut work: Vec<(&str, usize)> = vec![(root, 0)];
        while let Some(&(node, at)) = work.last() {
            if at == 0 {
                index.insert(node, counter);
                low.insert(node, counter);
                counter += 1;
                stack.push(node);
                on_stack.insert(node);
            }
            let outs = &next[node];
            if let Some(&succ) = outs.get(at) {
                if let Some(top) = work.last_mut() {
                    top.1 += 1;
                }
                if !index.contains_key(succ) {
                    work.push((succ, 0));
                } else if on_stack.contains(succ) {
                    let lowered = low[node].min(index[succ]);
                    low.insert(node, lowered);
                }
                continue;
            }
            work.pop();
            if let Some(&(parent, _)) = work.last() {
                let lowered = low[parent].min(low[node]);
                low.insert(parent, lowered);
            }
            if low[node] == index[node] {
                let mut group = Vec::new();
                while let Some(member) = stack.pop() {
                    on_stack.remove(member);
                    group.push(member.to_string());
                    if member == node {
                        break;
                    }
                }
                let selfish = group.len() == 1 && outs.contains(&node);
                if group.len() > 1 || selfish {
                    group.sort();
                    found.push(group);
                }
            }
        }
    }
    found.sort();
    found
}

/// The path that `new` would close into a cycle with `edges`, `None` if it would not: the `uid`s from the successor of
/// `new` along existing edges back to its predecessor (a new edge `a` to `b` closes a cycle when `a` can already be
/// reached from `b`). For `relate` to refuse an edge and to name the way round.
pub fn path_closing_cycle(edges: &[Edge], new: &Edge) -> Option<Vec<String>> {
    if new.from == new.to {
        return Some(vec![new.from.clone()]);
    }
    let next = successors(edges);
    let mut came: BTreeMap<&str, &str> = BTreeMap::new();
    let mut work = vec![new.to.as_str()];
    let mut seen: BTreeSet<&str> = BTreeSet::from([new.to.as_str()]);
    while let Some(node) = work.pop() {
        if node == new.from {
            let mut path = vec![node.to_string()];
            let mut at = node;
            while let Some(&before) = came.get(at) {
                path.push(before.to_string());
                at = before;
            }
            path.reverse();
            return Some(path);
        }
        for &succ in next.get(node).into_iter().flatten() {
            if seen.insert(succ) {
                came.insert(succ, node);
                work.push(succ);
            }
        }
    }
    None
}
