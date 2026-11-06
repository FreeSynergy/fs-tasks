//! Tasks and reports on the wire.
//!
//! Pattern: Adapter — `TaskEntry`, `ImportReport` and ids between Rust and `tasks.capnp`. The
//! JSCalendar JSON stays authoritative for the content; the envelope is the routing summary.

use fs_interfaces::rpc::{
    DEFAULT_PAGE_SIZE, Page, check_intake, envelope_weight, read_object_id, write_envelope,
    write_object_id,
};
use fs_interfaces::{common_capnp, tasks_capnp};
use fs_store::ObjectId;
use fs_tasks_core::{ImportReport, Task, TaskEntry};

use crate::errors::invalid;

fn count(n: usize) -> capnp::Result<u32> {
    u32::try_from(n).map_err(|_| capnp::Error::failed("list too long".into()))
}

/// Write one task.
pub(crate) fn write_task(mut out: tasks_capnp::task::Builder<'_>, entry: &TaskEntry) {
    write_envelope(out.reborrow().init_envelope(), &entry.envelope);
    out.set_jscalendar(entry.task.to_value().to_string().as_str());
}

/// Read the JSCalendar text of a request.
pub(crate) fn read_task(text: capnp::text::Reader<'_>) -> capnp::Result<Task> {
    let text = text.to_str()?;
    check_intake(text.len())?;
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| invalid(e.to_string()))?;
    Task::from_value(value).map_err(|e| invalid(e.to_string()))
}

/// A required text of a request; empty is refused here, so it never reaches the store as an index.
pub(crate) fn required<'a>(text: capnp::text::Reader<'a>, what: &str) -> capnp::Result<&'a str> {
    let text = text.to_str()?;
    if text.is_empty() {
        return Err(invalid(format!("{what} must not be empty")));
    }
    Ok(text)
}

/// Cut the page at `cursor` and write it.
pub(crate) fn write_page(
    mut out: tasks_capnp::task_page::Builder<'_>,
    all: Vec<TaskEntry>,
    cursor: capnp::Result<common_capnp::cursor::Reader<'_>>,
) -> capnp::Result<()> {
    let opaque = cursor?.get_opaque()?;
    let page = Page::of_weighted(all, opaque, DEFAULT_PAGE_SIZE, |e: &TaskEntry| {
        e.task.to_value().to_string().len() + envelope_weight(&e.envelope)
    })?;
    let mut items = out.reborrow().init_items(count(page.items.len())?);
    for (i, entry) in (0u32..).zip(&page.items) {
        write_task(items.reborrow().get(i), entry);
    }
    out.init_next().set_opaque(&page.next);
    Ok(())
}

/// Write the report of an import.
pub(crate) fn write_report(
    mut out: tasks_capnp::import_report::Builder<'_>,
    report: &ImportReport,
) -> capnp::Result<()> {
    let mut created = out.reborrow().init_created(count(report.created.len())?);
    for (i, id) in (0u32..).zip(&report.created) {
        write_object_id(created.reborrow().get(i), *id);
    }
    let texts = |list: capnp::text_list::Builder<'_>, items: &[String]| {
        let mut list = list;
        for (i, s) in (0u32..).zip(items) {
            list.set(i, s.as_str());
        }
    };
    let skipped = report.skipped.len();
    texts(
        out.reborrow().init_skipped(count(skipped)?),
        &report.skipped,
    );
    let failed = report.failed.len();
    texts(out.reborrow().init_failed(count(failed)?), &report.failed);
    let notes = report.notes.len();
    texts(out.init_notes(count(notes)?), &report.notes);
    Ok(())
}

/// Read a list of ids.
pub(crate) fn read_ids(
    list: capnp::struct_list::Reader<'_, common_capnp::uuid::Owned>,
) -> capnp::Result<Vec<ObjectId>> {
    list.iter().map(read_object_id).collect()
}
