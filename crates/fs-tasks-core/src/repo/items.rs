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
use crate::model::{ActorType, Relation, Task};
use crate::plan::Readiness;
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
        self.writing(async || self.edit_inner(id, task).await).await
    }
    async fn edit_checked(
        &self,
        id: ObjectId,
        expected: Revision,
        task: Task,
    ) -> Result<TaskEntry> {
        // The turn makes compare and write one step: nobody else writes in between.
        self.writing(async || {
            let current = self.entry(id).await?.ok_or(Error::NotFound(id))?;
            if Revision::of(&current) != expected {
                return Err(Error::Changed(id));
            }
            self.edit_inner(id, task).await
        })
        .await
    }
    async fn assign(&self, id: ObjectId, name: &str, email: &str) -> Result<TaskEntry> {
        let (name, email) = (name.to_string(), email.to_string());
        self.writing(async || {
            self.modify(id, move |t| {
                t.assign(&name, &email);
            })
            .await
        })
        .await
    }
    async fn relate(&self, id: ObjectId, other_uid: &str, relation: Relation) -> Result<TaskEntry> {
        let uid = other_uid.to_string();
        self.writing(async || {
            let holder = self.entry(id).await?.ok_or(Error::NotFound(id))?;
            self.check_no_cycle(&holder.task, &uid, relation).await?;
            self.modify(id, move |t| t.relate(&uid, relation)).await
        })
        .await
    }
    async fn assign_actor(
        &self,
        id: ObjectId,
        actor: &str,
        kind: ActorType,
        name: &str,
    ) -> Result<TaskEntry> {
        let (actor, name) = (actor.to_string(), name.to_string());
        let _one_at_a_time = self.writes.lock().await;
        self.modify(id, move |t| {
            t.assign_actor(&actor, kind, &name);
        })
        .await
    }
    async fn claim(
        &self,
        id: ObjectId,
        by: &str,
        until: chrono::DateTime<chrono::Utc>,
    ) -> Result<TaskEntry> {
        self.claim_inner(id, by, until).await
    }
    async fn release_claim(&self, id: ObjectId, by: &str) -> Result<TaskEntry> {
        self.release_inner(id, by).await
    }
    async fn relate_with_gap(
        &self,
        id: ObjectId,
        other_uid: &str,
        relation: Relation,
        gap: &str,
    ) -> Result<TaskEntry> {
        let (uid, gap) = (other_uid.to_string(), gap.to_string());
        self.writing(async || {
            let holder = self.entry(id).await?.ok_or(Error::NotFound(id))?;
            // The gap is checked before anything is written (and before the graph is read).
            holder.task.clone().relate_with_gap(&uid, relation, &gap)?;
            self.check_no_cycle(&holder.task, &uid, relation).await?;
            let stored = self.stored(id).await?.ok_or(Error::NotFound(id))?;
            let mut task = super::crud::entry_of(stored)?.task;
            task.relate_with_gap(&uid, relation, &gap)?;
            self.edit_inner(id, task).await
        })
        .await
    }
    async fn readiness(&self, id: ObjectId) -> Result<Readiness> {
        self.readiness_inner(id).await
    }
    async fn readiness_many(&self, ids: &[ObjectId]) -> Result<Vec<(ObjectId, Readiness)>> {
        let mut entries = Vec::with_capacity(ids.len());
        for id in ids {
            entries.push(self.entry(*id).await?.ok_or(Error::NotFound(*id))?);
        }
        let judged = self.readiness_many_entries(&entries).await?;
        Ok(entries
            .iter()
            .map(|e| {
                let r = judged.get(&e.task.uid).cloned().unwrap_or(Readiness::Ready);
                (e.id, r)
            })
            .collect())
    }
    async fn blocked_by(&self, id: ObjectId) -> Result<Vec<TaskEntry>> {
        self.blocked_by_inner(id).await
    }
    async fn dependents(&self, id: ObjectId) -> Result<Vec<TaskEntry>> {
        self.dependents_inner(id).await
    }
    async fn label(&self, id: ObjectId, label: &str) -> Result<TaskEntry> {
        let label = label.to_string();
        self.writing(async || self.modify(id, move |t| t.label(&label)).await)
            .await
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
        self.writing(async || self.delete_inner(id).await).await
    }
    async fn list(&self) -> Result<Vec<TaskEntry>> {
        self.query_inner(&TaskQuery::new()).await
    }
    async fn query(&self, query: &TaskQuery) -> Result<Vec<TaskEntry>> {
        match query.ready {
            Some(wanted) => self.query_ready(query, wanted).await,
            None => self.query_inner(query).await,
        }
    }
    async fn page(
        &self,
        query: &TaskQuery,
        after: Option<&Place>,
        size: usize,
    ) -> Result<TaskPage> {
        match query.ready {
            Some(wanted) => self.page_ready(query, wanted, after, size).await,
            None => self.page_inner(query, after, size).await,
        }
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
        "dependency.evaluate",
        "dependency.cycle-check",
        "task.claim",
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
