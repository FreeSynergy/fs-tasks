//! iCalendar `VTODO` import / export, original keeping (ADR 0030).

mod common;

use common::{ICS, repo, setup};
use fs_tasks_core::{TaskItems, ical};

#[test]
fn ics_is_split_per_uid_with_exact_text() {
    let parts = ical::split_ics(ICS);
    assert_eq!(
        parts.iter().map(|p| p.uid.as_str()).collect::<Vec<_>>(),
        ["todo-1", "todo-2"],
        "events belong to fs-calendar"
    );
    for p in &parts {
        assert!(
            p.text
                .starts_with("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VTODO"),
            "{}",
            p.text
        );
        assert!(p.text.ends_with("END:VCALENDAR\r\n"));
        assert_eq!(p.text.matches("BEGIN:VTODO").count(), 1);
        assert!(!p.text.contains("VEVENT"));
        assert!(
            ICS.contains(p.text.lines().nth(4).unwrap()),
            "lines are copied unchanged"
        );
    }
}

#[tokio::test]
async fn ics_import_keeps_originals_and_skips_known() {
    let (r, store) = setup().await;
    let rep = r.import_ics(ICS).await.unwrap();
    assert_eq!((rep.created.len(), rep.failed.len()), (2, 0), "{rep:?}");
    let first = r.get(rep.created[0]).await.unwrap().unwrap();
    assert_eq!(first.task.uid, "todo-1");
    assert_eq!(first.task.percent_complete, Some(10));
    assert_eq!(first.task.progress.as_deref(), Some("needs-action"));
    let stored = store.get(first.id).await.unwrap().unwrap();
    assert_eq!(stored.index["due_utc"], ["2026-11-12T17:00:00Z"]);
    assert!(stored.index.contains_key("tzdb_version"));
    let orig = stored.original.unwrap();
    assert_eq!(orig.media_type, "text/calendar");
    assert!(
        String::from_utf8(orig.bytes)
            .unwrap()
            .contains("UID:todo-1")
    );
    let second = r.get(rep.created[1]).await.unwrap().unwrap();
    assert_eq!(second.task.progress.as_deref(), Some("completed"));
    assert_eq!(second.task.time_zone.as_deref(), Some("Europe/Berlin"));
    assert_eq!(second.task.title.as_deref(), Some("Invoice, paid"));
    // re-import skips
    let again = r.import_ics(ICS).await.unwrap();
    assert_eq!((again.created.len(), again.skipped.len()), (0, 2));
    // garbage: no panic, imported or reported
    let bad = r
        .import_ics("BEGIN:VCALENDAR\r\nBEGIN:VTODO\r\nUID:x\r\nEND:VTODO\r\nEND:VCALENDAR\r\n")
        .await
        .unwrap();
    assert_eq!(bad.created.len() + bad.failed.len(), 1);
}

#[tokio::test]
async fn ics_export_roundtrip() {
    let r = repo().await;
    let rep = r.import_ics(ICS).await.unwrap();
    let out = r.export_ics(&rep.created).await.unwrap();
    for needle in [
        "BEGIN:VCALENDAR",
        "BEGIN:VTODO",
        "UID:todo-1",
        "SUMMARY:Minutes",
        "DUE:20261112T170000Z",
        "PERCENT-COMPLETE:10",
        "DUE;TZID=Europe/Berlin:20261120T120000",
    ] {
        assert!(out.contains(needle), "{needle} in\n{out}");
    }
    assert!(!out.contains("VEVENT"));
    let r2 = repo().await;
    let rep2 = r2.import_ics(&out).await.unwrap();
    assert_eq!(rep2.created.len(), 2, "{rep2:?}\n{out}");
    let a = r.get(rep.created[1]).await.unwrap().unwrap().task;
    let b = r2.get_by_uid("todo-2").await.unwrap().unwrap().task;
    assert_eq!(
        (a.due, a.time_zone, a.progress),
        (b.due, b.time_zone, b.progress)
    );
    // a native task exports too, with its assignee
    let id = r
        .create(fs_tasks_core::Task::new("Native"))
        .await
        .unwrap()
        .id;
    let e = r.assign(id, "Ben", "ben@example.org").await.unwrap();
    let out = r.export_ics(&[e.id]).await.unwrap();
    assert!(
        out.contains("SUMMARY:Native") && out.contains("mailto:ben@example.org"),
        "{out}"
    );
}

/// RFC 9253 names in capitals, as the import writes them, are relations like our own lower case ones; the direction
/// (the holder is the predecessor) and the text survive the way back.
#[test]
fn imported_rfc_9253_relations_are_seen_and_keep_their_direction() {
    use fs_tasks_core::Relation;
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VTODO\r\nUID:task-a\r\nDTSTAMP:20261101T000000Z\r\nSUMMARY:paint the room\r\nRELATED-TO;RELTYPE=FINISHTOSTART;GAP=P1D:task-b\r\nRELATED-TO;RELTYPE=DEPENDS-ON:task-c\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";
    let (tasks, _) = ical::parse_ics(ics).unwrap();
    assert_eq!(
        tasks[0].relations(),
        [
            ("task-b".to_string(), Relation::FinishToStart),
            ("task-c".to_string(), Relation::DependsOn)
        ]
    );
    let out = ical::to_ics(&tasks).unwrap();
    assert!(
        out.contains("RELATED-TO;RELTYPE=FINISHTOSTART;GAP=P1D:task-b\r\n"),
        "{out}"
    );
    assert!(
        out.contains("RELATED-TO;RELTYPE=DEPENDS-ON:task-c\r\n"),
        "{out}"
    );
    let (again, _) = ical::parse_ics(&out).unwrap();
    assert_eq!(again[0].relations(), tasks[0].relations());
}
