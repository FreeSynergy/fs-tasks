//! `TaskPages` over the socket.
//!
//! Pattern: Adapter — the `cursor` of `list` and `query` in `tasks.capnp` as the steps of
//! [`TaskPages`]. An empty query is `list`, any filter is `query`; the cursor goes back as the
//! server gave it, and an empty one means the end.

use async_trait::async_trait;
use fs_tasks_core::{Result, TaskQuery};

use crate::RpcTasks;
use crate::calls_page::page;
use crate::pages::{Cursor, TaskPage, TaskPages};

impl RpcTasks {
    async fn page_at(&self, query: &TaskQuery, cursor: Vec<u8>) -> Result<TaskPage> {
        let query = (*query != TaskQuery::new()).then(|| query.clone());
        let (items, next) = self
            .call(move |r| async move { page(&r.items, query.as_ref(), &cursor).await })
            .await?;
        Ok(TaskPage {
            items,
            next: (!next.is_empty()).then(|| Cursor::from_bytes(next)),
        })
    }
}

#[async_trait]
impl TaskPages for RpcTasks {
    async fn first(&self, query: &TaskQuery) -> Result<TaskPage> {
        self.page_at(query, Vec::new()).await
    }

    async fn next(&self, query: &TaskQuery, cursor: &Cursor) -> Result<TaskPage> {
        self.page_at(query, cursor.as_bytes().to_vec()).await
    }
}
