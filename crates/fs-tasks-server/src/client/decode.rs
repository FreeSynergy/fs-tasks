//! Wire answers to core values, and wire errors to core errors (the client side of `wire.rs`).
//!
//! Pattern: Adapter — the JSCalendar text stays authoritative; the envelope is read for the id and the routing data.

use fs_interfaces::rpc::{read_envelope, read_object_id};
use fs_interfaces::tasks_capnp::task;
use fs_interfaces::Unsupported;
use fs_objects::ObjectId;
use fs_tasks_core::{Error, ImportReport, Result, Task, TaskEntry};

/// A refusal of the server as a core error: "not implemented" is `Unsupported`, a conflict stays one, the rest is
/// `Rejected` with the server's text.
pub(super) fn wire(e: &capnp::Error) -> Error {
    let text = e.to_string();
    if e.kind == capnp::ErrorKind::Unimplemented {
        Unsupported::new("tasks", &text).into()
    } else if text.to_lowercase().contains("conflict") {
        Error::Conflict(text)
    } else {
        Error::Rejected(text)
    }
}

/// One task of an answer.
pub(super) fn entry(t: task::Reader<'_>) -> Result<TaskEntry> {
    let envelope = read_envelope(t.get_envelope().map_err(|e| wire(&e))?).map_err(|e| wire(&e))?;
    let text = t
        .get_jscalendar()
        .map_err(|e| wire(&e))?
        .to_str()
        .map_err(|e| Error::Stored(e.to_string()))?;
    let task: Task = serde_json::from_str(text)?;
    Ok(TaskEntry {
        id: envelope.id,
        task,
        envelope,
    })
}

/// The tasks of a list of the answer.
pub(super) fn entries(
    list: capnp::struct_list::Reader<'_, fs_interfaces::tasks_capnp::task::Owned>,
) -> Result<Vec<TaskEntry>> {
    list.iter().map(entry).collect()
}

fn texts(list: capnp::text_list::Reader<'_>) -> Result<Vec<String>> {
    list.iter()
        .map(|t| {
            Ok(t.map_err(|e| wire(&e))?
                .to_string()
                .unwrap_or_default())
        })
        .collect()
}

/// The report of an import.
pub(super) fn report(r: fs_interfaces::tasks_capnp::import_report::Reader<'_>) -> Result<ImportReport> {
    let created = r
        .get_created()
        .map_err(|e| wire(&e))?
        .iter()
        .map(|u| read_object_id(u).map_err(|e| wire(&e)))
        .collect::<Result<Vec<ObjectId>>>()?;
    Ok(ImportReport {
        created,
        skipped: texts(r.get_skipped().map_err(|e| wire(&e))?)?,
        failed: texts(r.get_failed().map_err(|e| wire(&e))?)?,
        notes: texts(r.get_notes().map_err(|e| wire(&e))?)?,
    })
}

pub(super) fn strings(list: capnp::text_list::Reader<'_>) -> Result<Vec<String>> {
    texts(list)
}
