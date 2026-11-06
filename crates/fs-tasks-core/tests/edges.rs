//! The edges of the time index and what an import says about a value it could not read.

mod common;

use common::{repo, setup, t};
use fs_tasks_core::{Error, MAX_TASK_BYTES, Task, TaskItems, TaskQuery};
use serde_json::json;

fn due(uid: &str, due: &str, zone: &str) -> Task {
    Task::from_value(json!({
        "@type": "Task", "version": "2.0", "uid": uid, "title": uid, "due": due, "timeZone": zone,
    }))
    .unwrap()
}

fn range(from: Option<&str>, to: Option<&str>) -> TaskQuery {
    TaskQuery {
        due_from: from.map(t),
        due_to: to.map(t),
        ..TaskQuery::new()
    }
}

fn uids(found: &[fs_tasks_core::TaskEntry]) -> Vec<&str> {
    found.iter().map(|e| e.task.uid.as_str()).collect()
}

#[tokio::test]
async fn the_last_second_is_inside_an_open_range_and_second_60_is_refused() {
    let r = repo().await;
    r.create(due("last", "9999-12-31T23:59:59", "Etc/UTC"))
        .await
        .unwrap();
    let open = r
        .query(&range(Some("2026-01-01T00:00:00Z"), None))
        .await
        .unwrap();
    assert_eq!(uids(&open), ["last"]);
    let refused = r
        .create(due("leap", "9999-12-31T23:59:60", "Etc/UTC"))
        .await;
    assert!(matches!(refused, Err(Error::Invalid(_))), "{refused:?}");
    let any_minute = r
        .create(due("minute", "2026-11-24T10:15:60", "Etc/UTC"))
        .await;
    assert!(matches!(any_minute, Err(Error::Invalid(_))));
}

#[tokio::test]
async fn a_time_beyond_the_years_of_the_index_stays_in_every_range() {
    let r = repo().await;
    r.create(due("early", "0000-01-01T00:00:00", "Pacific/Auckland"))
        .await
        .unwrap();
    r.create(due("late", "9999-12-31T23:59:59", "Pacific/Honolulu"))
        .await
        .unwrap();
    let before = r
        .query(&range(None, Some("2026-01-01T00:00:00Z")))
        .await
        .unwrap();
    assert_eq!(
        uids(&before),
        ["early"],
        "below the lowest bound it fell out"
    );
    let after = r
        .query(&range(Some("2026-01-01T00:00:00Z"), None))
        .await
        .unwrap();
    assert_eq!(
        uids(&after),
        ["late"],
        "above the highest bound it fell out"
    );
    let all = r
        .query(&range(
            Some("0000-01-01T00:00:00Z"),
            Some("9999-12-31T23:59:59Z"),
        ))
        .await
        .unwrap();
    assert_eq!(
        uids(&all),
        ["early"],
        "half-open: the last second is not before itself"
    );
    let order = r.list().await.unwrap();
    assert_eq!(uids(&order), ["early", "late"]);
    let kept = &order[0].envelope.end_time.unwrap();
    assert!(
        kept.to_string().starts_with("-0001-12-31"),
        "the task keeps its real time: {kept}"
    );
}

#[tokio::test]
async fn an_end_beyond_year_9999_stays_at_the_edge_of_the_index() {
    let (r, store) = setup().await;
    let task = Task::from_value(json!({
        "@type": "Task", "version": "2.0", "uid": "u", "title": "u",
        "start": "9999-12-31T00:00:00", "estimatedDuration": "P2D",
    }))
    .unwrap();
    let id = r.create(task).await.unwrap().id;
    let stored = store.get(id).await.unwrap().unwrap();
    assert_eq!(stored.index["end_utc"], ["9999-12-31T23:59:59Z"]);
    assert_eq!(stored.index["start_utc"], ["9999-12-31T00:00:00Z"]);
    assert_eq!(
        stored.envelope.end_time.unwrap().format("%Y").to_string(),
        "+10000"
    );
}

