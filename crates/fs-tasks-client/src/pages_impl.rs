//! `TaskPages` over the socket.
//!
//! Pattern: Adapter — the `cursor` of `list` and `query` in `tasks.capnp` as the steps of
//! [`TaskPages`]. Answers `Unsupported` until the tests are in.

use async_trait::async_trait;
use fs_tasks_core::{Result, TaskQuery};

use crate::RpcTasks;
use crate::pages::{Cursor, TaskPage, TaskPages};

#[async_trait]
impl TaskPages for RpcTasks {
    async fn first(&self, _query: &TaskQuery) -> Result<TaskPage> {
        Err(fs_interfaces::Unsupported::new("tasks", "pages: not yet").into())
    }

    async fn next(&self, _query: &TaskQuery, _cursor: &Cursor) -> Result<TaskPage> {
        Err(fs_interfaces::Unsupported::new("tasks", "pages: not yet").into())
    }
}
