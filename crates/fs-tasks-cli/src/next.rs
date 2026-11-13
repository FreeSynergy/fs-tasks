//! `next`: the first ready task nobody else holds, claimed for the worker.
//!
//! Pattern: Command (executor) — split from `work` to keep the files short; the claim itself is `work::take`.

use chrono::Utc;
use fs_interfaces::rpc::write_uuid;
use fs_interfaces::tasks_capnp::items;
use fs_objects::ObjectId;
use fs_tasks_core::Task;
use fs_tasks_core::contract::readiness_from_wire;
use fs_tasks_core::plan::Readiness;

use crate::edit::{all_tasks, change, project_of};
use crate::fail::Failure;
use crate::work::{take, until_of, worker};

async fn ready(items: &items::Client, id: ObjectId) -> Result<bool, Failure> {
    let mut req = items.readiness_request();
    write_uuid(req.get().init_id(), Some(id.as_uuid()));
    let answer = req.send().promise.await.map_err(|e| e.to_string())?;
    let report = answer
        .get()
        .and_then(items::readiness_results::Reader::get_report)
        .map_err(|e| e.to_string())?;
    let by = report
        .get_by()
        .map_err(|e| e.to_string())?
        .iter()
        .map(|t| {
            Ok(t.map_err(|e| e.to_string())?
                .to_string()
                .unwrap_or_default())
        })
        .collect::<Result<Vec<_>, String>>()?;
    let state = report
        .get_readiness()
        .map_err(|e| capnp::Error::from(e).to_string())?;
    Ok(matches!(readiness_from_wire(state, by), Readiness::Ready))
}

/// The first open, ready task without somebody else's claim (the worker's own claim counts), claimed.
pub(crate) async fn next(
    items: &items::Client,
    name: &str,
    project: Option<&str>,
) -> Result<Option<(ObjectId, Task)>, Failure> {
    let iri = worker(name)?;
    let until = until_of(None, Utc::now())?;
    for (id, task, _) in all_tasks(items).await? {
        let free = task.claimed_at(Utc::now()).is_none_or(|c| c.by == iri);
        if task.effective_progress() != "needs-action"
            || !free
            || project.is_some_and(|p| project_of(&task) != Some(p))
            || !ready(items, id).await?
        {
            continue;
        }
        match change(items, id, |t| take(t, name, &iri, until)).await {
            Ok(task) => return Ok(Some((id, task))),
            // Somebody was quicker: the next one.
            Err(f) if f.conflict => {}
            Err(f) => return Err(f),
        }
    }
    Ok(None)
}
