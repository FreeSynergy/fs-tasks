//! `TaskBackend` against the real core over an in-memory store: create, change, find, refusals.
//!
//! Pattern: none (tests).
#![allow(clippy::pedantic, clippy::cloned_ref_to_slice_refs)]

mod support;

use fs_admin_frontend::{Backend, BackendError, Values};
use fs_tasks_core::{Task, TaskItems};
use fs_types_frontend::{RecordProblem, TypeProblem};
use support::{create, first, v, world};

#[test]
fn create_and_list_with_instants_in_the_zone_of_the_task() {
    let w = world();
    let id = create(
        &w,
        &[
            ("title", &["Minutes"]),
            ("timeZone", &["Europe/Berlin"]),
            ("due", &["2026-12-01T09:00:00Z"]),
            ("keywords", &["b, a"]),
        ],
    );
    assert!(id.starts_with("urn:uuid:"));
    // stored as local time of the zone, shown as the instant again
    let stored = w.runtime.block_on(w.repo.list()).unwrap();
    assert_eq!(stored[0].task.due.as_deref(), Some("2026-12-01T10:00:00"));
    assert_eq!(
        first(&w, &id, "due").as_deref(),
        Some("2026-12-01T09:00:00Z")
    );
    assert_eq!(first(&w, &id, "keywords").as_deref(), Some("a, b"));
    assert_eq!(first(&w, &id, "progress").as_deref(), Some("needs-action"));
}

#[test]
fn change_is_partial() {
    let w = world();
    let id = create(
        &w,
        &[
            ("title", &["One"]),
            ("description", &["keep me"]),
            ("keywords", &["x"]),
            ("priority", &["3"]),
        ],
    );
    // title is required and sent; nothing else is: nothing else changes
    w.backend
        .run("change", &v(&[("id", &[&id]), ("title", &["Two"])]))
        .unwrap();
    assert_eq!(first(&w, &id, "title").as_deref(), Some("Two"));
    assert_eq!(first(&w, &id, "description").as_deref(), Some("keep me"));
    assert_eq!(first(&w, &id, "priority").as_deref(), Some("3"));
    // an empty list clears, values replace
    w.backend
        .run(
            "change",
            &v(&[
                ("id", &[&id]),
                ("title", &["Two"]),
                ("keywords", &[]),
                ("description", &["new"]),
            ]),
        )
        .unwrap();
    assert_eq!(first(&w, &id, "keywords"), None);
    assert_eq!(first(&w, &id, "description").as_deref(), Some("new"));
    assert_eq!(first(&w, &id, "priority").as_deref(), Some("3"));
}

#[test]
fn change_keeps_what_the_description_does_not_show() {
    let w = world();
    let mut task = Task::new("Vendor");
    task.set_vendor("freesynergy.net:note", serde_json::json!("secret"));
    task.privacy = Some("private".into());
    let entry = w.runtime.block_on(w.repo.create(task)).unwrap();
    let id = fs_tasks_app::record_id(entry.id);
    w.backend
        .run("change", &v(&[("id", &[&id]), ("title", &["Vendor 2"])]))
        .unwrap();
    let after = w
        .runtime
        .block_on(w.repo.get(entry.id))
        .unwrap()
        .unwrap()
        .task;
    assert_eq!(after.title.as_deref(), Some("Vendor 2"));
    assert_eq!(after.privacy.as_deref(), Some("private"));
    assert_eq!(
        after.vendor("freesynergy.net:note"),
        Some(&serde_json::json!("secret"))
    );
}

#[test]
fn locations_and_links_replace_only_what_has_a_value() {
    let w = world();
    let id = create(
        &w,
        &[
            ("title", &["Where"]),
            ("locations", &["geo:48.2,16.37"]),
            ("links", &["https://example.org/a"]),
        ],
    );
    w.backend
        .run(
            "change",
            &v(&[
                ("id", &[&id]),
                ("title", &["Where"]),
                ("links", &["https://example.org/b"]),
            ]),
        )
        .unwrap();
    assert_eq!(
        first(&w, &id, "links").as_deref(),
        Some("https://example.org/b")
    );
    assert_eq!(
        first(&w, &id, "locations").as_deref(),
        Some("geo:48.2,16.37")
    );
}

#[test]
fn find_filters() {
    let w = world();
    let a = create(
        &w,
        &[
            ("title", &["Alpha"]),
            ("due", &["2026-12-01T09:00:00Z"]),
            ("keywords", &["work"]),
        ],
    );
    let b = create(&w, &[("title", &["Beta"]), ("progress", &["completed"])]);
    let ids = |q: &[(&str, &[&str])]| -> Vec<String> {
        w.backend
            .list(&v(q))
            .unwrap()
            .into_iter()
            .map(|r| r.id)
            .collect()
    };
    assert_eq!(ids(&[]).len(), 2);
    assert_eq!(ids(&[("term", &["alp"])]), [a.clone()]);
    assert_eq!(ids(&[("progress", &["completed"])]), [b.clone()]);
    assert_eq!(ids(&[("label", &["work"])]), [a.clone()]);
    assert_eq!(ids(&[("open", &["open"])]), [a.clone()]);
    assert_eq!(
        ids(&[
            ("dueFrom", &["2026-12-01T00:00:00Z"]),
            ("dueTo", &["2026-12-02T00:00:00Z"])
        ]),
        [a]
    );
    assert!(ids(&[("dueFrom", &["2027-01-01T00:00:00Z"])]).is_empty());
}

#[test]
fn findings_are_refusals_with_issues() {
    let w = world();
    let id = create(&w, &[("title", &["T"])]);
    let bad = |input: Values| match w.backend.run("change", &input) {
        Err(BackendError::Rejected(issues)) => issues,
        other => panic!("{other:?}"),
    };
    let issues = bad(v(&[
        ("id", &[&id]),
        ("title", &["T"]),
        ("priority", &["10"]),
    ]));
    assert_eq!(issues[0].field, "priority");
    let issues = bad(v(&[
        ("id", &[&id]),
        ("title", &["T"]),
        ("progress", &["done"]),
    ]));
    assert_eq!(
        issues[0].problem,
        RecordProblem::Value(TypeProblem::NotInList)
    );
    let issues = bad(v(&[
        ("id", &[&id]),
        ("title", &["T"]),
        ("due", &["tomorrow"]),
    ]));
    assert_eq!(issues[0].field, "due");
    let issues = bad(v(&[
        ("id", &[&id]),
        ("title", &["T"]),
        ("estimatedDuration", &["two hours"]),
    ]));
    assert_eq!(issues[0].field, "estimatedDuration");
    // required field missing
    assert!(
        matches!(w.backend.run("create", &v(&[])), Err(BackendError::Rejected(i)) if i[0].problem == RecordProblem::Missing)
    );
    // nothing was written by the refusals
    assert_eq!(first(&w, &id, "priority"), None);
}

#[test]
fn unknown_task_and_unknown_command() {
    let w = world();
    let gone = "urn:uuid:00000000-0000-7000-8000-000000000000";
    assert_eq!(
        w.backend
            .run("change", &v(&[("id", &[gone]), ("title", &["x"])])),
        Err(BackendError::NotFound)
    );
    assert_eq!(
        w.backend.run("delete", &v(&[("id", &[gone])])),
        Err(BackendError::NotFound)
    );
    assert_eq!(
        w.backend.run("nonsense", &v(&[])),
        Err(BackendError::NotFound)
    );
}
