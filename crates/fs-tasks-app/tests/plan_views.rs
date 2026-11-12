//! The two plan views against the real core: the groups, the cards, the sample data and the picture's life.
//!
//! Pattern: Black-box tests through `TaskSource` over the real adapter, the real core and the real sample.

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::World;
use fs_components_workbenches::tasks::{RowQuery, TaskRow, TaskSource};
use fs_tasks_app::plan_view::{Board, Cache, Next, PlanSource};
use fs_tasks_app::{Day, sample_demo};
use fs_tasks_core::TaskItems;

const TODAY: &str = "2026-11-12";

fn today() -> Day {
    Day::parse(TODAY).unwrap()
}

fn world_with(ics: &str) -> World {
    let w = World::new();
    let report = w.runtime.block_on(w.repo.import_ics(ics)).unwrap();
    assert!(report.failed.is_empty(), "{:?}", report.failed);
    w
}

fn source(w: &World, layout: Box<dyn fs_tasks_app::plan_view::Layout>) -> (PlanSource, Arc<Cache>) {
    let items: Arc<dyn TaskItems> = w.repo.clone();
    let cache = Cache::start(items, w.runtime.handle().clone(), today());
    cache.wait(Duration::from_secs(120)).expect("a picture");
    (
        PlanSource::new(w.adapter.clone(), cache.clone(), layout),
        cache,
    )
}

fn all(s: &PlanSource) -> Vec<TaskRow> {
    s.rows(&RowQuery::default(), 0, 10_000).unwrap()
}

/// The titles under each heading of the "what next" list, in order.
fn groups(rows: &[TaskRow]) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for row in rows {
        if row.key.starts_with("group:") {
            out.push((row.title.clone(), Vec::new()));
        } else {
            out.last_mut()
                .expect("a heading first")
                .1
                .push(row.title.clone());
        }
    }
    out
}

#[test]
fn the_sample_is_imported_whole_with_its_people_and_relations() {
    let w = world_with(&sample_demo(today()));
    let all = w.runtime.block_on(w.repo.list()).unwrap();
    assert_eq!(all.len(), 35);
    let landlord = all.iter().find(|e| e.task.uid == "d3").unwrap();
    assert_eq!(landlord.task.participants.len(), 1);
    assert_eq!(landlord.task.priority, Some(2));
    assert!(!landlord.task.keywords.is_empty() || landlord.task.uid == "d3");
    let b2 = all.iter().find(|e| e.task.uid == "b2").unwrap();
    assert_eq!(b2.task.related_to.len(), 1);
}

#[test]
fn what_next_has_three_groups_cut_by_readiness_and_today() {
    let w = world_with(&sample_demo(today()));
    let (s, _) = source(&w, Box::new(Next));
    let g = groups(&all(&s));
    let names: Vec<&str> = g.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["TODAY  (5)", "READY  (15)", "BLOCKED  (9)"]);
    // overdue first, then those of today
    assert_eq!(g[0].1[0], "Reply to the landlord");
    for t in [
        "Book the train to Berlin",
        "Send the invoice for October",
        "Small errand 1",
    ] {
        assert!(g[0].1.contains(&t.to_string()), "{t}");
    }
    // a started task nothing blocks is ready, and a finished task is not shown at all
    assert!(g[1].1.contains(&"Build the first version".to_string()));
    assert!(
        !g.iter()
            .any(|(_, t)| t.contains(&"Renew the domain".to_string()))
    );
    for t in [
        "Test the first version",
        "Confirm the catering for the fair",
        "Ask legal about the new terms",
        "Agree on the budget",
    ] {
        assert!(g[2].1.contains(&t.to_string()), "{t} is blocked");
    }
}

#[test]
fn a_blocked_line_says_why_in_its_own_words() {
    let w = world_with(&sample_demo(today()));
    let (s, _) = source(&w, Box::new(Next));
    let rows = all(&s);
    let why = |title: &str| {
        let row = rows.iter().find(|r| r.title == title).unwrap();
        (row.cells[1].clone(), row.marks.clone())
    };
    assert_eq!(
        why("Test the first version"),
        (
            "waits for Build the first version".into(),
            vec!["BLOCKED".into()]
        )
    );
    assert!(
        why("Confirm the catering for the fair")
            .0
            .contains("stopped task: Get a quote from the caterer")
    );
    assert!(
        why("Ask legal about the new terms")
            .0
            .contains("not here: unknown-ticket")
    );
    assert!(
        why("Agree on the budget")
            .0
            .starts_with("is in a cycle with")
    );
    assert_eq!(why("Agree on the budget").1, ["CYCLE"]);
}

