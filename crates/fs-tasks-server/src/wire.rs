//! Tasks and reports on the wire.
//!
//! Pattern: Adapter — `TaskEntry`, `ImportReport` and ids between Rust and `tasks.capnp`. The
//! JSCalendar JSON stays authoritative for the content; the envelope is the routing summary.

use std::borrow::Cow;

use fs_interfaces::rpc::{
    MAX_PAGE_BYTES, capped, check_intake, invalid_request, read_object_id, write_envelope,
    write_object_id,
};
use fs_interfaces::tasks_capnp::items;
use fs_interfaces::{common_capnp, tasks_capnp};
use fs_store::ObjectId;
use fs_tasks_core::contract::read_filter;
use fs_tasks_core::{ImportReport, Revision, Task, TaskEntry, TaskQuery};

use crate::errors::{invalid, rpc_error};

fn count(n: usize) -> capnp::Result<u32> {
    u32::try_from(n).map_err(|_| capnp::Error::failed("list too long".into()))
}

/// Write one task.
pub(crate) fn write_task(mut out: tasks_capnp::task::Builder<'_>, entry: &TaskEntry) {
    write_envelope(out.reborrow().init_envelope(), &entry.envelope);
    out.set_jscalendar(entry.task.to_value().to_string().as_str());
    out.set_revision(Revision::of(entry).get());
}

/// Read the JSCalendar text of a request.
pub(crate) fn read_task(text: capnp::text::Reader<'_>) -> capnp::Result<Task> {
    let text = text.to_str()?;
    check_intake(text.len())?;
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| invalid(e.to_string(), &e.to_string()))?;
    // The core says what is wrong, issue by issue; a text that is no task at all is one issue.
    Task::from_value(value).map_err(|e| match fs_tasks_core::Error::from(e) {
        fs_tasks_core::Error::Json(inner) => invalid(inner.to_string(), &inner.to_string()),
        other => rpc_error(other),
    })
}

/// The filter of a `query` request; none given = no restriction.
pub(crate) fn filter_of(p: &items::query_params::Reader<'_>) -> capnp::Result<TaskQuery> {
    if !p.has_filter() {
        return Ok(TaskQuery::new());
    }
    read_filter(p.get_filter()?).map_err(|e| invalid_request(e.extra))
}

/// A required text of a request; empty is refused here, so it never reaches the store as an index.
pub(crate) fn required<'a>(text: capnp::text::Reader<'a>, what: &str) -> capnp::Result<&'a str> {
    let text = text.to_str()?;
    if text.is_empty() {
        return Err(invalid(format!("{what} must not be empty"), what));
    }
    check_intake(text.len())?;
    Ok(text)
}

/// What one id of a report weighs, and one text beyond its own bytes.
const ID_BYTES: usize = 24;
const TEXT_BYTES: usize = 16;
/// Room for the last line of a list that was cut.
const MORE_BYTES: usize = 64;

/// The texts of `items` that fit `budget`, each cut to what an answer carries, ending with
/// `... N more` if not all do. Takes what it used from `budget`.
fn fitting<'a>(items: &'a [String], budget: &mut usize) -> Vec<Cow<'a, str>> {
    let mut kept: Vec<Cow<'a, str>> = Vec::new();
    for text in items {
        let text = capped(text);
        let weight = text.len() + TEXT_BYTES;
        if weight > *budget {
            kept.push(format!("... {} more", items.len() - kept.len()).into());
            break;
        }
        *budget -= weight;
        kept.push(text.into());
    }
    kept
}

/// Write the report of an import. `created` stays whole: those tasks exist, and a consumer has no other
/// way to learn their ids. The texts share what is left of a page, what failed first.
pub(crate) fn write_report(
    mut out: tasks_capnp::import_report::Builder<'_>,
    report: &ImportReport,
) -> capnp::Result<()> {
    let mut created = out.reborrow().init_created(count(report.created.len())?);
    for (i, id) in (0u32..).zip(&report.created) {
        write_object_id(created.reborrow().get(i), *id);
    }
    let texts = |list: capnp::text_list::Builder<'_>, items: &[Cow<'_, str>]| {
        let mut list = list;
        for (i, s) in (0u32..).zip(items) {
            list.set(i, s.as_ref());
        }
    };
    let mut budget = MAX_PAGE_BYTES
        .saturating_sub(report.created.len().saturating_mul(ID_BYTES))
        .saturating_sub(3 * MORE_BYTES);
    let failed = fitting(&report.failed, &mut budget);
    let skipped = fitting(&report.skipped, &mut budget);
    let notes = fitting(&report.notes, &mut budget);
    texts(out.reborrow().init_failed(count(failed.len())?), &failed);
    texts(out.reborrow().init_skipped(count(skipped.len())?), &skipped);
    texts(out.init_notes(count(notes.len())?), &notes);
    Ok(())
}

/// Read a list of ids.
pub(crate) fn read_ids(
    list: capnp::struct_list::Reader<'_, common_capnp::uuid::Owned>,
) -> capnp::Result<Vec<ObjectId>> {
    list.iter().map(read_object_id).collect()
}
