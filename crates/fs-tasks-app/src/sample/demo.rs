//! The small believable set of the board and the list.
//!
//! Pattern: Builder data — a table of parts, written by the shared `push`.

use super::parts::{HEAD, Part, push};
use crate::day::Day;

/// About forty tasks around `today`: ready ones due today, overdue and later, some started, chains that wait,
/// a task behind a cancelled one, one that names a task nobody holds, two in a cycle, finished and cancelled ones.
#[must_use]
#[allow(clippy::too_many_lines)] // a table of tasks, one line each
pub fn demo(today: Day) -> String {
    let mut out = String::from(HEAD);
    let d = |n: i64| Some((today.plus(n), "1700"));
    let todo = "NEEDS-ACTION";
    let go = "IN-PROCESS";
    let parts = [
        Part {
            uid: "d1",
            title: "Send the invoice for October",
            status: todo,
            due: d(0),
            who: Some("Anna"),
            priority: Some(1),
            labels: &["money"],
            ..Part::default()
        },
        Part {
            uid: "d2",
            title: "Book the train to Berlin",
            status: todo,
            due: d(0),
            who: Some("Ben"),
            labels: &["travel"],
            ..Part::default()
        },
        Part {
            uid: "d3",
            title: "Reply to the landlord",
            status: todo,
            due: d(-2),
            who: Some("Anna"),
            priority: Some(2),
            ..Part::default()
        },
        Part {
            uid: "d4",
            title: "Review the draft of the handbook",
            status: go,
            due: d(0),
            who: Some("Chiara"),
            labels: &["docs"],
            ..Part::default()
        },
        Part {
            uid: "r1",
            title: "Order new business cards",
            status: todo,
            due: d(3),
            who: Some("Eva"),
            ..Part::default()
        },
        Part {
            uid: "r2",
            title: "Plan the team evening",
            status: todo,
            due: d(9),
            who: Some("Dmitri"),
            labels: &["team"],
            ..Part::default()
        },
        Part {
            uid: "r3",
            title: "Water the plants in the office",
            status: todo,
            ..Part::default()
        },
        Part {
            uid: "r4",
            title: "Update the backup notes",
            status: go,
            due: d(5),
            who: Some("Ben"),
            labels: &["docs", "ops"],
            ..Part::default()
        },
        Part {
            uid: "r5",
            title: "Call the printer about the flyers",
            status: todo,
            due: d(1),
            who: Some("Eva"),
            priority: Some(5),
            ..Part::default()
        },
        Part {
            uid: "r6",
            title: "Sort the photos of the summer fair",
            status: todo,
            due: d(14),
            labels: &["team"],
            ..Part::default()
        },
        Part {
            uid: "a1",
            title: "Write the specification",
            status: "COMPLETED",
            due: d(-6),
            who: Some("Chiara"),
            labels: &["docs"],
            ..Part::default()
        },
        Part {
            uid: "a2",
            title: "Build the first version",
            status: go,
            due: d(4),
            who: Some("Ben"),
            waits_for: &["a1"],
            ..Part::default()
        },
        Part {
            uid: "a3",
            title: "Test the first version",
            status: todo,
            due: d(7),
            who: Some("Anna"),
            waits_for: &["a2"],
            ..Part::default()
        },
        Part {
            uid: "a4",
            title: "Publish the release notes",
            status: todo,
            due: d(8),
            who: Some("Eva"),
            waits_for: &["a3"],
            labels: &["docs"],
            ..Part::default()
        },
        Part {
            uid: "b1",
            title: "Get a quote from the caterer",
            status: "CANCELLED",
            due: d(-3),
            ..Part::default()
        },
        Part {
            uid: "b2",
            title: "Confirm the catering for the fair",
            status: todo,
            due: d(2),
            who: Some("Dmitri"),
            waits_for: &["b1"],
            ..Part::default()
        },
        Part {
            uid: "u1",
            title: "Ask legal about the new terms",
            status: todo,
            due: d(6),
            who: Some("Anna"),
            waits_for: &["unknown-ticket"],
            ..Part::default()
        },
        Part {
            uid: "c1",
            title: "Agree on the budget",
            status: todo,
            due: d(1),
            waits_for: &["c2"],
            ..Part::default()
        },
        Part {
            uid: "c2",
            title: "Agree on the schedule",
            status: todo,
            due: d(1),
            waits_for: &["c1"],
            ..Part::default()
        },
        Part {
            uid: "f1",
            title: "Renew the domain",
            status: "COMPLETED",
            due: d(-9),
            who: Some("Ben"),
            labels: &["ops"],
            ..Part::default()
        },
        Part {
            uid: "f2",
            title: "Pay the hosting bill",
            status: "COMPLETED",
            due: d(-4),
            who: Some("Anna"),
            labels: &["money"],
            ..Part::default()
        },
        Part {
            uid: "f3",
            title: "Clear out the storage room",
            status: "COMPLETED",
            ..Part::default()
        },
        Part {
            uid: "f4",
            title: "Cancel the old phone contract",
            status: "CANCELLED",
            ..Part::default()
        },
    ];
    for part in &parts {
        push(&mut out, part);
    }
    // More of the same kind, so a column has to scroll: a dozen small ones, a few due, a few waiting.
    for i in 0..12_u8 {
        let title = format!("Small errand {}", i + 1);
        let uid = format!("e{i}");
        let wait = [if i % 4 == 3 { "a3" } else { "a1" }];
        let part = Part {
            uid: &uid,
            title: &title,
            status: todo,
            due: if i % 3 == 0 { d(i64::from(i)) } else { None },
            who: [Some("Eva"), None, Some("Ben")][usize::from(i) % 3],
            waits_for: if i % 4 == 3 { &wait } else { &[] },
            ..Part::default()
        };
        push(&mut out, &part);
    }
    out.push_str("END:VCALENDAR\r\n");
    out
}
