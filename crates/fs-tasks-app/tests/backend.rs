//! `TaskBackend` against the real core over an in-memory store, command by command.
//!
//! Pattern: none (tests). Nothing here pretends to be the core: the repository is the real one.
#![allow(clippy::pedantic)]

use std::sync::Arc;

use fs_admin_frontend::{Backend, BackendError, Values};
use fs_objects::{Store, TursoStore};
use fs_tasks_app::TaskBackend;
use fs_tasks_core::describe::support::{self, TABLE};
use fs_tasks_core::{DescribesType, Task, TaskItems, TaskRepo};
use fs_types::{RecordProblem, TypeProblem};
use tokio::runtime::Runtime;

struct World {
    runtime: Runtime,
    repo: Arc<TaskRepo>,
    backend: TaskBackend,
}

fn world() -> World {
    let runtime = Runtime::new().unwrap();
    let store: Arc<dyn Store> = Arc::new(runtime.block_on(TursoStore::open_in_memory()).unwrap());
    let repo = Arc::new(TaskRepo::new(store, "https://local.invalid/users/owner"));
    let backend = TaskBackend::new(repo.clone(), runtime.handle().clone());
    World {
        runtime,
        repo,
        backend,
    }
}

fn v(pairs: &[(&str, &[&str])]) -> Values {
    pairs
        .iter()
        .map(|(k, x)| ((*k).to_owned(), x.iter().map(|s| (*s).to_owned()).collect()))
        .collect()
}

fn create(w: &World, pairs: &[(&str, &[&str])]) -> String {
    w.backend.run("create", &v(pairs)).unwrap().id.unwrap()
}

fn first(w: &World, id: &str, field: &str) -> Option<String> {
    let all = w.backend.list(&Values::new()).unwrap();
    all.into_iter()
        .find(|r| r.id == id)
        .and_then(|r| r.values.get(field).and_then(|x| x.first().cloned()))
}

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
    assert_eq!(ids(&[("text", &["alp"])]), [a.clone()]);
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
    assert!(!support::is_supported("cardMove"));
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
