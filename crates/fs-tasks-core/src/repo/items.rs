//! `TaskItems` and `TaskFeed` implemented by `TaskRepo`.
//!
//! Pattern: Facade — each trait method forwards to the flow behind it (`create_inner`,
//! `edit_inner`, `modify`, `query_inner`, ...), so the contract stays one thin file.

use std::sync::Arc;

use async_trait::async_trait;
use fs_interfaces::Capabilities;
use fs_objects::{ObjectId, StoreId};

use super::handle::TaskRepo;
use crate::error::{Error, Result};
use crate::interface::{
    CAPABILITIES, ImportReport, Place, Subscription, TaskEntry, TaskFeed, TaskItems, TaskListener,
    TaskPage, TaskQuery,
};
use crate::model::{Relation, Task};
use crate::revision::Revision;

#[async_trait]
impl TaskItems for TaskRepo {
    fn capabilities(&self) -> Capabilities {
        capabilities()
    }
    fn identity(&self) -> Option<StoreId> {
        self.info.as_ref().map(|info| info.identity())
    }
    async fn create(&self, task: Task) -> Result<TaskEntry> {
        let _one_at_a_time = self.writes.lock().await;
        self.create_inner(task, None).await
    }
    async fn edit(&self, id: ObjectId, task: Task) -> Result<TaskEntry> {
        let _one_at_a_time = self.writes.lock().await;
        self.edit_inner(id, task).await
    }
    async fn edit_checked(
        &self,
        id: ObjectId,
        expected: Revision,
        task: Task,
    ) -> Result<TaskEntry> {
        // The turn makes compare and write one step: nobody else writes in between.
        let _one_at_a_time = self.writes.lock().await;
        let current = self.entry(id).await?.ok_or(Error::NotFound(id))?;
        if Revision::of(&current) != expected {
            return Err(Error::Changed(id));
        }
        self.edit_inner(id, task).await
    }
    async fn assign(&self, id: ObjectId, name: &str, email: &str) -> Result<TaskEntry> {
        let (name, email) = (name.to_string(), email.to_string());
        let _one_at_a_time = self.writes.lock().await;
        self.modify(id, move |t| {
            t.assign(&name, &email);
        })
        .await
    }
    async fn relate(&self, id: ObjectId, other_uid: &str, relation: Relation) -> Result<TaskEntry> {
        let uid = other_uid.to_string();
        let _one_at_a_time = self.writes.lock().await;
        self.modify(id, move |t| t.relate(&uid, relation)).await
    }
    async fn label(&self, id: ObjectId, label: &str) -> Result<TaskEntry> {
        let label = label.to_string();
        let _one_at_a_time = self.writes.lock().await;
        self.modify(id, move |t| t.label(&label)).await
    }
    async fn get(&self, id: ObjectId) -> Result<Option<TaskEntry>> {
        self.entry(id).await
    }
    async fn get_by_uid(&self, uid: &str) -> Result<Option<TaskEntry>> {
        self.stored_by_uid(uid)
            .await?
            .map(super::crud::entry_of)
            .transpose()
    }
    async fn delete(&self, id: ObjectId) -> Result<()> {
        let _one_at_a_time = self.writes.lock().await;
        self.delete_inner(id).await
    }
    async fn list(&self) -> Result<Vec<TaskEntry>> {
        self.query_inner(&TaskQuery::new()).await
    }
    async fn query(&self, query: &TaskQuery) -> Result<Vec<TaskEntry>> {
        self.query_inner(query).await
    }
    async fn page(
        &self,
        query: &TaskQuery,
        after: Option<&Place>,
        size: usize,
    ) -> Result<TaskPage> {
        self.page_inner(query, after, size).await
    }
    async fn import_ics(&self, text: &str) -> Result<ImportReport> {
        let _one_at_a_time = self.writes.lock().await;
        self.import_ics_inner(text).await
    }
    async fn export_ics(&self, ids: &[ObjectId]) -> Result<String> {
        self.export_ics_inner(ids).await
    }
}

impl TaskFeed for TaskRepo {
    fn subscribe(&self, listener: Arc<dyn TaskListener>) -> Subscription {
        self.feed.add(listener)
    }
}

/// The store-backed provider offers every method and the live feed. The five ids of the schema
/// plus the ids this repo names for the other methods (the schema gives none, see `DESIGN.md`).
fn capabilities() -> Capabilities {
    let mut caps = Capabilities::new("fs-tasks");
    for id in CAPABILITIES {
        caps.supported(id);
    }
    for id in [
        "task.read",
        "task.query",
        "task.delete",
        "task.import",
        "task.export",
        "task.subscribe",
    ] {
        caps.supported(id);
    }
    for id in [
        "board.open",
        "board.create",
        "column.create",
        "column.limit",
        "card.move",
    ] {
        caps.unsupported(
            id,
            "Boards is a placeholder in the contract; not built yet, it is possible",
        );
    }
    caps
}
