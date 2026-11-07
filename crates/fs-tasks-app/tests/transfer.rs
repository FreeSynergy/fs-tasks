//! iCalendar in and out, through the real core. What is sensitive leaves only when named.
//!
//! Round-trip tests.

mod common;

use common::{Scratch, World};
use fs_tasks_app::{export_all, import_file};
use fs_tasks_core::{Task, TaskItems};
use serde_json::json;

const ICS: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VEVENT\r\nUID:ev-1\r\nDTSTAMP:20261101T000000Z\r\nDTSTART:20261130T090000Z\r\nDURATION:PT2H\r\nSUMMARY:An event\r\nEND:VEVENT\r\nBEGIN:VTODO\r\nUID:todo-1\r\nDTSTAMP:20261101T000000Z\r\nDUE:20261112T170000Z\r\nSUMMARY:Minutes\r\nSTATUS:NEEDS-ACTION\r\nEND:VTODO\r\nBEGIN:VTODO\r\nUID:todo-2\r\nDTSTAMP:20261101T000000Z\r\nDUE;TZID=Europe/Berlin:20261120T120000\r\nSUMMARY:Invoice\r\nSTATUS:COMPLETED\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";

fn marked(w: &World, uid: &str) -> String {
    w.add_json(
        json!({"@type": "Task", "version": "2.0", "uid": uid, "title": "Clinic",
        "freesynergy.net:sensitive": ["pd:HealthRecord"]}),
    )
}

#[test]
fn an_import_brings_the_tasks_in_and_a_second_one_replaces_nothing() {
    let w = World::new();
    let dir = Scratch::new();
    let file = dir.file("in.ics");
    std::fs::write(&file, ICS).unwrap();
    let handle = w.runtime.handle().clone();
    let first = import_file(w.tasks.as_ref(), &handle, &file).unwrap();
    assert_eq!((first.created, first.skipped), (2, 0));
    assert!(first.is_clean());
    let rows = w.state();
    assert_eq!(rows.rows().len(), 2);
    assert_eq!(rows.rows()[1].column, "completed");
    let again = import_file(w.tasks.as_ref(), &handle, &file).unwrap();
    assert_eq!((again.created, again.skipped), (0, 2));
}

#[test]
fn a_missing_file_and_a_file_without_a_calendar_are_errors() {
    let w = World::new();
    let dir = Scratch::new();
    let handle = w.runtime.handle().clone();
    assert!(import_file(w.tasks.as_ref(), &handle, &dir.file("none.ics")).is_err());
    let file = dir.file("text.ics");
    std::fs::write(&file, "hello").unwrap();
    assert!(import_file(w.tasks.as_ref(), &handle, &file).is_err());
}

#[test]
fn an_export_leaves_out_what_is_sensitive_names_it_and_can_be_read_back() {
    let w = World::new();
    let dir = Scratch::new();
    w.add(Task::new("Plain"));
    marked(&w, "clinic");
    let file = dir.file("out.ics");
    let handle = w.runtime.handle().clone();
    let report = export_all(w.tasks.as_ref(), &handle, &file, &[]).unwrap();
    assert_eq!(report.written, 1);
    assert_eq!(report.left_out, ["clinic"]);
    let said = report.message(&file);
    assert!(
        said.contains("clinic") && !said.contains("Clinic"),
        "the uid, not the title: {said}"
    );
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("Plain") && !text.contains("Clinic"));
    let other = World::new();
    let back = other
        .runtime
        .block_on(other.tasks.import_ics(&text))
        .unwrap();
    assert_eq!(back.created.len(), 1);
}

#[test]
fn a_sensitive_task_named_by_uid_is_released_and_only_that_one() {
    let w = World::new();
    let dir = Scratch::new();
    marked(&w, "clinic");
    marked(&w, "court");
    let file = dir.file("out.ics");
    let handle = w.runtime.handle().clone();
    let report = export_all(w.tasks.as_ref(), &handle, &file, &["clinic".into()]).unwrap();
    assert_eq!(report.written, 1);
    assert_eq!(report.released, ["clinic"]);
    assert_eq!(report.left_out, ["court"]);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "for its owner alone");
    }
}

#[test]
fn naming_a_task_that_does_not_exist_writes_nothing() {
    let w = World::new();
    let dir = Scratch::new();
    w.add(Task::new("Plain"));
    let file = dir.file("out.ics");
    let handle = w.runtime.handle().clone();
    assert!(export_all(w.tasks.as_ref(), &handle, &file, &["typo".into()]).is_err());
    assert!(dir.names().is_empty());
}

#[test]
fn an_existing_file_is_not_replaced() {
    let w = World::new();
    let dir = Scratch::new();
    w.add(Task::new("Plain"));
    let file = dir.file("out.ics");
    std::fs::write(&file, "mine").unwrap();
    let handle = w.runtime.handle().clone();
    assert!(export_all(w.tasks.as_ref(), &handle, &file, &[]).is_err());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "mine");
    assert_eq!(dir.names(), ["out.ics"]);
}

#[test]
fn the_same_uuid_in_another_form_overwrites_nothing_on_create_and_on_import() {
    const UUID: &str = "019a0000-0000-7000-8000-000000000001";
    let w = World::new();
    let plain = Task::from_value(json!({"@type": "Task", "version": "2.0",
        "uid": format!("urn:uuid:{UUID}"), "title": "First"}))
    .unwrap();
    let key = w.add(plain);
    let second = Task::from_value(json!({"@type": "Task", "version": "2.0",
        "uid": UUID, "title": "Second"}))
    .unwrap();
    assert!(w.runtime.block_on(w.tasks.create(second)).is_err());
    assert_eq!(w.json(&key)["title"], "First");

    let dir = Scratch::new();
    let file = dir.file("in.ics");
    let ics = format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VTODO\r\nUID:{UUID}\r\nDTSTAMP:20261101T000000Z\r\nSUMMARY:Second\r\nEND:VTODO\r\nBEGIN:VTODO\r\nUID:later\r\nDTSTAMP:20261101T000000Z\r\nSUMMARY:Later\r\nEND:VTODO\r\nEND:VCALENDAR\r\n"
    );
    std::fs::write(&file, ics).unwrap();
    let handle = w.runtime.handle().clone();
    let summary = import_file(w.tasks.as_ref(), &handle, &file).unwrap();
    eprintln!("the core's answer: {}", summary.message());
    assert!(!summary.is_clean(), "said, not hidden");
    assert_eq!(w.json(&key)["title"], "First", "not overwritten");
}

#[test]
fn one_unreadable_stored_task_is_said_plainly_and_nothing_is_written() {
    // Not yet what it should be: `TaskItems::list` answers all or nothing, and the contract offers no
    // other way to go through the tasks, so the adapter cannot keep the rest (with Thread 3).
    let w = World::new();
    w.add(Task::new("Good"));
    let mut envelope = fs_store::Envelope::new(fs_tasks_core::TASK_TYPE, "Bad");
    envelope.context = Some(fs_tasks_core::CONTEXT.into());
    let bad = fs_store::StoredObject::new(envelope, fs_tasks_core::CONTENT_TYPE, json!({"uid": 5}));
    w.runtime.block_on(w.store.put(bad, common::ME)).unwrap();
    let s = w.state();
    assert!(s.list_problem().is_some(), "not an empty list");
    let dir = Scratch::new();
    let handle = w.runtime.handle().clone();
    assert!(export_all(w.tasks.as_ref(), &handle, &dir.file("out.ics"), &[]).is_err());
    assert!(dir.names().is_empty());
}
