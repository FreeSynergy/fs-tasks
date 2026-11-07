//! `Items.editIf`: compare-and-set on the revision.
//!
//! Pattern: Adapter — wire request in, one `TaskItems::edit_checked` call, wire response out. The
//! compare and the write are one step in the provider; a refusal arrives as `tasks/conflict` (409) whose
//! detail starts with `conflict:`.

use std::sync::Arc;

use fs_interfaces::rpc::{read_object_id, run_to_end};
use fs_interfaces::tasks_capnp::items;
use fs_tasks_core::{Revision, TaskItems};

use crate::errors::rpc_error;
use crate::wire::{read_task, write_task};

pub(crate) async fn edit_if(
    provider: &Arc<dyn TaskItems>,
    params: items::EditIfParams,
    mut results: items::EditIfResults,
) -> Result<(), capnp::Error> {
    let p = params.get()?;
    let id = read_object_id(p.get_id()?)?;
    let revision = Revision::from(p.get_revision());
    let task = read_task(p.get_jscalendar()?)?;
    let provider = provider.clone();
    let entry = run_to_end(async move { provider.edit_checked(id, revision, task).await })
        .await?
        .map_err(rpc_error)?;
    write_task(results.get().init_task(), &entry);
    Ok(())
}
