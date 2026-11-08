//! A marked task is marked wherever it appears, is not opened on its own, and a save never reduces the
//! marking (ADR 0029).
//!
//! Pattern: Specification tests over the real core.

mod common;

use common::{World, without};
use fs_components_workbenches::tasks::{Shown, TaskSource};
use fs_objects::ObjectId;
use fs_tasks_app::is_marked;
use fs_tasks_core::TaskItems;
use serde_json::{Value, json};

fn task(uid: &str, extra: Value) -> Value {
    let mut value = json!({"@type": "Task", "version": "2.0", "uid": uid, "title": uid});
    let Value::Object(extra) = extra else {
        panic!("extra must be an object");
    };
    for (k, v) in extra {
        value[k] = v;
    }
    value
}

fn marked(w: &World, key: &str) -> bool {
    let id: ObjectId = key.parse().unwrap();
    let entry = w
        .runtime
        .block_on(TaskItems::get(w.repo.as_ref(), id))
        .unwrap()
        .unwrap();
    is_marked(&entry)
}

#[test]
fn every_form_of_a_marking_counts_and_a_mention_does_not() {
    let w = World::new();
    let sensitive = json!(["pd:HealthRecord"]);
    for (uid, extra) in [
        ("top", json!({"freesynergy.net:sensitive": sensitive})),
        ("empty", json!({"freesynergy.net:sensitive": []})),
        ("private", json!({"privacy": "private"})),
        ("secret", json!({"privacy": "secret"})),
        (
            "in-override",
            json!({"start": "2026-11-02T09:00:00",
            "recurrenceRule": {"@type": "RecurrenceRule", "frequency": "weekly"},
            "recurrenceOverrides": {"2026-11-09T09:00:00": {"freesynergy.net:sensitive": true}}}),
        ),
        (
            "in-participant",
            json!({"participants": {"p": {"@type": "Participant",
            "freesynergy.net:sensitive": ["x"], "roles": {"attendee": true}}}}),
        ),
    ] {
        let key = w.add_json(task(uid, extra));
        assert!(marked(&w, &key), "{uid}");
    }
    for (uid, extra) in [
        ("plain", json!({})),
        ("public", json!({"privacy": "public"})),
        (
            "mention",
            json!({"description": "see freesynergy.net:sensitive and privacy"}),
        ),
    ] {
        let key = w.add_json(task(uid, extra));
        assert!(!marked(&w, &key), "{uid}");
    }
}

fn repeating(patch: Value) -> Value {
    let mut value = json!({"start": "2026-11-02T09:00:00",
        "recurrenceRule": {"@type": "RecurrenceRule", "frequency": "weekly"},
        "recurrenceOverrides": {}});
    value["recurrenceOverrides"]["2026-11-09T09:00:00"] = patch;
    value
}

#[test]
fn a_marking_written_as_a_patch_path_counts() {
    let w = World::new();
    for (uid, patch) in [
        (
            "as-path",
            json!({"participants/p/freesynergy.net:sensitive": ["x"]}),
        ),
        ("in-patch", json!({"freesynergy.net:sensitive": ["x"]})),
        ("private-patch", json!({"privacy": "private"})),
        ("private-path", json!({"participants/p/privacy": "secret"})),
    ] {
        let key = w.add_json(task(uid, repeating(patch)));
        assert!(marked(&w, &key), "{uid}");
    }
}

#[test]
fn a_key_of_an_id_map_is_data_and_marks_nothing() {
    let w = World::new();
    for (uid, extra) in [
        (
            "label",
            json!({"keywords": {"privacy": true, "freesynergy.net:sensitive": true}}),
        ),
        ("label-path", repeating(json!({"keywords/privacy": true}))),
        (
            "label-path-2",
            repeating(json!({"keywords/freesynergy.net:sensitive": true})),
        ),
        ("public-patch", repeating(json!({"privacy": "public"}))),
        (
            "participant-id",
            json!({"participants": {"privacy": {"@type": "Participant",
            "roles": {"attendee": true}}}}),
        ),
        (
            "related-uid",
            json!({"relatedTo": {"privacy": {"@type": "Relation"}}}),
        ),
    ] {
        let key = w.add_json(task(uid, extra));
        assert!(!marked(&w, &key), "{uid}");
    }
    let marked_inside = json!({"participants": {"privacy": {"@type": "Participant",
        "freesynergy.net:sensitive": ["x"], "roles": {"attendee": true}}}});
    let key = w.add_json(task("inside", marked_inside));
    assert!(marked(&w, &key), "the value of an id is still looked at");
}

