//! The board and the timeline over the real core: what they show, what a move does, what is locked.
//!
//! Pattern: none (tests).
#![allow(clippy::pedantic)]

use std::sync::Arc;

use fs_admin_frontend::Backend;
use fs_components_fields::KeysAsText;
use fs_components_kanban::{Answer, BoardEvents, BoardSource, Operation};
use fs_components_timeline::{Kind, TimelineSource};
use fs_objects::{Store, TursoStore};
use fs_tasks_app::{TaskBackend, TaskBoard, TaskTimeline};
use fs_tasks_core::{Relation, Task, TaskItems, TaskRepo};
use tokio::runtime::Runtime;

struct World {
    runtime: Runtime,
    repo: Arc<TaskRepo>,
    board: TaskBoard,
    timeline: TaskTimeline,
}

fn world() -> World {
    let runtime = Runtime::new().unwrap();
    let store: Arc<dyn Store> = Arc::new(runtime.block_on(TursoStore::open_in_memory()).unwrap());
    let repo = Arc::new(TaskRepo::new(store, "https://local.invalid/users/owner"));
    let items: Arc<dyn TaskItems> = repo.clone();
    let backend: Arc<dyn Backend> =
        Arc::new(TaskBackend::new(items.clone(), runtime.handle().clone()));
    let board = TaskBoard::new(
        items.clone(),
        runtime.handle().clone(),
        backend,
        Arc::new(KeysAsText),
    );
    let timeline = TaskTimeline::new(items, runtime.handle().clone());
    World {
        runtime,
        repo,
        board,
        timeline,
    }
}

fn add(w: &World, json: serde_json::Value) -> fs_objects::ObjectId {
    w.runtime
        .block_on(w.repo.create(Task::from_value(json).unwrap()))
        .unwrap()
        .id
}

fn task(uid: &str, title: &str, extra: serde_json::Value) -> serde_json::Value {
    let mut v = serde_json::json!({"@type":"Task","version":"2.0","uid":uid,"title":title});
    v.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    v
}

#[test]
fn the_board_has_a_column_per_progress_and_a_card_per_task() {
    let w = world();
    add(
        &w,
        task(
            "urn:uuid:a",
            "Open",
            serde_json::json!({"keywords":{"work":true}}),
        ),
    );
    add(
        &w,
        task(
            "urn:uuid:b",
            "Done",
            serde_json::json!({"progress":"completed","due":"2020-01-01T10:00:00"}),
        ),
    );
    let columns = w.board.columns();
    let keys: Vec<&str> = columns.iter().map(|c| c.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "needs-action",
            "in-process",
            "completed",
            "failed",
            "cancelled"
        ]
    );
    assert_eq!(columns[0].cards.len(), 1);
    assert_eq!(columns[0].cards[0].labels[0].text, "work");
    assert_eq!(columns[2].cards[0].title, "Done");
    assert_eq!(columns[2].cards[0].due.as_ref().unwrap().text, "2020-01-01");
    assert_eq!(
        columns[2].cards[0].due.as_ref().unwrap().state,
        fs_components_kanban::DueState::Done
    );
}

#[test]
fn a_move_changes_progress_and_only_that() {
    let w = world();
    let id = add(
        &w,
        task(
            "urn:uuid:a",
            "Move me",
            serde_json::json!({"description":"stay"}),
        ),
    );
    let key = fs_tasks_app::record_id(id);
    assert_eq!(
        w.board.move_card(&key, "needs-action", "in-process"),
        Answer::Accepted
    );
    let after = w.runtime.block_on(w.repo.get(id)).unwrap().unwrap().task;
    assert_eq!(after.progress.as_deref(), Some("in-process"));
    assert_eq!(after.description.as_deref(), Some("stay"));
    assert_eq!(w.board.columns()[1].cards.len(), 1);
    // a task that is gone is refused with a code, nothing else happens
    let gone = "urn:uuid:00000000-0000-7000-8000-000000000000";
    assert_eq!(
        w.board.move_card(gone, "needs-action", "failed"),
        Answer::Refused("gone".into())
    );
    assert_eq!(
        w.board.move_card(&key, "in-process", "in-process"),
        Answer::Accepted
    );
}

#[test]
fn moves_are_supported_and_the_column_operations_are_locked() {
    let w = world();
    assert!(w.board.supports(Operation::Move));
    for op in [Operation::AddColumn, Operation::Rename, Operation::Limit] {
        assert!(!w.board.supports(op), "{op:?}");
    }
}

#[test]
fn the_timeline_has_bars_milestones_and_dependency_arrows() {
    let w = world();
    let a = add(
        &w,
        task(
            "urn:uuid:a",
            "First",
            serde_json::json!({"start":"2026-12-01T09:00:00","due":"2026-12-03T17:00:00","percentComplete":40}),
        ),
    );
    let mut second = Task::from_value(task(
        "urn:uuid:b",
        "Second",
        serde_json::json!({"due":"2026-12-05T17:00:00"}),
    ))
    .unwrap();
    second.relate("urn:uuid:a", Relation::DependsOn);
    let b = w.runtime.block_on(w.repo.create(second)).unwrap().id;
    add(&w, task("urn:uuid:c", "No dates", serde_json::json!({})));
    let items = w.timeline.items();
    assert_eq!(
        items.len(),
        2,
        "a task without a date has no place on the axis"
    );
    let first = items.iter().find(|i| i.title == "First").unwrap();
    assert!(matches!(first.kind, Kind::Bar { progress: 40, .. }));
    assert!(matches!(
        items.iter().find(|i| i.title == "Second").unwrap().kind,
        Kind::Milestone { .. }
    ));
    let deps = w.timeline.dependencies();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].from, fs_tasks_app::record_id(a));
    assert_eq!(deps[0].to, fs_tasks_app::record_id(b));
}
