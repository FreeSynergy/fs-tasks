//! `TaskItems` and `TaskFeed` implemented by `TaskRepo`.
//!
//! Pattern: Facade — each trait method forwards to the flow behind it (`create_inner`,
//! `edit_inner`, `modify`, `query_inner`, ...), so the contract stays one thin file.

use std::sync::Arc;

use async_trait::async_trait;
use fs_store::ObjectId;

use super::handle::TaskRepo;
use crate::error::Result;
use crate::interface::{
    ImportReport, Subscription, TaskEntry, TaskFeed, TaskItems, TaskListener, TaskQuery,
};
use crate::model::{Relation, Task};

#[async_trait]
impl TaskItems for TaskRepo {
    async fn create(&self, task: Task) -> Result<TaskEntry> {
        self.create_inner(task, None).await
    }
    async fn edit(&self, id: ObjectId, task: Task) -> Result<TaskEntry> {
        self.edit_inner(id, task).await
    }
    async fn assign(&self, id: ObjectId, name: &str, email: &str) -> Result<TaskEntry> {
        let (name, email) = (name.to_string(), email.to_string());
        self.modify(id, move |t| {
            t.assign(&name, &email);
        })
        .await
    }
    async fn relate(&self, id: ObjectId, other_uid: &str, relation: Relation) -> Result<TaskEntry> {
        let uid = other_uid.to_string();
        self.modify(id, move |t| t.relate(&uid, relation)).await
    }
    async fn label(&self, id: ObjectId, label: &str) -> Result<TaskEntry> {
        let label = label.to_string();
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
        self.delete_inner(id).await
    }
    async fn list(&self) -> Result<Vec<TaskEntry>> {
        self.query_inner(&TaskQuery::new()).await
    }
    async fn query(&self, query: &TaskQuery) -> Result<Vec<TaskEntry>> {
        self.query_inner(query).await
    }
    async fn import_ics(&self, text: &str) -> Result<ImportReport> {
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
