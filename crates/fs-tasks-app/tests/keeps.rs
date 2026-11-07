//! An edit keeps everything else on the task: the whole task compared as JSON around it.
//!
//! Pattern: Round-trip tests through the real form, the real adapter and the real core.

mod common;

use common::{ME, World, rich, series, without};
use fs_components_workbenches::ErrorKind;
use fs_components_workbenches::tasks::{TaskSource, TasksState};
use fs_store::Blob;
use fs_tasks_app::apply_draft;
use fs_tasks_core::Task;
use serde_json::json;

fn edit(state: &mut TasksState, key: &str, fields: &[(&str, &str)]) {
    state.select(key);
    state.begin_edit();
    for (field, value) in fields {
        state.set_field(field, value);
    }
    state.save();
    assert!(state.editing().is_none(), "{:?}", state.editing());
}

#[test]
fn changing_the_title_changes_the_title_and_nothing_else() {
    let w = World::new();
    let key = w.add_json(rich());
    let before = w.json(&key);
    edit(&mut w.state(), &key, &[("title", "New title")]);
    let after = w.json(&key);
    assert_eq!(after["title"], "New title");
    assert_eq!(without(after, &["title"]), without(before, &["title"]));
}

#[test]
fn every_field_of_the_form_changes_only_itself() {
    let cases: [(&str, &str, &str, serde_json::Value); 3] = [
        ("description", "Other", "description", json!("Other")),
        ("priority", "7", "priority", json!(7)),
        (
            "due",
            "2026-12-24 12:30",
            "due",
            json!("2026-12-24T12:30:00"),
        ),
    ];
    for (field, typed, member, wanted) in cases {
        let w = World::new();
        let key = w.add_json(rich());
        let before = w.json(&key);
        edit(&mut w.state(), &key, &[(field, typed)]);
        let after = w.json(&key);
        assert_eq!(after[member], wanted, "{field}");
        // A changed due date is a change of schedule: the core counts it.
        let core_sets = if field == "due" { "sequence" } else { member };
        assert_eq!(
            without(after, &[member, core_sets]),
            without(before, &[member, core_sets]),
            "{field}"
        );
    }
}

#[test]
fn emptying_a_field_removes_only_that_value() {
    let w = World::new();
    let key = w.add_json(rich());
    let before = w.json(&key);
    edit(
        &mut w.state(),
        &key,
        &[("priority", ""), ("description", "  ")],
    );
    let after = w.json(&key);
    assert!(after.get("priority").is_none() && after.get("description").is_none());
    assert_eq!(
        without(after, &["priority", "description"]),
        without(before, &["priority", "description"])
    );
}

#[test]
fn changing_labels_keeps_what_the_form_cannot_show_and_a_label_with_a_comma_can_go() {
    let w = World::new();
    let key = w.add_json(rich());
    let mut s = w.state();
    s.select(&key);
    s.begin_edit();
    let shown = s
        .editing()
        .unwrap()
        .draft
        .value("labels")
        .unwrap()
        .to_string();
    assert_eq!(
        shown, "one, \"with,comma\"",
        "the one switched off is not shown"
    );
    s.set_field("labels", "one, \"with,comma\", two");
    s.save();
    assert_eq!(
        w.json(&key)["keywords"],
        json!({"one": true, "off": false, "with,comma": true, "two": true})
    );
    edit(&mut s, &key, &[("labels", "two")]);
    assert_eq!(
        w.json(&key)["keywords"],
        json!({"off": false, "two": true}),
        "what Saved says is true"
    );
}

#[test]
fn a_save_without_changes_writes_nothing() {
    let w = World::new();
    let key = w.add_json(rich());
    let before = w.stored(&key);
    let mut s = w.state();
    s.select(&key);
    s.begin_edit();
    s.save();
    assert_eq!(s.notice(), Some(s.labels().no_changes.as_str()));
    let after = w.stored(&key);
    assert_eq!(after.content, before.content);
    assert_eq!(after.envelope.updated, before.envelope.updated);
}

#[test]
fn the_original_icalendar_stays_byte_for_byte_after_an_edit_and_a_move() {
    let w = World::new();
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VTODO\r\nUID:todo-1\r\nDTSTAMP:20261101T000000Z\r\nDUE:20261112T170000Z\r\nSUMMARY:Minutes\r\nX-OTHER:kept\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";
    let report = w
        .runtime
        .block_on(fs_tasks_core::TaskItems::import_ics(w.repo.as_ref(), ics))
        .unwrap();
    let key = report.created[0].to_string();
    let before: Blob = w.stored(&key).original.expect("the core keeps it");
    edit(&mut w.state(), &key, &[("title", "Other")]);
    let mut s = w.state();
    s.move_task(&key, "completed");
    let stored = w.stored(&key);
    assert_eq!(stored.original.expect("still there").bytes, before.bytes);
    assert_eq!(stored.content["title"], "Other");
    assert_eq!(stored.content["progress"], "completed");
}