#[test]
fn a_mark_on_the_envelope_alone_counts() {
    let w = World::new();
    let key = w.plain("Plain");
    assert!(!marked(&w, &key));
    w.mark_envelope(&key);
    assert!(marked(&w, &key));
}

#[test]
fn a_marked_task_is_marked_in_list_card_sheet_and_form() {
    let w = World::new();
    let key = w.add_json(task("m", json!({"freesynergy.net:sensitive": ["x"]})));
    let mut s = w.state();
    let row = s.rows()[0].clone();
    assert_eq!(row.marks, ["Sensitive"]);
    assert!(row.only_when_chosen);
    s.select(&key);
    let Shown::Task { sheet, .. } = s.shown() else {
        panic!("{:?}", s.shown());
    };
    assert_eq!(sheet.marks, ["Sensitive"]);
    s.begin_edit();
    assert_eq!(s.editing().unwrap().draft.marks, ["Sensitive"]);
}

#[test]
fn a_marked_task_is_not_opened_on_its_own() {
    let w = World::new();
    let mut first = task("a-first", json!({"freesynergy.net:sensitive": ["x"]}));
    first["due"] = "2026-01-01T00:00:00".into();
    w.add_json(first);
    let s = w.state();
    assert_eq!(s.rows().len(), 1);
    assert_eq!(s.selected(), None, "at the start");

    let w = World::new();
    let mut plain = task("plain", json!({}));
    plain["due"] = "2026-01-01T00:00:00".into();
    let plain = w.add_json(plain);
    let mut next = task("next", json!({"freesynergy.net:sensitive": ["x"]}));
    next["due"] = "2026-01-02T00:00:00".into();
    w.add_json(next);
    let mut s = w.state();
    assert_eq!(s.selected(), Some(plain.as_str()));
    s.ask_delete();
    s.confirm_delete();
    assert_eq!(s.selected(), None, "after a delete");
}

#[test]
fn a_save_and_a_move_keep_every_marking() {
    let w = World::new();
    let key = w.add_json(task(
        "m",
        json!({"freesynergy.net:sensitive": ["pd:HealthRecord"], "privacy": "secret"}),
    ));
    w.mark_envelope(&key);
    let before = w.stored(&key);
    let mut s = w.state();
    s.select(&key);
    s.begin_edit();
    s.set_field("title", "Other");
    s.save();
    assert!(s.editing().is_none(), "{:?}", s.editing());
    s.move_task(&key, "completed");
    let after = w.stored(&key);
    assert_eq!(after.envelope.sensitive, before.envelope.sensitive);
    assert_eq!(
        after.content["freesynergy.net:sensitive"],
        before.content["freesynergy.net:sensitive"]
    );
    assert_eq!(after.content["privacy"], "secret");
    let others = ["title", "progress", "progressUpdated"];
    assert_eq!(
        without(after.content, &others),
        without(before.content, &others)
    );
}

#[test]
fn the_adapter_reads_marks_from_what_is_stored_now_not_from_what_a_draft_says() {
    let w = World::new();
    let key = w.plain("Plain");
    let mut draft = w.adapter.draft(&key).unwrap();
    assert_eq!(draft.marks, [] as [std::string::String; 0]);
    w.mark_envelope(&key);
    draft.revision = w.adapter.draft(&key).unwrap().revision;
    draft.fields[0].value = "Other".into();
    w.adapter.save(&key, &draft).unwrap();
    assert_eq!(w.stored(&key).envelope.sensitive, ["pd:HealthRecord"]);
}
