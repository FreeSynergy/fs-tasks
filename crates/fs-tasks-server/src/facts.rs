//! The facts behind a problem, as typed extensions.
//!
//! Pattern: none (a function that only computes from its argument). What a client used to cut out
//! of the detail text (`coding.md` §2a) is stated here as `id`, `kind` and `issue`; the detail text
//! stays as it was. The table is in `DESIGN.md`.

use fs_interfaces::rpc::{Problem, problem_keys as keys};
use fs_objects::Error as StoreError;
use fs_tasks_core::Error;

/// `kind` of a task.
pub(crate) const TASK: &str = "task";
/// `kind` of a time, duration or time zone that could not be handled.
pub(crate) const TIME: &str = "time";
/// `kind` of an iCalendar text that could not be read or written.
pub(crate) const ICALENDAR: &str = "icalendar";

/// Extension of a cycle problem: the `uid`s of the way round, one value each.
pub(crate) const PATH: &str = "path";
/// Most `uid`s of a cycle one problem names (an extension list is bounded; the rest is in the detail text).
const MAX_PATH: usize = 32;

/// `problem` with every fact the core error `e` knows.
pub(crate) fn with_facts(problem: Problem, e: &Error) -> Problem {
    match e {
        Error::NotFound(id) | Error::Changed(id) | Error::Exists(id) => problem
            .with_ext(keys::ID, &id.to_string())
            .with_ext(keys::KIND, TASK),
        // One extension `path` for every uid of the cycle, in order (as many as an answer can carry).
        Error::Cycle(path) => path
            .iter()
            .take(MAX_PATH)
            .fold(problem.with_ext(keys::KIND, TASK), |p, uid| {
                p.with_ext(PATH, uid)
            }),
        Error::Store(StoreError::NotFound(id)) => problem.with_ext(keys::ID, &id.to_string()),
        Error::Invalid(issues) => issues.iter().fold(problem, |p, issue| {
            p.with_ext(keys::ISSUE, &issue.to_string())
        }),
        Error::Time(reason) => problem
            .with_ext(keys::KIND, TIME)
            .with_ext(keys::ISSUE, reason),
        Error::ICalendar(reason) => problem
            .with_ext(keys::KIND, ICALENDAR)
            .with_ext(keys::ISSUE, reason),
        Error::Store(StoreError::Invalid(reason)) => problem.with_ext(keys::ISSUE, reason),
        _ => problem,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::rpc_error;

    fn problem(e: Error) -> Problem {
        Problem::from_error(&rpc_error(e)).expect("a problem")
    }

    #[test]
    fn a_time_and_an_icalendar_error_say_what_they_are_about_without_the_text_prefix() {
        let p = problem(Error::Time("no such zone".into()));
        assert_eq!(p.ext("kind"), Some("time"));
        assert_eq!(p.ext("issue"), Some("no such zone"));
        assert_eq!(p.detail, "time: no such zone");
        let p = problem(Error::ICalendar("no VTODO".into()));
        assert_eq!(p.ext("kind"), Some("icalendar"));
        assert_eq!(p.ext("issue"), Some("no VTODO"));
        assert_eq!(p.detail, "iCalendar: no VTODO");
    }

    #[test]
    fn a_store_refusal_gives_its_reason() {
        let p = problem(Error::Store(StoreError::Invalid("name is empty".into())));
        assert_eq!(p.ext("issue"), Some("name is empty"));
    }
}
