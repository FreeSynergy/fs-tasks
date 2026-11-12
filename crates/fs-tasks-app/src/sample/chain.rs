//! A long chain of tasks.
//!
//! Pattern: Builder — the same part, repeated.

use super::parts::{HEAD, Part, push};
use crate::day::Day;

/// A chain of `n` tasks: task `i + 1` depends on task `i`, so only the first is ready. Due dates are spread over
/// the days after `today`, a few people hold tasks.
#[must_use]
pub fn chain(n: usize, today: Day) -> String {
    let mut out = String::from(HEAD);
    let people = ["Anna", "Ben", "Chiara", "Dmitri", "Eva"];
    for i in 0..n {
        let (uid, title) = (format!("t{i}"), format!("Step {}", i + 1));
        let prev = [format!("t{}", i.wrapping_sub(1))];
        let prev_ref: Vec<&str> = if i == 0 {
            vec![]
        } else {
            vec![prev[0].as_str()]
        };
        let due_day = today.plus(i64::try_from(i % 30).unwrap_or(0));
        let part = Part {
            uid: &uid,
            title: &title,
            status: if i % 97 == 96 {
                "IN-PROCESS"
            } else {
                "NEEDS-ACTION"
            },
            due: Some((due_day, "1700")),
            who: Some(people[i % people.len()]),
            waits_for: &prev_ref,
            ..Part::default()
        };
        push(&mut out, &part);
    }
    out.push_str("END:VCALENDAR\r\n");
    out
}