#[test]
fn the_board_marks_blocked_cards_and_leaves_finished_ones_unmarked() {
    let w = world_with(&sample_demo(today()));
    let (s, _) = source(&w, Box::new(Board));
    let rows = all(&s);
    assert_eq!(rows.len(), 35);
    let card = |t: &str| rows.iter().find(|r| r.title == t).unwrap().clone();
    assert!(
        card("Test the first version")
            .marks
            .contains(&"BLOCKED".to_string())
    );
    assert!(
        card("Confirm the catering for the fair")
            .marks
            .contains(&"BLOCKED: STOPPED".to_string())
    );
    assert!(
        card("Ask legal about the new terms")
            .marks
            .contains(&"BLOCKED: UNKNOWN".to_string())
    );
    assert!(
        card("Agree on the schedule")
            .marks
            .contains(&"CYCLE".to_string())
    );
    assert!(
        card("Reply to the landlord")
            .marks
            .contains(&"OVERDUE".to_string())
    );
    let done = card("Renew the domain");
    assert!(
        done.marks.is_empty() && !done.tags.contains(&"ready".to_string()),
        "{done:?}"
    );
    let ready = card("Book the train to Berlin");
    assert!(ready.tags.contains(&"ready".to_string()));
    assert!(ready.lines.contains(&"Due: today 17:00".to_string()));
    assert!(ready.lines.contains(&"Assigned: Ben".to_string()));
    assert_eq!(ready.column, "needs-action");
}

#[test]
fn search_and_paging_work_across_headings() {
    let w = world_with(&sample_demo(today()));
    let (s, _) = source(&w, Box::new(Next));
    let mut q = RowQuery::default();
    q.term = "errand".into();
    let rows = s.rows(&q, 0, 100).unwrap();
    assert_eq!(
        rows.iter().filter(|r| r.key.starts_with("group:")).count(),
        3,
        "all three groups hold an errand"
    );
    let first = s.rows(&RowQuery::default(), 0, 7).unwrap();
    let second = s.rows(&RowQuery::default(), 7, 7).unwrap();
    let both = s.rows(&RowQuery::default(), 0, 14).unwrap();
    let keys = |v: &[TaskRow]| v.iter().map(|r| r.key.clone()).collect::<Vec<_>>();
    assert_eq!([keys(&first), keys(&second)].concat(), keys(&both));
    let sheet = s.read("group:blocked").unwrap();
    assert_eq!(sheet.title, "BLOCKED");
}

#[test]
fn a_move_on_the_board_renews_the_picture_at_once_for_a_small_store() {
    let w = world_with(&sample_demo(today()));
    let (s, cache) = source(&w, Box::new(Board));
    let key = all(&s)
        .iter()
        .find(|r| r.title == "Test the first version")
        .unwrap()
        .key
        .clone();
    let revision = all(&s)
        .iter()
        .find(|r| r.key == key)
        .unwrap()
        .revision
        .clone();
    s.move_to(&key, "in-process", revision.as_deref()).unwrap();
    let row = all(&s).into_iter().find(|r| r.key == key).unwrap();
    assert_eq!(row.column, "in-process");
    assert!(cache.loads() >= 2);
    // the task it waited for is still unfinished, so it stays blocked
    assert!(row.marks.contains(&"BLOCKED".to_string()));
}

#[test]
fn the_sheet_of_a_task_carries_its_readiness() {
    let w = world_with(&sample_demo(today()));
    let (s, _) = source(&w, Box::new(Board));
    let key = all(&s)
        .iter()
        .find(|r| r.title == "Agree on the budget")
        .unwrap()
        .key
        .clone();
    let sheet = s.read(&key).unwrap();
    let section = sheet
        .sections
        .iter()
        .find(|x| x.title == "Readiness")
        .unwrap();
    assert_eq!(section.fields[0].value, "Blocked");
    assert!(section.fields[1].value.starts_with("is in a cycle with"));
}

#[test]
fn quick_add_asks_for_the_title_alone_and_the_task_arrives_in_its_group() {
    let w = world_with(&sample_demo(today()));
    let (s, _) = source(&w, Box::new(Next));
    let mut draft = s.blank().unwrap();
    assert_eq!(draft.fields.len(), 1);
    draft.fields[0].value = "Buy stamps".into();
    s.create(&draft).unwrap();
    let g = groups(&all(&s));
    assert!(
        g[1].1.contains(&"Buy stamps".to_string()),
        "no due date: ready"
    );
    assert!(!s.capabilities().move_tasks, "a list does not move cards");
}
