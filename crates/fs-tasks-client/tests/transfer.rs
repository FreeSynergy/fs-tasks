//! iCalendar in and out through the server.
//!
//! Pattern: Integration tests against the trait; no stand-ins.

mod common;

use common::{Served, task};
use fs_tasks_core::{Error, TaskItems};

const ICS: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VTODO\r\nUID:todo-1\r\nDTSTAMP:20261101T000000Z\r\nDUE:20261112T170000Z\r\nSUMMARY:Minutes\r\nSTATUS:NEEDS-ACTION\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";

#[tokio::test(flavor = "multi_thread")]
async fn an_import_reports_what_it_created_and_skipped() {
    let served = Served::start().await;
    let tasks = served.client();
    let first = tasks.import_ics(ICS).await.unwrap();
    assert_eq!(first.created.len(), 1);
    assert!(first.skipped.is_empty() && first.failed.is_empty());
    let again = tasks.import_ics(ICS).await.unwrap();
    assert!(again.created.is_empty());
    assert_eq!(again.skipped, ["todo-1"]);
    let listed = tasks.list().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, first.created[0]);
    assert_eq!(listed[0].task.title.as_deref(), Some("Minutes"));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_report_equals_the_providers_own() {
    let served = Served::start().await;
    let other = Served::start().await;
    let wire = served.client().import_ics(ICS).await.unwrap();
    let core = other.repo.import_ics(ICS).await.unwrap();
    assert_eq!(wire.created.len(), core.created.len());
    assert_eq!(wire.skipped, core.skipped);
    assert_eq!(wire.failed, core.failed);
    assert_eq!(wire.notes, core.notes);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_export_is_the_text_of_the_server() {
    let served = Served::start().await;
    let tasks = served.client();
    let a = tasks.create(task("a", "Alpha")).await.unwrap();
    let b = tasks.create(task("b", "Beta")).await.unwrap();
    let text = tasks.export_ics(&[a.id, b.id]).await.unwrap();
    assert_eq!(text, served.repo.export_ics(&[a.id, b.id]).await.unwrap());
    assert!(text.contains("SUMMARY:Alpha") && text.contains("SUMMARY:Beta"));
    let only_b = tasks.export_ics(&[b.id]).await.unwrap();
    assert!(!only_b.contains("Alpha"));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_export_of_an_unknown_task_is_not_found() {
    let served = Served::start().await;
    let tasks = served.client();
    let got = tasks.export_ics(&[fs_store::ObjectId::new()]).await;
    assert!(
        matches!(got, Err(Error::NotFound(_)) | Err(Error::Store(_))),
        "{got:?}"
    );
}
