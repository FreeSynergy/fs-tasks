//! The counting reader of pages for the tests.
//!
//! Pattern: Decorator over the real client.
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use fs_tasks_client::{Cursor, RpcTasks, TaskPage, TaskPages};
use fs_tasks_core::{Result, TaskQuery};

/// `TaskPages` over the real client that counts the pages it reads.
pub struct Counting {
    inner: Arc<RpcTasks>,
    firsts: AtomicUsize,
    nexts: AtomicUsize,
}

impl Counting {
    pub fn over(inner: Arc<RpcTasks>) -> Self {
        Self {
            inner,
            firsts: AtomicUsize::new(0),
            nexts: AtomicUsize::new(0),
        }
    }

    /// (first pages read, continued pages read) so far.
    pub fn reads(&self) -> (usize, usize) {
        (
            self.firsts.load(Ordering::SeqCst),
            self.nexts.load(Ordering::SeqCst),
        )
    }
}

#[async_trait::async_trait]
impl TaskPages for Counting {
    async fn first(&self, query: &TaskQuery) -> Result<TaskPage> {
        self.firsts.fetch_add(1, Ordering::SeqCst);
        self.inner.first(query).await
    }

    async fn next(&self, query: &TaskQuery, cursor: &Cursor) -> Result<TaskPage> {
        self.nexts.fetch_add(1, Ordering::SeqCst);
        self.inner.next(query, cursor).await
    }
}
