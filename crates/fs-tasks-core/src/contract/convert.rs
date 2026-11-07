//! Wire types <-> the Rust interface types of `TaskItems`.
//!
//! Pattern: Adapter — small pure functions between generated Cap'n Proto structs and
//! `TaskQuery` / `Relation`; the RPC server (`fs-tasks-server`) calls these.

use capnp::Result;

use crate::common_capnp;
use crate::interface::TaskQuery;
use crate::model::Relation;
use crate::tasks_capnp::{Relation as Wire, task_filter};

/// Map to the wire enum.
pub fn relation_to_wire(r: Relation) -> Wire {
    match r {
        Relation::First => Wire::First,
        Relation::Next => Wire::Next,
        Relation::Child => Wire::Child,
        Relation::Parent => Wire::Parent,
        Relation::DependsOn => Wire::DependsOn,
        Relation::FinishToStart => Wire::FinishToStart,
        Relation::FinishToFinish => Wire::FinishToFinish,
        Relation::StartToStart => Wire::StartToStart,
        Relation::StartToFinish => Wire::StartToFinish,
    }
}

/// Map from the wire enum.
pub fn relation_from_wire(w: Wire) -> Relation {
    match w {
        Wire::First => Relation::First,
        Wire::Next => Relation::Next,
        Wire::Child => Relation::Child,
        Wire::Parent => Relation::Parent,
        Wire::DependsOn => Relation::DependsOn,
        Wire::FinishToStart => Relation::FinishToStart,
        Wire::FinishToFinish => Relation::FinishToFinish,
        Wire::StartToStart => Relation::StartToStart,
        Wire::StartToFinish => Relation::StartToFinish,
    }
}

/// Write a [`TaskQuery`] as a `TaskFilter` (unset filters stay unset).
pub fn write_filter(q: &TaskQuery, mut out: task_filter::Builder<'_>) {
    if let Some(t) = q.due_from {
        out.reborrow()
            .init_due_from()
            .set_unix_millis(t.timestamp_millis());
    }
    if let Some(t) = q.due_to {
        out.reborrow()
            .init_due_to()
            .set_unix_millis(t.timestamp_millis());
    }
    out.reborrow()
        .set_progress(q.progress.as_deref().unwrap_or(""));
    out.reborrow().set_label(q.label.as_deref().unwrap_or(""));
    out.reborrow().set_term(q.term.as_deref().unwrap_or(""));
    out.set_open(q.open);
}

/// Read a `TaskFilter` into a [`TaskQuery`] (empty text = unset).
pub fn read_filter(f: task_filter::Reader<'_>) -> Result<TaskQuery> {
    let millis = |has: bool, get: Result<common_capnp::timestamp::Reader<'_>>| -> Result<_> {
        if !has {
            return Ok(None);
        }
        let millis = get?.get_unix_millis();
        chrono::DateTime::from_timestamp_millis(millis)
            .map(Some)
            .ok_or_else(|| capnp::Error::failed(format!("timestamp {millis} out of range")))
    };
    let text = |s: capnp::text::Reader<'_>| -> Result<Option<String>> {
        let s = s.to_str()?;
        Ok((!s.is_empty()).then(|| s.to_string()))
    };
    Ok(TaskQuery {
        due_from: millis(f.has_due_from(), f.get_due_from())?,
        due_to: millis(f.has_due_to(), f.get_due_to())?,
        progress: text(f.get_progress()?)?,
        label: text(f.get_label()?)?,
        term: text(f.get_term()?)?,
        open: f.get_open(),
    })
}
