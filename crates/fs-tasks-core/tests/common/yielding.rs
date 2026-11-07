//! The real store with the pauses a remote one has: every call waits a moment first, so two writers meet.
//!
//! Pattern: Decorator.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use fs_objects::traits::{ObjectQuery, ObjectRepository};
use fs_objects::{
    Activity, LogEntry, ObjectId, Query, Result as StoreResult, Store, StoredObject, TursoStore,
};
use fs_tasks_core::TaskRepo;

/// A backend that yields at every step, as a remote one would.
pub struct Yielding(Arc<dyn Store>);

async fn pause() {
    tokio::time::sleep(Duration::from_millis(1)).await;
}

#[async_trait]
impl ObjectRepository for Yielding {
    async fn put(&self, object: StoredObject, actor: &str) -> StoreResult<Activity> {
        pause().await;
        self.0.put(object, actor).await
    }
    async fn put_and_log(
        &self,
        object: StoredObject,
        actor: &str,
        extra: Vec<LogEntry>,
    ) -> StoreResult<Vec<Activity>> {
        pause().await;
        self.0.put_and_log(object, actor, extra).await
    }
    async fn get(&self, id: ObjectId) -> StoreResult<Option<StoredObject>> {
        pause().await;
        self.0.get(id).await
    }
    async fn delete(&self, id: ObjectId, actor: &str) -> StoreResult<Activity> {
        pause().await;
        self.0.delete(id, actor).await
    }
}

#[async_trait]
impl ObjectQuery for Yielding {
    async fn query(&self, query: &Query) -> StoreResult<Vec<StoredObject>> {
        pause().await;
        self.0.query(query).await
    }
    async fn count(&self, query: &Query) -> StoreResult<usize> {
        pause().await;
        self.0.count(query).await
    }
}

/// A repository over that backend.
pub async fn yielding_repo() -> TaskRepo {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let backend = Arc::new(Yielding(store));
    TaskRepo::with_backend(backend.clone(), backend, super::ME)
}

/// Run all of these at once and collect what they answer.
pub async fn join_all<F: Future + Send + 'static>(all: impl Iterator<Item = F>) -> Vec<F::Output>
where
    F::Output: Send + 'static,
{
    let handles: Vec<_> = all.map(tokio::spawn).collect();
    let mut out = Vec::new();
    for handle in handles {
        out.push(handle.await.unwrap());
    }
    out
}
