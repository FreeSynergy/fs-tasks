//! Create, read and delete of single tasks.
//!
//! Pattern: Repository — the CRUD side of the tasks; every change goes through the store, which
//! logs it and makes it undoable, and is announced to the subscribers. (`edit` has its own file.)

use fs_store::{Blob, ObjectId, Query, StoredObject};

use super::consts::{CONTEXT, TASK_TYPE};
use super::handle::TaskRepo;
use crate::error::{Error, Result};
use crate::interface::{TaskChange, TaskEntry};
use crate::model::Task;
use crate::now_utc;
use crate::time::tzdb_version;

impl TaskRepo {
    pub(super) async fn create_inner(
        &self,
        mut task: Task,
        original: Option<Blob>,
    ) -> Result<TaskEntry> {
        let now = now_utc();
        task.normalise();
        task.created.get_or_insert_with(|| now.clone());
        task.updated.get_or_insert(now);
        if self.stored_by_uid(&task.uid).await?.is_some() {
            return Err(Error::Conflict(format!(
                "a task with uid {} exists",
                task.uid
            )));
        }
        let mut o = self.build(&task, None, original.clone())?;
        if original.is_some() {
            o.index
                .entry("tzdb_version".into())
                .or_default()
                .push(tzdb_version().into());
        }
        let id = o.envelope.id;
        self.objects.put(o, &self.actor).await?;
        self.feed.notify(&TaskChange::Created(id));
        self.entry(id).await?.ok_or(Error::NotFound(id))
    }

    pub(super) async fn stored(&self, id: ObjectId) -> Result<Option<StoredObject>> {
        Ok(self
            .objects
            .get(id)
            .await?
            .filter(|o| o.envelope.object_type == TASK_TYPE))
    }

    pub(super) async fn stored_by_uid(&self, uid: &str) -> Result<Option<StoredObject>> {
        let q = Query::new()
            .of_type(TASK_TYPE)
            .in_context(CONTEXT)
            .with_index("uid", uid)
            .limit(1);
        Ok(self.queries.query(&q).await?.into_iter().next())
    }

    pub(super) async fn entry(&self, id: ObjectId) -> Result<Option<TaskEntry>> {
        self.stored(id).await?.map(entry_of).transpose()
    }

    pub(super) async fn delete_inner(&self, id: ObjectId) -> Result<()> {
        self.stored(id).await?.ok_or(Error::NotFound(id))?;
        self.objects.delete(id, &self.actor).await?;
        self.feed.notify(&TaskChange::Deleted(id));
        Ok(())
    }
}

/// The typed entry of a stored object.
pub(super) fn entry_of(o: StoredObject) -> Result<TaskEntry> {
    Ok(TaskEntry {
        id: o.envelope.id,
        task: Task::from_value(o.content)?,
        envelope: o.envelope,
    })
}
