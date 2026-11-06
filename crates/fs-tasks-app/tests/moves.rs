//! Moving a card is an edit of the task's own status, and of nothing else.
//!
//! Pattern: Round-trip tests through the real adapter and the real core.

mod common;

use common::{World, rich, series, without};
use fs_components_workbenches::ErrorKind;
use fs_components_workbenches::tasks::{
    BoardOutcome, MoveOutcome, Place, TaskSource, TaskView, TasksWidget,
};
use fs_tasks_app::labels;

fn revision(w: &World, key: &str) -> Option<String> {
    w.adapter.read(key).unwrap().revision
}

#[test]
fn a_move_changes_the_status_and_its_time_and_nothing_else() {
    let w = World::new();
    let key = w.add_json(rich());
    let before = w.json(&key);
    let mut s = w.state();
    assert!(matches!(
        s.move_task(&key, "completed"),
        MoveOutcome::Moved { .. }
    ));
    let after = w.json(&key);
    assert_eq!(after["progress"], "completed");
    assert_ne!(after["progressUpdated"], before["progressUpdated"]);
    let others = ["progress", "progressUpdated"];
    assert_eq!(without(after, &others), without(before, &others));
}

#[test]
fn the_card_stands_in_the_column_of_its_effective_progress() {
    let w = World::new();
    let mut task = rich();
    task.as_object_mut().unwrap().remove("progress");
    let key = w.add_json(task);
    let s = w.state();
    let row = s.rows().iter().find(|r| r.key == key).unwrap();
    assert_eq!(row.column, "in-process", "computed from the participant");
}

#[test]
fn a_move_to_the_status_it_effectively_has_writes_nothing() {
    let w = World::new();
    let mut task = rich();
    task.as_object_mut().unwrap().remove("progress");
    let key = w.add_json(task);
    let before = w.stored(&key);
    let revision = revision(&w, &key);
    w.adapter
        .move_to(&key, "in-process", revision.as_deref())
        .unwrap();
    let after = w.stored(&key);
    assert_eq!(after.content, before.content, "still no stated progress");
    assert_eq!(after.envelope.updated, before.envelope.updated);
    assert_eq!(
        w.state().move_task(&key, "in-process"),
        MoveOutcome::Unchanged
    );
}

#[test]
fn a_move_to_another_status_writes_only_the_value_the_person_chose() {
    let w = World::new();
    let mut task = rich();
    task.as_object_mut().unwrap().remove("progress");
    let key = w.add_json(task);
    let before = w.json(&key);
    w.state().move_task(&key, "failed");
    let after = w.json(&key);
    assert_eq!(after["progress"], "failed");
    assert_eq!(
        after["participants"], before["participants"],
        "what the participants said stays"
    );
    assert_eq!(after["percentComplete"], before["percentComplete"]);
    assert_eq!(
        after["freesynergy.net:taskStatus"],
        before["freesynergy.net:taskStatus"]
    );
}

#[test]
fn a_series_is_refused_clearly_and_stays_as_it_was() {
    let w = World::new();
    let key = w.add_json(series());
    let before = w.stored(&key);
    let mut s = w.state();
    let row = s.rows()[0].clone();
    assert!(row.move_refusal.is_some(), "the card says so");
    let got = s.move_task(&key, "completed");
    assert!(matches!(got, MoveOutcome::Refused(_)), "{got:?}");
    assert!(s.notice().unwrap().contains("repeats"));
    let after = w.stored(&key);
    assert_eq!(after.content, before.content);
    assert_eq!(after.envelope.updated, before.envelope.updated);
    let refused = w
        .adapter
        .move_to(&key, "completed", row.revision.as_deref());
    assert!(refused.is_err(), "the source refuses too, whoever asks");
}

#[test]
fn one_occurrence_on_its_own_is_refused_too() {
    let w = World::new();
    let mut task = serde_json::json!({
        "@type": "Task", "version": "2.0", "uid": "one-occurrence", "title": "One",
        "recurrenceId": "2026-11-09T09:00:00"
    });
    task["start"] = "2026-11-09T09:00:00".into();
    let key = w.add_json(task);
    let got = w.state().move_task(&key, "completed");
    assert!(matches!(got, MoveOutcome::Refused(_)), "{got:?}");
}

#[test]
fn a_move_from_an_old_row_is_a_conflict_and_changes_nothing() {
    let w = World::new();
    let key = w.plain("Plain");
    let old = revision(&w, &key);
    w.other_changes(&key, "Theirs");
    let got = w.adapter.move_to(&key, "completed", old.as_deref());
    assert_eq!(got.unwrap_err().kind, ErrorKind::Conflict);
    assert_eq!(w.json(&key)["progress"], "needs-action");
}

#[test]
fn a_status_the_standard_does_not_know_is_refused() {
    let w = World::new();
    let key = w.plain("Plain");
    let revision = revision(&w, &key);
    assert!(
        w.adapter
            .move_to(&key, "someday", revision.as_deref())
            .is_err()
    );
    assert_eq!(w.json(&key)["progress"], "needs-action");
}

#[test]
fn a_task_gone_from_the_store_leaves_the_view() {
    let w = World::new();
    let key = w.plain("Plain");
    let mut s = w.state();
    let id = key.parse().unwrap();
    w.runtime
        .block_on(fs_tasks_core::TaskItems::delete(w.repo.as_ref(), id))
        .unwrap();
    assert_eq!(s.move_task(&key, "completed"), MoveOutcome::Gone);
    assert!(s.rows().is_empty());
    assert_eq!(s.selected(), None);
}

#[test]
fn the_list_and_the_status_view_are_views_of_one_set_of_tasks() {
    let w = World::new();
    let key = w.plain("Plain");
    let widget = TasksWidget::new(w.adapter.clone(), labels());
    widget.with_state(|s| s.set_view(TaskView::Status));
    assert_eq!(widget.lanes().len(), 5, "one column per value of progress");
    assert_eq!(widget.lanes()[0].items[0].key, key);
    assert_eq!(widget.lanes()[0].title, "To do (1)");
    assert!(matches!(widget.grab(&key), BoardOutcome::Grabbed(_)));
    widget.hover(&Place::new("completed", 0));
    let moved = widget.drop_held();
    assert!(matches!(moved, BoardOutcome::Moved(_)), "{moved:?}");
    assert_eq!(w.json(&key)["progress"], "completed");
    assert!(widget.lanes()[0].items.is_empty(), "the lanes follow");
    assert_eq!(widget.lanes()[2].items[0].key, key);
    let row = widget.with_state(|s| s.rows()[0].cells[3].clone());
    assert_eq!(row, "Done", "and so does the list");
    assert_eq!(widget.context().view, TaskView::Status);
    assert_eq!(widget.context().selected.as_deref(), Some(key.as_str()));
}

#[test]
fn a_move_by_gesture_that_the_core_refuses_puts_the_card_back() {
    let w = World::new();
    let key = w.add_json(series());
    let widget = TasksWidget::new(w.adapter.clone(), labels());
    let refused = widget.grab(&key);
    assert!(matches!(refused, BoardOutcome::Refused(_)), "{refused:?}");
    assert_eq!(widget.lanes()[0].items[0].key, key);
    let says = widget.with_state(|s| s.notice().map(str::to_string));
    assert!(says.unwrap().contains("repeats"));
}
