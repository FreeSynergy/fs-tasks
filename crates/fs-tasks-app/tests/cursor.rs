//! "Show more" and search read on from a cursor: the pages already read are not read again. Both
//! views of the workbench (list and status) show the same loaded rows, so they page alike.
//!
//! Integration tests against the real server; the client only counts its page reads.

mod common;

use common::World;
use fs_components_workbenches::tasks::{RowQuery, TaskSource, TaskView};
use fs_tasks_core::Task;

/// Task `n` is due `n` minutes after midnight of `day` December 2026, so the order is the day and
/// the number.
fn fill(w: &World, prefix: &str, day: u32, count: usize) {
    for n in 0..count {
        let mut t = Task::new(format!("{prefix} {n:03}"));
        t.uid = format!("{}-{n:04}", prefix.to_lowercase());
        t.due = Some(format!("2026-12-{day:02}T{:02}:{:02}:00", n / 60, n % 60));
        w.add(t);
    }
}

fn all() -> RowQuery {
    RowQuery::default()
}

fn term(t: &str) -> RowQuery {
    let mut q = RowQuery::default();
    q.term = t.into();
    q
}

#[test]
fn show_more_continues_from_the_cursor() {
    let w = World::new();
    fill(&w, "P", 1, 250);
    let a = w.adapter.rows(&all(), 0, 120).unwrap();
    assert_eq!((a.len(), w.pages.reads()), (120, (1, 1)));
    let b = w.adapter.rows(&all(), 120, 120).unwrap();
    assert_eq!((b.len(), w.pages.reads()), (120, (1, 2)));
    assert_eq!(b[0].cells[0], "P 120");
    let c = w.adapter.rows(&all(), 240, 120).unwrap();
    assert_eq!(
        (c.len(), w.pages.reads()),
        (10, (1, 2)),
        "the end was known"
    );
    assert!(w.adapter.rows(&all(), 250, 120).unwrap().is_empty());
    assert_eq!(w.pages.reads(), (1, 2));
}

#[test]
fn a_search_continues_from_the_cursor_and_a_new_term_starts_over() {
    let w = World::new();
    fill(&w, "Alpha", 1, 150);
    fill(&w, "Beta", 2, 3);
    let a = w.adapter.rows(&term("alpha"), 0, 100).unwrap();
    assert_eq!((a.len(), w.pages.reads()), (100, (1, 0)));
    let b = w.adapter.rows(&term("alpha"), 100, 100).unwrap();
    assert_eq!((b.len(), w.pages.reads()), (50, (1, 1)));
    let beta = w.adapter.rows(&term("beta"), 0, 100).unwrap();
    // The three come last in the list: the walk reads both pages to find them.
    assert_eq!((beta.len(), w.pages.reads()), (3, (2, 2)));
}

#[test]
fn a_reload_from_the_start_or_a_jump_reads_afresh_and_stays_right() {
    let w = World::new();
    fill(&w, "P", 1, 250);
    w.adapter.rows(&all(), 0, 100).unwrap();
    let again = w.adapter.rows(&all(), 0, 100).unwrap();
    assert_eq!((again.len(), w.pages.reads().0), (100, 2));
    let jump = w.adapter.rows(&all(), 200, 100).unwrap();
    assert_eq!(jump.len(), 50);
    assert_eq!(jump[0].cells[0], "P 200");
}

#[test]
fn a_task_added_after_the_first_page_does_not_break_show_more() {
    let w = World::new();
    fill(&w, "P", 1, 150);
    w.adapter.rows(&all(), 0, 100).unwrap();
    let mut late = Task::new("Q last");
    late.due = Some("2026-12-30T10:00:00".into());
    w.add(late);
    let more = w.adapter.rows(&all(), 100, 100).unwrap();
    assert_eq!(more.len(), 51);
}

#[test]
fn only_open_tasks_are_filtered_while_walking_and_still_page() {
    let w = World::new();
    for n in 0..260 {
        let mut t = Task::new(format!("T {n:03}"));
        t.uid = format!("t-{n:04}");
        t.due = Some(format!("2026-12-01T{:02}:{:02}:00", n / 60, n % 60));
        t.progress = Some(
            if n % 2 == 0 {
                "needs-action"
            } else {
                "completed"
            }
            .into(),
        );
        w.add(t);
    }
    let mut q = RowQuery::default();
    q.open_only = true;
    let a = w.adapter.rows(&q, 0, 100).unwrap();
    assert_eq!(a.len(), 100);
    assert_eq!(a[99].cells[0], "T 198");
    let b = w.adapter.rows(&q, 100, 100).unwrap();
    assert_eq!((b.len(), b[0].cells[0].as_str()), (30, "T 200"));
    assert_eq!(w.pages.reads(), (1, 2), "three pages of 100 for 260");
}

/// The window asks for one row more than it shows and asks on from the rows it shows, as
/// `TasksState` of the workbench does: it must still be one walk, no restart. The status view shows
/// the same loaded rows.
#[test]
fn the_windows_show_more_is_one_walk_without_restarts() {
    let w = World::new();
    fill(&w, "P", 1, 650);
    let mut s = w.state();
    s.set_view(TaskView::Status);
    assert_eq!(s.rows().len(), 200);
    assert_eq!(w.pages.reads(), (1, 2));
    for shown in [400, 600, 650] {
        s.load_more();
        assert_eq!(s.rows().len(), shown);
    }
    assert!(!s.has_more());
    assert_eq!(w.pages.reads(), (1, 6), "7 pages for 650 tasks");
    assert_eq!(s.rows()[649].cells[0], "P 649");
    assert_eq!(s.rows()[200].cells[0], "P 200", "no row twice, none lost");
}
