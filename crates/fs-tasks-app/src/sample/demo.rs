//! The small believable set of the board and the list.
//!
//! Pattern: Builder data — a table of parts, written by the shared `push`.

use super::parts::{HEAD, Part, push};
use crate::day::Day;

/// Titles of the small errands.
const ERRANDS: [&str; 12] = [
    "Prepare the agenda for the Monday meeting",
    "Order coffee and oatmilk for the kitchen",
    "Back up the laptops of the new colleagues",
    "Renew the accessibility statement",
    "Reply to the press enquiry from Kurier",
    "Check the fire extinguisher dates",
    "Collect receipts for the expense report",
    "Update the opening hours on the website",
    "Send the thank-you notes after the fair",
    "Proofread the newsletter for December",
    "Book the meeting room for the review",
    "Print the visitor badges for Thursday",
];

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
            title: "Send the October invoice to Brightwave",
            status: todo,
            due: d(0),
            who: Some("Anna Becker"),
            priority: Some(1),
            labels: &["finance"],
            ..Part::default()
        },
        Part {
            uid: "d2",
            title: "Book train tickets for the Berlin workshop",
            status: todo,
            due: d(0),
            who: Some("Ben Okafor"),
            labels: &["event"],
            ..Part::default()
        },
        Part {
            uid: "d3",
            title: "Reply to the landlord about the rent review",
            status: todo,
            due: d(-2),
            who: Some("Anna Becker"),
            priority: Some(2),
            ..Part::default()
        },
        Part {
            uid: "d4",
            title: "Review the draft of the onboarding handbook",
            status: go,
            due: d(0),
            who: Some("Chiara Rossi"),
            labels: &["portal"],
            ..Part::default()
        },
        Part {
            uid: "r1",
            title: "Order new business cards for the team",
            status: todo,
            due: d(3),
            who: Some("Eva Lindqvist"),
            ..Part::default()
        },
        Part {
            uid: "r2",
            title: "Plan the autumn team evening",
            status: todo,
            due: d(9),
            who: Some("Dmitri Volkov"),
            labels: &["office"],
            ..Part::default()
        },
        Part {
            uid: "r3",
            title: "Replace the broken desk lamp in room 2",
            status: todo,
            ..Part::default()
        },
        Part {
            uid: "r4",
            title: "Update the backup and restore notes",
            status: go,
            due: d(5),
            who: Some("Ben Okafor"),
            labels: &["portal", "infrastructure"],
            ..Part::default()
        },
        Part {
            uid: "r5",
            title: "Call the printer about the event flyers",
            status: todo,
            due: d(1),
            who: Some("Eva Lindqvist"),
            priority: Some(5),
            ..Part::default()
        },
        Part {
            uid: "r6",
            title: "Sort and caption the photos of the summer fair",
            status: todo,
            due: d(14),
            labels: &["office"],
            ..Part::default()
        },
        Part {
            uid: "a1",
            title: "Write the specification for the customer portal",
            status: "COMPLETED",
            due: d(-6),
            who: Some("Chiara Rossi"),
            labels: &["portal"],
            ..Part::default()
        },
        Part {
            uid: "a2",
            title: "Build the first version of the portal",
            status: go,
            due: d(4),
            who: Some("Ben Okafor"),
            waits_for: &["a1"],
            ..Part::default()
        },
        Part {
            uid: "a3",
            title: "Test the portal with three pilot customers",
            status: todo,
            due: d(7),
            who: Some("Anna Becker"),
            waits_for: &["a2"],
            ..Part::default()
        },
        Part {
            uid: "a4",
            title: "Publish the release notes and announce the portal",
            status: todo,
            due: d(8),
            who: Some("Eva Lindqvist"),
            waits_for: &["a3"],
            labels: &["portal"],
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
            title: "Confirm the catering for the autumn fair",
            status: todo,
            due: d(2),
            who: Some("Dmitri Volkov"),
            waits_for: &["b1"],
            ..Part::default()
        },
        Part {
            uid: "u1",
            title: "Ask legal about the new terms of service",
            status: todo,
            due: d(6),
            who: Some("Anna Becker"),
            waits_for: &["unknown-ticket"],
            ..Part::default()
        },
        Part {
            uid: "c1",
            title: "Agree on the budget for next year",
            status: todo,
            due: d(1),
            waits_for: &["c2"],
            ..Part::default()
        },
        Part {
            uid: "c2",
            title: "Agree on the schedule for next year",
            status: todo,
            due: d(1),
            waits_for: &["c1"],
            ..Part::default()
        },
        Part {
            uid: "f1",
            title: "Renew the domain registration",
            status: "COMPLETED",
            due: d(-9),
            who: Some("Ben Okafor"),
            labels: &["infrastructure"],
            ..Part::default()
        },
        Part {
            uid: "f2",
            title: "Pay the hosting bill for the quarter",
            status: "COMPLETED",
            due: d(-4),
            who: Some("Anna Becker"),
            labels: &["finance"],
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
        let title = ERRANDS[usize::from(i)].to_string();
        let uid = format!("e{i}");
        let wait = [if i % 4 == 3 { "a3" } else { "a1" }];
        let part = Part {
            uid: &uid,
            title: &title,
            status: todo,
            due: if i % 3 == 0 { d(i64::from(i)) } else { None },
            who: [Some("Eva Lindqvist"), None, Some("Ben Okafor")][usize::from(i) % 3],
            waits_for: if i % 4 == 3 { &wait } else { &[] },
            ..Part::default()
        };
        push(&mut out, &part);
    }
    out.push_str("END:VCALENDAR\r\n");
    out
}
