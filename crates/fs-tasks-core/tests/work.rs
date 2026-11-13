//! Work-board members: the origin an import writes, and the bounds of the short members.

use std::sync::Arc;

use fs_objects::{Store, TursoStore};
use fs_tasks_core::model::{
    CHECK_KEY, CHECKED_KEY, NOTE_KEY, ORIGIN_IMPORTED, ORIGIN_KEY, ORIGIN_LOCAL,
};
use fs_tasks_core::{Task, TaskItems, TaskRepo};
use serde_json::json;

async fn repo() -> TaskRepo {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    TaskRepo::new(store, "https://local.invalid/users/owner")
}

#[tokio::test]
async fn an_import_is_marked_imported_and_a_task_made_over_the_wire_keeps_what_it_says() {
    let repo = repo().await;
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VTODO\r\nUID:t1\r\n\
DTSTAMP:20261101T000000Z\r\nSUMMARY:In\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";
    let report = repo.import_ics(ics).await.unwrap();
    assert_eq!(report.created.len(), 1);
    let task = repo.get(report.created[0]).await.unwrap().unwrap().task;
    assert_eq!(task.origin(), Some(ORIGIN_IMPORTED));

    // Only the import path rewrites the origin; the wire is the local user's own door (same socket owner), so what
    // a created task says stays. (Sync, when it comes, must rewrite it as the import does.)
    let mut claiming = Task::new("x");
    claiming.set_vendor(ORIGIN_KEY, json!(ORIGIN_LOCAL));
    claiming.set_checked("true", "2026-11-01T00:00:00Z");
    claiming.set_vendor(CHECK_KEY, json!("true"));
    let made = repo.create(claiming).await.unwrap().task;
    assert_eq!(
        made.origin(),
        Some(ORIGIN_LOCAL),
        "a task made through the wire keeps what it says"
    );
    assert!(made.vendor(CHECKED_KEY).is_some());
}

#[tokio::test]
async fn a_note_and_a_check_command_are_at_most_500_bytes() {
    let repo = repo().await;
    for key in [NOTE_KEY, CHECK_KEY] {
        let mut t = Task::new("bounded");
        t.set_vendor(key, json!("x".repeat(500)));
        repo.create(t).await.unwrap();
        let mut t = Task::new("too long");
        t.set_vendor(key, json!("x".repeat(501)));
        let e = repo.create(t).await.unwrap_err().to_string();
        assert!(e.contains("500 bytes"), "{key}: {e}");
        let mut t = Task::new("not a text");
        t.set_vendor(key, json!(5));
        assert!(repo.create(t).await.is_err());
    }
}
