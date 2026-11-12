//! The provider rule (ADR 0045): every provider has every method; what it does not offer answers
//! an explicit "not possible", and its report says the same.

mod common;

use std::sync::Arc;

use async_trait::async_trait;
use common::repo;
use fs_interfaces::Capabilities;
use fs_objects::ObjectId;
use fs_tasks_core::{Error, Place, Result, Task, TaskEntry, TaskItems, TaskPage, TaskQuery};

/// A read-only provider, for example a task list it cannot write to.
struct ReadOnly;

#[async_trait]
impl TaskItems for ReadOnly {
    fn capabilities(&self) -> Capabilities {
        let mut c = Capabilities::new("read-only");
        c.supported("task.read");
        c.unsupported("task.create", "this source cannot be written");
        c
    }
    async fn get(&self, _: ObjectId) -> Result<Option<TaskEntry>> {
        Ok(None)
    }
    async fn get_by_uid(&self, _: &str) -> Result<Option<TaskEntry>> {
        Ok(None)
    }
    async fn list(&self) -> Result<Vec<TaskEntry>> {
        Ok(vec![])
    }
    async fn query(&self, _: &TaskQuery) -> Result<Vec<TaskEntry>> {
        Ok(vec![])
    }
    async fn page(&self, _: &TaskQuery, _: Option<&Place>, _: usize) -> Result<TaskPage> {
        Ok(TaskPage::default())
    }
}

#[tokio::test]
async fn a_missing_method_says_not_possible() {
    let p: Arc<dyn TaskItems> = Arc::new(ReadOnly);
    assert_eq!(p.list().await.unwrap(), [] as [fs_tasks_core::TaskEntry; 0]);
    let Err(Error::Unsupported(u)) = p.create(Task::new("x")).await else {
        panic!("expected an explicit not possible");
    };
    assert_eq!(u.capability(), "task.create");
    assert_eq!(u.status(), 501);
    assert!(!p.capabilities().is_supported("task.create"));
    for r in [
        p.delete(ObjectId::new()).await,
        p.import_ics("").await.map(|_| ()),
        p.export_ics(&[]).await.map(|_| ()),
        p.label(ObjectId::new(), "x").await.map(|_| ()),
    ] {
        assert!(matches!(r, Err(Error::Unsupported(_))));
    }
}

#[tokio::test]
async fn the_store_backed_provider_reports_what_it_can() {
    let p: Arc<dyn TaskItems> = Arc::new(repo().await);
    let caps = p.capabilities();
    for id in [
        "task.create",
        "relation.create",
        "task.import",
        "task.subscribe",
    ] {
        assert!(caps.is_supported(id), "{id}");
    }
    // boards and claims are offered by the provider itself (the wire does not carry them yet)
    for id in [
        "card.move",
        "board.open",
        "task.claim",
        "dependency.evaluate",
    ] {
        assert!(caps.is_supported(id), "{id}");
    }
}
