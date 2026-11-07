//! What the server sends, as the values of the core.
//!
//! Pattern: Adapter — `Task` and `ImportReport` of `tasks.capnp` to `TaskEntry` and `ImportReport`.
//! A reply that cannot be read is a store error saying so: the request is not to blame.

use fs_interfaces::rpc::{read_envelope, read_object_id};
use fs_interfaces::tasks_capnp::{import_report, task};
use fs_store::Error as StoreError;
use fs_tasks_core::{Error, ImportReport, Result, Revision, Task, TaskEntry};

use crate::revised::Revised;

/// A reply that does not read.
pub(crate) fn unreadable(e: impl std::fmt::Display) -> Error {
    Error::Store(StoreError::Backend(format!(
        "tasks server: it sent something unreadable: {e}"
    )))
}

fn wire<T>(r: capnp::Result<T>) -> Result<T> {
    r.map_err(unreadable)
}

fn text(t: capnp::Result<capnp::text::Reader<'_>>) -> Result<String> {
    wire(t.and_then(|t| Ok(t.to_string()?)))
}

/// A task: the envelope as sent, the JSCalendar text parsed as the task. The id is the envelope's.
pub(crate) fn entry(r: task::Reader<'_>) -> Result<TaskEntry> {
    revised(r).map(|read| read.entry)
}

/// A task and the revision the server gave it (`Task.revision`), handed on as it came.
pub(crate) fn revised(r: task::Reader<'_>) -> Result<Revised> {
    let envelope = wire(r.get_envelope().and_then(read_envelope))?;
    let value = serde_json::from_str(&text(r.get_jscalendar())?).map_err(unreadable)?;
    let task = Task::from_value(value).map_err(unreadable)?;
    Ok(Revised {
        revision: Revision::from(r.get_revision()),
        entry: TaskEntry {
            id: envelope.id,
            task,
            envelope,
        },
    })
}

fn list(l: capnp::Result<capnp::text_list::Reader<'_>>) -> Result<Vec<String>> {
    wire(l)?.iter().map(text).collect()
}

/// The report of an import.
pub(crate) fn report(r: import_report::Reader<'_>) -> Result<ImportReport> {
    Ok(ImportReport {
        created: wire(r.get_created())?
            .iter()
            .map(|id| wire(read_object_id(id)))
            .collect::<Result<_>>()?,
        skipped: list(r.get_skipped())?,
        failed: list(r.get_failed())?,
        notes: list(r.get_notes())?,
    })
}