#[tokio::test]
async fn the_name_counts_as_well_so_the_answer_always_fits() {
    let r = repo().await;
    let task = |title: String| {
        Task::from_value(json!({"@type": "Task", "version": "2.0", "uid": "u", "title": title}))
            .unwrap()
    };
    let refused = r.create(task("x".repeat(MAX_TASK_BYTES - 120))).await;
    assert!(
        matches!(refused, Err(Error::TooLarge(_))),
        "only the JSON was counted"
    );
    let half = r
        .create(task("x".repeat(MAX_TASK_BYTES / 2 - 200)))
        .await
        .unwrap();
    let json = half.task.to_value().to_string().len();
    assert!(json + half.envelope.name.len() <= MAX_TASK_BYTES);
}

const OWN: &str = "UID:a\r\nDTSTAMP:20261101T000000Z\r\nSUMMARY:s\r\n";

async fn notes_of(body: &str) -> Vec<String> {
    let r = repo().await;
    let ics = format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:x\r\nBEGIN:VTODO\r\n{OWN}{body}END:VTODO\r\nEND:VCALENDAR\r\n"
    );
    let report = r.import_ics(&ics).await.unwrap();
    assert_eq!(report.created.len(), 1, "{report:?}");
    report
        .notes
        .into_iter()
        .filter(|n| n.contains("could not be read"))
        .collect()
}

#[tokio::test]
async fn garbage_in_any_of_these_properties_is_named() {
    for name in [
        "DTSTART",
        "DUE",
        "DURATION",
        "COMPLETED",
        "PERCENT-COMPLETE",
        "RRULE",
    ] {
        let notes = notes_of(&format!("{name}:garbage\r\n")).await;
        assert_eq!(notes, [format!("a: {name} could not be read")]);
    }
    let folded = notes_of("DUE:gar\r\n bage\r\n").await;
    assert_eq!(folded, ["a: DUE could not be read"], "a folded value too");
    let split_name = notes_of("DU\r\n E:garbage\r\n").await;
    assert_eq!(
        split_name,
        ["a: DUE could not be read"],
        "a folded name too"
    );
}

#[tokio::test]
async fn good_values_raise_no_alarm() {
    let all = "DTSTART:20261101T100000Z\r\nDUE:20261102T100000Z\r\nDURATION:PT1H\r\nCOMPLETED:20261103T100000Z\r\nPERCENT-COMPLETE:40\r\nRRULE:FREQ=DAILY;COUNT=3\r\n";
    assert_eq!(notes_of(all).await, Vec::<String>::new());
}

#[tokio::test]
async fn only_the_lines_of_the_vtodo_itself_count() {
    let alarm = "BEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT15M\r\nDURATION:PT5M\r\nREPEAT:2\r\nEND:VALARM\r\n";
    assert_eq!(
        notes_of(alarm).await,
        Vec::<String>::new(),
        "an alarm's DURATION"
    );
    let quoted = "DESCRIPTION:see\r\n DUE:garbage\r\n";
    assert_eq!(
        notes_of(quoted).await,
        Vec::<String>::new(),
        "a description's words"
    );

    let r = repo().await;
    let zone = "BEGIN:VTIMEZONE\r\nTZID:Europe/Berlin\r\nBEGIN:STANDARD\r\nDTSTART:19701025T030000\r\nTZOFFSETFROM:+0200\r\nTZOFFSETTO:+0100\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\n";
    let ics = format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:x\r\n{zone}BEGIN:VTODO\r\n{OWN}END:VTODO\r\nEND:VCALENDAR\r\n"
    );
    let report = r.import_ics(&ics).await.unwrap();
    assert!(
        !format!("{:?}", report.notes).contains("could not be read"),
        "{report:?}"
    );
}

#[tokio::test]
async fn words_on_a_continuation_line_are_no_lost_component() {
    let r = repo().await;
    let ics = format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:x\r\nBEGIN:VEVENT\r\nUID:e\r\nDTSTAMP:20261101T000000Z\r\nDTSTART:20261101T100000Z\r\nDESCRIPTION:see\r\n BEGIN:VTODO\r\nEND:VEVENT\r\nBEGIN:VTODO\r\n{OWN}DESCRIPTION:a\r\n END:VTODO\r\n b\r\nEND:VTODO\r\nEND:VCALENDAR\r\n"
    );
    let report = r.import_ics(&ics).await.unwrap();
    assert!(report.failed.is_empty(), "{report:?}");
    let task = r.get_by_uid("a").await.unwrap().unwrap().task;
    assert!(
        task.description.as_deref().unwrap().contains("END:VTODO"),
        "{task:?}"
    );
}