#[test]
fn a_status_the_core_only_computes_is_not_written_by_an_edit() {
    let w = World::new();
    let mut task = rich();
    task.as_object_mut().unwrap().remove("progress");
    let key = w.add_json(task);
    let mut s = w.state();
    s.select(&key);
    s.begin_edit();
    assert_eq!(
        s.editing().unwrap().draft.value("progress"),
        Some("in-process"),
        "the form shows what the core computes"
    );
    s.set_field("title", "Other");
    s.save();
    assert!(
        w.json(&key).get("progress").is_none(),
        "and does not write it"
    );
}

#[test]
fn a_status_chosen_in_the_form_is_written_with_its_time() {
    let w = World::new();
    let key = w.add_json(rich());
    let before = w.json(&key);
    edit(&mut w.state(), &key, &[("progress", "completed")]);
    let after = w.json(&key);
    assert_eq!(after["progress"], "completed");
    assert_ne!(after["progressUpdated"], before["progressUpdated"]);
    let others = ["progress", "progressUpdated"];
    assert_eq!(without(after, &others), without(before, &others));
}

#[test]
fn a_series_keeps_its_rule_and_overrides_and_its_form_offers_no_due_date_or_status() {
    let w = World::new();
    let key = w.add_json(series());
    let before = w.json(&key);
    let mut s = w.state();
    s.select(&key);
    s.begin_edit();
    let draft = s.editing().unwrap().draft.clone();
    assert!(draft.note.is_some());
    assert_eq!(draft.value("due"), None);
    assert_eq!(draft.value("progress"), None);
    s.set_field("title", "Report");
    s.save();
    let after = w.json(&key);
    assert_eq!(after["title"], "Report");
    assert_eq!(without(after, &["title"]), without(before, &["title"]));
}

#[test]
fn a_form_that_still_changes_the_due_date_or_status_of_a_series_is_refused_and_changes_nothing() {
    let w = World::new();
    let plain = w.plain("Plain");
    let draft = |field: &str, value: &str| {
        let mut s = w.state();
        s.select(&plain);
        s.begin_edit();
        s.set_field(field, value);
        s.editing().unwrap().draft.clone()
    };
    let mut task = Task::from_value(series()).unwrap();
    let before = task.clone();
    assert!(apply_draft(&mut task, &draft("due", "2027-01-01 10:00")).is_err());
    assert!(apply_draft(&mut task, &draft("progress", "completed")).is_err());
    assert_eq!(task, before);
}

#[test]
fn what_cannot_be_read_as_a_value_is_refused_and_the_draft_stays() {
    let w = World::new();
    let key = w.plain("Plain");
    let before = w.json(&key);
    for (field, value) in [
        ("title", "  "),
        ("priority", "ten"),
        ("priority", "10"),
        ("due", "2026-11-30"),
        ("due", "tomorrow"),
    ] {
        let mut s = w.state();
        s.select(&key);
        s.begin_edit();
        s.set_field(field, value);
        s.save();
        let edit = s.editing().expect("the draft stays");
        assert!(edit.error.is_some(), "{field} {value}");
        assert_eq!(w.json(&key), before, "{field} {value}");
    }
}

#[test]
fn a_save_after_someone_else_changed_the_task_is_a_conflict_and_the_next_writes_knowingly() {
    let w = World::new();
    let key = w.add_json(rich());
    let mut s = w.state();
    s.select(&key);
    s.begin_edit();
    s.set_field("priority", "5");
    w.other_changes(&key, "Theirs");
    let draft = s.editing().unwrap().draft.clone();
    let got = w.adapter.save(&key, &draft);
    assert_eq!(got.unwrap_err().kind, ErrorKind::Conflict);
    assert_eq!(w.json(&key)["priority"], 2, "nothing written");
    s.save();
    assert!(s.editing().is_some(), "the draft stays");
    s.save();
    let after = w.json(&key);
    assert_eq!(after["priority"], 5);
    assert_eq!(after["title"], "Theirs", "only what was changed is written");
}

#[test]
fn a_task_created_in_the_form_has_what_was_filled_in() {
    let w = World::new();
    let mut s = w.state();
    s.begin_create();
    s.set_field("description", "No title yet");
    s.save();
    assert!(s.editing().is_some(), "a title is needed");
    s.set_field("title", "New");
    s.set_field("due", "2026-11-30 17:00");
    s.set_field("priority", "1");
    s.set_field("labels", "a, b");
    s.save();
    assert!(s.editing().is_none());
    let key = s.selected().unwrap().to_string();
    let stored = w.json(&key);
    assert_eq!(stored["title"], "New");
    assert_eq!(stored["due"], "2026-11-30T17:00:00");
    assert_eq!(stored["priority"], 1);
    assert_eq!(stored["keywords"], json!({"a": true, "b": true}));
    assert_eq!(stored["progress"], "needs-action");
    assert_eq!(w.stored(&key).envelope.attributed_to.as_deref(), Some(ME));
}
