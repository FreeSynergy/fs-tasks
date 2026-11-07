//! `TaskItems` over the socket: the methods the app uses.
//!
//! Pattern: Adapter — each method is one call of `Items` (`edit_checked` is `editIf`). `assign`, `relate` and `label` are not
//! overridden, so they answer the `Unsupported` of the trait's defaults (ADR 0045); `capabilities()`
//! says the same (`peer.rs`), and `DESIGN.md` lists them.

use async_trait::async_trait;
use fs_interfaces::Capabilities;
use fs_objects::ObjectId;
use fs_tasks_core::{ImportReport, Result, Revision, Task, TaskEntry, TaskItems, TaskQuery};

use crate::RpcTasks;
use crate::revised::{Revised, RevisedTasks};
use crate::{calls, calls_revised};

#[async_trait]
impl TaskItems for RpcTasks {
    fn capabilities(&self) -> Capabilities {
        self.report()
    }

    async fn get(&self, id: ObjectId) -> Result<Option<TaskEntry>> {
        self.call(move |r| async move { calls::get(&r.items, id).await })
            .await
    }

    async fn get_by_uid(&self, uid: &str) -> Result<Option<TaskEntry>> {
        let uid = uid.to_string();
        self.call(move |r| async move { calls::get_by_uid(&r.items, uid).await })
            .await
    }

    async fn list(&self) -> Result<Vec<TaskEntry>> {
        self.call(move |r| async move { calls::list(&r.items).await })
            .await
    }

    async fn query(&self, query: &TaskQuery) -> Result<Vec<TaskEntry>> {
        let query = query.clone();
        self.call(move |r| async move { calls::query(&r.items, query).await })
            .await
    }

    async fn create(&self, task: Task) -> Result<TaskEntry> {
        let task = calls::task_text(&task);
        self.call(move |r| async move { calls::create(&r.items, task).await })
            .await
    }

    async fn edit(&self, id: ObjectId, task: Task) -> Result<TaskEntry> {
        let task = calls::task_text(&task);
        self.call(move |r| async move { calls::edit(&r.items, id, task).await })
            .await
    }

    async fn edit_checked(
        &self,
        id: ObjectId,
        expected: Revision,
        task: Task,
    ) -> Result<TaskEntry> {
        let task = calls::task_text(&task);
        self.call(
            move |r| async move { calls_revised::edit_if(&r.items, id, expected, task).await },
        )
        .await
    }

    async fn delete(&self, id: ObjectId) -> Result<()> {
        self.call(move |r| async move { calls::delete(&r.items, id).await })
            .await
    }

    async fn import_ics(&self, text: &str) -> Result<ImportReport> {
        let text = text.to_string();
        self.call(move |r| async move { calls::import(&r.items, text).await })
            .await
    }

    async fn export_ics(&self, ids: &[ObjectId]) -> Result<String> {
        let ids = ids.to_vec();
        self.call(move |r| async move { calls::export(&r.items, ids).await })
            .await
    }
}

#[async_trait]
impl RevisedTasks for RpcTasks {
    async fn get_revised(&self, id: ObjectId) -> Result<Option<Revised>> {
        self.call(move |r| async move { calls_revised::get_revised(&r.items, id).await })
            .await
    }

    async fn list_revised(&self) -> Result<Vec<Revised>> {
        self.call(move |r| async move { calls_revised::list_revised(&r.items).await })
            .await
    }
}
