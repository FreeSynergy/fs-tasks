//! iCalendar in and out through the server.
//!
//! Integration tests against the trait; nothing stands in for the server or the store.

mod common;

use common::{Served, task};
use fs_tasks_core::{Error, TaskItems};

const ICS: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VTODO\r\nUID:todo-1\r\nDTSTAMP:20261101T000000Z\r\nDUE:20261112T170000Z\r\nSUMMARY:Minutes\r\nSTATUS:NEEDS-ACTION\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";

/// Each export names its calendar with a fresh `UID`; the lines of the tasks are what is compared.
fn without_calendar_uid(text: &str) -> String {
    text.replacen(
        &text[text.find("UID:urn:uuid:").unwrap()..text.find("PRODID").unwrap()],
        "",
        1,
    )
}

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
    let at_server = served.repo.export_ics(&[a.id, b.id]).await.unwrap();
    assert_eq!(
        without_calendar_uid(&text),
        without_calendar_uid(&at_server)
    );
    assert!(text.contains("SUMMARY:Alpha") && text.contains("SUMMARY:Beta"));
    let only_b = tasks.export_ics(&[b.id]).await.unwrap();
    assert!(!only_b.contains("Alpha"));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_export_of_an_unknown_task_is_not_found() {
    let served = Served::start().await;
    let tasks = served.client();
    let got = tasks.export_ics(&[fs_objects::ObjectId::new()]).await;
    assert!(matches!(got, Err(Error::NotFound(_))), "{got:?}");
}
