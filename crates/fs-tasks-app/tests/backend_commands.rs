//! `TaskBackend` against the real core over an in-memory store: the other commands and the refusal of the unsupported ones.
//!
//! Pattern: none (tests).
#![allow(clippy::pedantic, clippy::cloned_ref_to_slice_refs)]

mod support;

use fs_admin_frontend::{Backend, BackendError, Values};
use fs_tasks_core::describe::support::{self as table, TABLE};
use fs_tasks_core::{DescribesType, Task, TaskItems};

use support::{create, first, v, world};

#[test]
fn every_unsupported_command_is_refused_and_changes_nothing() {
    let w = world();
    let id = create(&w, &[("title", &["Stay"])]);
    let d = Task::describe();
    let mut refused = 0;
    for row in TABLE.iter().filter(|r| !r.supported) {
        let input = if d
            .find_command(row.command)
            .unwrap()
            .input
            .iter()
            .any(|f| f.name == "id")
        {
            v(&[("id", &[&id])])
        } else {
            v(&[])
        };
        assert_eq!(
            w.backend.run(row.command, &input),
            Err(BackendError::Unavailable(format!(
                "unsupported: {}",
                row.command
            ))),
            "{}",
            row.command
        );
        refused += 1;
    }
    assert!(refused > 30);
    assert_eq!(w.backend.list(&Values::new()).unwrap().len(), 1);
    assert!(!table::is_supported("cardMove"));
}

#[test]
fn assign_label_relate_delete() {
    let w = world();
    let a = create(&w, &[("title", &["A"])]);
    let b = create(&w, &[("title", &["B"])]);
    w.backend
        .run(
            "assign",
            &v(&[
                ("id", &[&a]),
                ("name", &["Ada"]),
                ("email", &["ada@example.org"]),
            ]),
        )
        .unwrap();
    assert_eq!(
        first(&w, &a, "participants").as_deref(),
        Some("mailto:ada@example.org")
    );
    w.backend
        .run("label", &v(&[("id", &[&a]), ("label", &["urgent"])]))
        .unwrap();
    assert_eq!(first(&w, &a, "keywords").as_deref(), Some("urgent"));
    w.backend
        .run(
            "relate",
            &v(&[
                ("id", &[&a]),
                ("other", &[&b]),
                ("relation", &["depends-on"]),
            ]),
        )
        .unwrap();
    let uid_b = w
        .runtime
        .block_on(w.repo.list())
        .unwrap()
        .into_iter()
        .find(|e| e.task.title.as_deref() == Some("B"))
        .unwrap()
        .task
        .uid;
    assert_eq!(first(&w, &a, "relatedTo"), Some(uid_b));
    w.backend.run("delete", &v(&[("id", &[&b])])).unwrap();
    assert_eq!(w.backend.list(&Values::new()).unwrap().len(), 1);
}

#[test]
fn ics_check_suggest() {
    let w = world();
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VTODO\r\nUID:todo-1\r\nDTSTAMP:20261101T000000Z\r\nDUE:20261112T170000Z\r\nSUMMARY:Minutes\r\nSTATUS:NEEDS-ACTION\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";
    let out = w.backend.run("importIcs", &v(&[("ics", &[ics])])).unwrap();
    assert_eq!(out.values["created"].len(), 1);
    let id = out.values["created"][0].clone();
    let exported = w.backend.run("exportIcs", &v(&[("id", &[&id])])).unwrap();
    assert!(exported.values["ics"][0].contains("SUMMARY:Minutes"));
    let again = w.backend.run("importIcs", &v(&[("ics", &[ics])])).unwrap();
    assert_eq!(again.values["skipped"].len(), 1);
    let checked = w.backend.run("check", &v(&[("title", &["ok"])])).unwrap();
    assert!(checked.values["issues"].is_empty());
    // nothing was stored by check
    assert_eq!(w.backend.list(&Values::new()).unwrap().len(), 1);
    create(&w, &[("title", &["K"]), ("keywords", &["home, work"])]);
    let s = w.backend.run("suggest", &v(&[("typed", &["wo"])])).unwrap();
    assert_eq!(s.values["keywords"], ["work"]);
}

#[test]
fn shown_only_fields_are_accepted_unchanged_and_refused_changed() {
    let w = world();
    let id = create(&w, &[("title", &["P"])]);
    w.backend
        .run(
            "assign",
            &v(&[
                ("id", &[&id]),
                ("name", &["Ada"]),
                ("email", &["ada@example.org"]),
            ]),
        )
        .unwrap();
    // sent back as they are: fine
    w.backend
        .run(
            "change",
            &v(&[
                ("id", &[&id]),
                ("title", &["P2"]),
                ("participants", &["mailto:ada@example.org"]),
            ]),
        )
        .unwrap();
    // changed: refused, nothing written
    let r = w.backend.run(
        "change",
        &v(&[
            ("id", &[&id]),
            ("title", &["P3"]),
            ("participants", &["mailto:bob@example.org"]),
        ]),
    );
    assert!(matches!(r, Err(BackendError::Rejected(i)) if i[0].field == "participants"));
    assert_eq!(first(&w, &id, "title").as_deref(), Some("P2"));
}
