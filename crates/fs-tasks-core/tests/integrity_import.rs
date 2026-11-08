//! An import neither overwrites nor loses anything without saying so.

mod common;

use common::repo;
use fs_tasks_core::{Task, TaskItems};

const UUID: &str = "019a0000-0000-7000-8000-000000000001";

fn task(uid: &str) -> Task {
    Task::from_value(
        serde_json::json!({"@type": "Task", "version": "2.0", "uid": uid, "title": uid}),
    )
    .unwrap()
}

fn vtodo(uid: &str) -> String {
    format!(
        "BEGIN:VTODO\r\nUID:{uid}\r\nDTSTAMP:20261101T000000Z\r\nSUMMARY:{uid}\r\nEND:VTODO\r\n"
    )
}

fn calendar(todos: &[String]) -> String {
    format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\n{}END:VCALENDAR\r\n",
        todos.concat()
    )
}

#[tokio::test]
async fn an_import_with_another_form_of_a_uuid_neither_overwrites_nor_loses_the_report() {
    let r = repo().await;
    let existing = r.create(task(&format!("urn:uuid:{UUID}"))).await.unwrap();
    for form in [UUID.to_string(), UUID.to_uppercase()] {
        let report = r
            .import_ics(&calendar(&[vtodo(&form), vtodo(&format!("other-{form}"))]))
            .await
            .unwrap();
        assert_eq!(report.failed.len(), 1, "{form}: {report:?}");
        assert!(report.failed[0].contains(&form), "{report:?}");
        assert_eq!(
            report.created.len(),
            1,
            "{form}: the other one still came in"
        );
        let kept = r.get(existing.id).await.unwrap().unwrap();
        assert_eq!(kept.task.uid, format!("urn:uuid:{UUID}"), "not overwritten");
    }
}

#[tokio::test]
async fn create_refuses_an_uppercase_form_of_a_uuid_too() {
    let r = repo().await;
    r.create(task(UUID)).await.unwrap();
    let second = r.create(task(&UUID.to_uppercase())).await;
    assert!(
        matches!(second, Err(ref e) if e.is_conflict()),
        "{second:?}"
    );
}

#[tokio::test]
async fn nothing_in_an_import_loses_the_report_or_the_value_silently() {
    let r = repo().await;
    let bad = "BEGIN:VTODO\r\nUID:bad\r\nDTSTAMP:20261101T000000Z\r\nATTENDEE:mailto:\r\nCATEGORIES:\r\nDUE:garbage\r\nDURATION:P99999999999W\r\nSUMMARY:b\r\nEND:VTODO\r\n";
    let report = r
        .import_ics(&calendar(&[vtodo("first"), bad.to_string(), vtodo("last")]))
        .await
        .unwrap();
    assert!(r.get_by_uid("first").await.unwrap().is_some());
    assert!(r.get_by_uid("last").await.unwrap().is_some(), "{report:?}");
    let said = format!("{:?}{:?}", report.failed, report.notes);
    assert!(said.contains("DUE") && said.contains("DURATION"), "{said}");
}

#[tokio::test]
async fn a_description_quoting_a_component_is_no_lost_component() {
    let r = repo().await;
    let todo = "BEGIN:VTODO\r\nUID:q\r\nDTSTAMP:20261101T000000Z\r\nDESCRIPTION:BEGIN:VTODO quoted\r\nSUMMARY:q\r\nEND:VTODO\r\n";
    let report = r.import_ics(&calendar(&[todo.to_string()])).await.unwrap();
    assert!(report.failed.is_empty(), "{report:?}");
}

#[tokio::test]
async fn an_import_says_what_it_lost() {
    let r = repo().await;
    let report = r.import_ics("this is no calendar").await.unwrap();
    assert_eq!(report.failed, ["not an iCalendar text"]);
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VTODO\r\nDTSTAMP:20261101T000000Z\r\nSUMMARY:no uid\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";
    let report = r.import_ics(ics).await.unwrap();
    assert_eq!(report.failed, ["VTODO without a UID"]);
    assert_eq!(report.created, [] as [fs_objects::ObjectId; 0]);
}
