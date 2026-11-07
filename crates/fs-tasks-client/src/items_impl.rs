//! `TaskItems` over the socket: the methods the app uses.
//!
//! Pattern: Adapter — each method is one call of `Items`. `get_by_uid` and `query` are required by
//! the trait and answer `Unsupported` by name, as does every method that is not overridden (the
//! trait's defaults, ADR 0045): the app calls none of them, and `DESIGN.md` lists them.

use async_trait::async_trait;
use fs_interfaces::{Capabilities, Unsupported};
use fs_store::ObjectId;
use fs_tasks_core::{ImportReport, Result, Task, TaskEntry, TaskItems, TaskQuery};

use crate::RpcTasks;
use crate::calls;

fn not_called<T>(capability: &str) -> Result<T> {
    Err(Unsupported::new(
        capability,
        "the client of the tasks program does not offer it yet",
    )
    .into())
}

#[async_trait]
impl TaskItems for RpcTasks {
    fn capabilities(&self) -> Capabilities {
        self.report()
    }

    async fn get(&self, id: ObjectId) -> Result<Option<TaskEntry>> {
        self.call(move |r| async move { calls::get(&r.items, id).await })
            .await
    }

    async fn get_by_uid(&self, _uid: &str) -> Result<Option<TaskEntry>> {
        not_called("task.read")
    }

    async fn list(&self) -> Result<Vec<TaskEntry>> {
        self.call(move |r| async move { calls::list(&r.items).await })
            .await
    }

    async fn query(&self, _query: &TaskQuery) -> Result<Vec<TaskEntry>> {
        not_called("task.query")
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
