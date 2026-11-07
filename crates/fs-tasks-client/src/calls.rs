//! The calls of `Items`, one function each, on the connection's thread.
//!
//! Pattern: Adapter — one request in, one reply read back as values of the core. A list is read page
//! by page until the cursor is empty.

use fs_interfaces::rpc::write_object_id;
use fs_interfaces::tasks_capnp::items::Client;
use fs_store::ObjectId;
use fs_tasks_core::{ImportReport, Result, Task, TaskEntry};

use crate::problems::{Subject, error_of};
use crate::read::{entry, report, unreadable};

/// The JSON text of a task, as the server reads it.
pub(crate) fn task_text(task: &Task) -> String {
    task.to_value().to_string()
}

/// `create`.
pub(crate) async fn create(c: &Client, task: String) -> Result<TaskEntry> {
    let mut req = c.create_request();
    req.get().set_jscalendar(task.as_str());
    let answer = req
        .send()
        .promise
        .await
        .map_err(|e| error_of(&e, Subject::None))?;
    entry(
        answer
            .get()
            .and_then(|r| r.get_task())
            .map_err(unreadable)?,
    )
}

/// `edit`.
pub(crate) async fn edit(c: &Client, id: ObjectId, task: String) -> Result<TaskEntry> {
    let mut req = c.edit_request();
    write_object_id(req.get().init_id(), id);
    req.get().set_jscalendar(task.as_str());
    let answer = req
        .send()
        .promise
        .await
        .map_err(|e| error_of(&e, Subject::Task(id)))?;
    entry(
        answer
            .get()
            .and_then(|r| r.get_task())
            .map_err(unreadable)?,
    )
}

/// `delete`.
pub(crate) async fn delete(c: &Client, id: ObjectId) -> Result<()> {
    let mut req = c.delete_request();
    write_object_id(req.get().init_id(), id);
    req.send()
        .promise
        .await
        .map_err(|e| error_of(&e, Subject::Task(id)))?;
    Ok(())
}

/// `get`: `None` when the server says `found = false`.
pub(crate) async fn get(c: &Client, id: ObjectId) -> Result<Option<TaskEntry>> {
    let mut req = c.get_request();
    write_object_id(req.get().init_id(), id);
    let answer = req
        .send()
        .promise
        .await
        .map_err(|e| error_of(&e, Subject::Task(id)))?;
    let r = answer.get().map_err(unreadable)?;
    if !r.get_found() {
        return Ok(None);
    }
    entry(r.get_task().map_err(unreadable)?).map(Some)
}

/// `list`, every page until the cursor is empty. A page that is empty but names a next one cannot
/// be followed to an end, so it is an error instead of a silently short list.
pub(crate) async fn list(c: &Client) -> Result<Vec<TaskEntry>> {
    let mut found: Vec<TaskEntry> = Vec::new();
    let mut cursor: Vec<u8> = Vec::new();
    loop {
        let mut req = c.list_request();
        req.get().init_cursor().set_opaque(&cursor);
        let answer = req
            .send()
            .promise
            .await
            .map_err(|e| error_of(&e, Subject::None))?;
        let page = answer
            .get()
            .and_then(|r| r.get_page())
            .map_err(unreadable)?;
        let before = found.len();
        for item in page.get_items().map_err(unreadable)? {
            found.push(entry(item)?);
        }
        cursor = page
            .get_next()
            .and_then(|n| n.get_opaque())
            .map_err(unreadable)?
            .to_vec();
        if cursor.is_empty() {
            return Ok(found);
        }
        if found.len() == before {
            return Err(unreadable("an empty page that names a next one"));
        }
    }
}

/// `importIcs`.
pub(crate) async fn import(c: &Client, text: String) -> Result<ImportReport> {
    let mut req = c.import_ics_request();
    req.get().set_text(text.as_str());
    let answer = req
        .send()
        .promise
        .await
        .map_err(|e| error_of(&e, Subject::None))?;
    report(
        answer
            .get()
            .and_then(|r| r.get_report())
            .map_err(unreadable)?,
    )
}

/// `exportIcs`.
pub(crate) async fn export(c: &Client, ids: Vec<ObjectId>) -> Result<String> {
    let mut req = c.export_ics_request();
    let count = u32::try_from(ids.len()).map_err(|_| unreadable("too many ids"))?;
    let mut list = req.get().init_ids(count);
    for (i, id) in (0u32..).zip(&ids) {
        write_object_id(list.reborrow().get(i), *id);
    }
    let answer = req
        .send()
        .promise
        .await
        .map_err(|e| error_of(&e, Subject::None))?;
    answer
        .get()
        .and_then(|r| r.get_text())
        .and_then(|t| Ok(t.to_string()?))
        .map_err(unreadable)
}
