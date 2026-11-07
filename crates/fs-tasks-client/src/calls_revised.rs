//! The calls that carry the server's revision: `editIf`, and `get` and `list` with `Task.revision`.
//!
//! Pattern: Adapter — one request in, one reply read back as values of the core, with the revision
//! of the answer handed on as it came (`RevisedTasks`).

use fs_interfaces::rpc::write_object_id;
use fs_interfaces::tasks_capnp::items::Client;
use fs_store::ObjectId;
use fs_tasks_core::{Result, Revision, TaskEntry};

use crate::calls::pages;
use crate::problems::{Subject, error_of};
use crate::read::{entry, revised, unreadable};
use crate::revised::Revised;

/// `editIf`: only if the task still has `revision`; the server answers `conflict:` otherwise.
pub(crate) async fn edit_if(
    c: &Client,
    id: ObjectId,
    revision: Revision,
    task: String,
) -> Result<TaskEntry> {
    let mut req = c.edit_if_request();
    write_object_id(req.get().init_id(), id);
    req.get().set_revision(revision.get());
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

/// `get` with the revision of the answer: `None` when the server says `found = false`.
pub(crate) async fn get_revised(c: &Client, id: ObjectId) -> Result<Option<Revised>> {
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
    revised(r.get_task().map_err(unreadable)?).map(Some)
}

/// `list` with the revisions, every page.
pub(crate) async fn list_revised(c: &Client) -> Result<Vec<Revised>> {
    pages(c, None).await
}
