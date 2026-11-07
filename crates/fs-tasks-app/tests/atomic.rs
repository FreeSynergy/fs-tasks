//! Saving and moving are compare-and-set: a change that lands between the adapter's read and its
//! write is not overwritten. The server, store and client are the real ones; only
//! `RevisedTasks::get_revised` is decorated, to let someone else write at the moment between read and
//! write. The control run has the same decorator with nobody interfering and must save.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use common::World;
use fs_components_workbenches::tasks::{TaskDraft, TaskSource};
use fs_store::ObjectId;
use fs_tasks_app::TaskAdapter;
use fs_tasks_client::{Revised, RevisedTasks, RpcTasks};
use fs_tasks_core::{Result, TaskItems};

/// The real client whose `get_revised` lets another writer in right after it has read.
struct Interfering {
    inner: Arc<RpcTasks>,
    active: AtomicBool,
}

#[async_trait]
impl RevisedTasks for Interfering {
    async fn get_revised(&self, id: ObjectId) -> Result<Option<Revised>> {
        let read = self.inner.get_revised(id).await?;
        if let (Some(read), true) = (&read, self.active.load(Ordering::SeqCst)) {
            let mut theirs = read.entry.task.clone();
            theirs.description = Some("theirs".into());
            self.inner.edit(id, theirs).await?;
        }
        Ok(read)
    }
    async fn list_revised(&self) -> Result<Vec<Revised>> {
        self.inner.list_revised().await
    }
}

fn adapter(w: &World, active: bool) -> TaskAdapter {
    let revised = Arc::new(Interfering {
        inner: w.tasks.clone(),
        active: AtomicBool::new(active),
    });
    TaskAdapter::new(w.tasks.clone(), revised, w.runtime.handle().clone())
}

fn retitled(draft: &mut TaskDraft) {
    let title = draft.fields.iter_mut().find(|f| f.key == "title").unwrap();
    title.value = "Mine".into();
}

fn run<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|s| s.spawn(f).join().unwrap())
}

#[test]
fn a_write_between_read_and_save_is_a_conflict_and_nothing_is_lost() {
    let w = World::new();
    let key = w.plain("Ada");
    let source = adapter(&w, true);
    let mut draft = w.adapter.draft(&key).unwrap();
    retitled(&mut draft);
    let error = run(|| source.save(&key, &draft)).expect_err("their write came in between");
    assert!(error.message.contains("Someone else"), "{}", error.message);
    let stored = w.json(&key);
    assert_eq!(stored["title"], "Ada", "my change was not written");
    assert_eq!(
        stored["description"], "theirs",
        "their change is still there"
    );
}

#[test]
fn control_without_the_second_writer_the_same_save_is_written() {
    let w = World::new();
    let key = w.plain("Ada");
    let source = adapter(&w, false);
    let mut draft = w.adapter.draft(&key).unwrap();
    retitled(&mut draft);
    run(|| source.save(&key, &draft)).unwrap();
    assert_eq!(w.json(&key)["title"], "Mine");
}

#[test]
fn a_write_between_read_and_move_is_a_conflict_and_nothing_is_lost() {
    let w = World::new();
    let key = w.plain("Ada");
    let source = adapter(&w, true);
    let revision = w.adapter.read(&key).unwrap().revision;
    let error = run(|| source.move_to(&key, "in-process", revision.as_deref()))
        .expect_err("their write came in between");
    assert!(error.message.contains("Someone else"), "{}", error.message);
    let stored = w.json(&key);
    assert_eq!(stored["description"], "theirs");
    assert!(stored.get("progress").is_none() || stored["progress"] == "needs-action");
}

#[test]
fn control_without_the_second_writer_the_same_move_is_written() {
    let w = World::new();
    let key = w.plain("Ada");
    let source = adapter(&w, false);
    let revision = w.adapter.read(&key).unwrap().revision;
    run(|| source.move_to(&key, "in-process", revision.as_deref())).unwrap();
    assert_eq!(w.json(&key)["progress"], "in-process");
}

#[test]
fn a_task_changed_since_the_draft_is_a_conflict_before_the_form_is_applied() {
    let w = World::new();
    let key = w.plain("Ada");
    let mut draft = w.adapter.draft(&key).unwrap();
    let title = draft.fields.iter_mut().find(|f| f.key == "title").unwrap();
    title.value = String::new();
    let id: ObjectId = key.parse().unwrap();
    let mut theirs = w.json(&key);
    theirs["description"] = "theirs".into();
    let theirs = fs_tasks_core::Task::from_value(theirs).unwrap();
    w.runtime.block_on(w.tasks.edit(id, theirs)).unwrap();
    let error = run(|| w.adapter.save(&key, &draft)).expect_err("the task has changed");
    assert!(error.message.contains("Someone else"), "{}", error.message);
}
