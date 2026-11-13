//! Changing a task safely: read, change, write only if nobody wrote in between.
//!
//! Pattern: Optimistic lock (compare and set) — `editIf` on the revision that was read. Several clients (the
//! window, many workers) change tasks at the same time; whoever loses the race reads again and decides again,
//! so a claim is decided on what is stored, never on what was stored a moment ago.

use fs_interfaces::rpc::write_uuid;
use fs_interfaces::tasks_capnp::items;
use fs_objects::ObjectId;
use fs_tasks_core::Task;
use serde_json::{Value, json};

use crate::fail::Failure;
use crate::run::{all, fetch};

/// How often a change is tried again after somebody else wrote first.
const TRIES: usize = 8;

pub(crate) fn task_of(text: &str) -> Result<Task, String> {
    serde_json::from_str(text).map_err(|e| format!("not a task: {e}"))
}

/// Every task with its id and revision.
pub(crate) async fn all_tasks(
    items: &items::Client,
) -> Result<Vec<(ObjectId, Task, u64)>, Failure> {
    let mut out = vec![];
    for (id, text, revision) in all(items).await? {
        if let Ok(task) = task_of(&text) {
            out.push((id, task, revision));
        }
    }
    Ok(out)
}

/// Apply `change` to the task and store it if it is still the one that was read; if not, read again and apply again.
/// `change` may refuse (nothing is written then).
pub(crate) async fn change<F>(
    items: &items::Client,
    id: ObjectId,
    mut change: F,
) -> Result<Task, Failure>
where
    F: FnMut(&mut Task) -> Result<(), Failure>,
{
    for _ in 0..TRIES {
        let (text, revision) = fetch(items, id).await?;
        let mut task = task_of(&text)?;
        change(&mut task)?;
        let mut req = items.edit_if_request();
        write_uuid(req.get().init_id(), Some(id.as_uuid()));
        req.get().set_revision(revision);
        req.get()
            .set_jscalendar(task.to_value().to_string().as_str());
        match req.send().promise.await {
            Ok(_) => return Ok(task),
            Err(e) => {
                // A lost race looks like any refusal: read again; if the task is the same, the refusal is real.
                if fetch(items, id).await?.1 == revision {
                    return Err(Failure::from(e.to_string()));
                }
            }
        }
    }
    Err("the task keeps changing under the command; gave up".into())
}

/// The status word of a task as the command line says it.
pub(crate) fn status_word(task: &Task) -> &'static str {
    match task.effective_progress() {
        "in-process" => "in-progress",
        "completed" => "done",
        "failed" => "failed",
        "cancelled" => "cancelled",
        _ => "open",
    }
}

/// The project of a task: the label `project:<name>`.
pub(crate) fn project_of(task: &Task) -> Option<&str> {
    task.keywords
        .keys()
        .find_map(|k| k.strip_prefix("project:"))
}

/// A task for a script.
pub(crate) fn view(id: ObjectId, task: &Task) -> Value {
    let claim = task.claim();
    json!({
        "id": id.to_string(),
        "title": task.title,
        "status": status_word(task),
        "project": project_of(task),
        "claimed_by": claim.as_ref().map(|c| c.by.clone()),
        "claimed_until": claim.map(|c| c.until.format("%Y-%m-%dT%H:%M:%SZ").to_string()),
        "note": task.note(),
        "objective": task.check().is_some(),
        "check": task.check(),
        "origin": task.origin(),
        "result": task.result(),
    })
}
