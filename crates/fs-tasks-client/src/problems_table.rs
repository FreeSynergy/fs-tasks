//! The table of problems, read back: every type of the server and the variant it must become.
//!
//! Pattern: Registry — the map of problem type to the variant it becomes is what is tested here, row
//! by row, for the types the real server cannot be made to send (too large, a text with `; ` inside);
//! the others are covered over the real socket.

use fs_interfaces::rpc::Problem;
use fs_store::{Error as StoreError, ObjectId};
use fs_tasks_core::Error;

use crate::problems::{BASE, Subject, error_of};

fn said(kind: &str, status: u16, detail: &str, subject: Subject) -> Error {
    let problem = Problem::new(format!("{BASE}{kind}"), "t", detail, status);
    error_of(&problem.into_error(), subject)
}

#[test]
fn the_table_of_the_server_comes_back() {
    let id = ObjectId::new();
    let none = Subject::None;
    let e = said("too-large", 413, "the answer has 99 bytes, at most 5", none);
    assert!(matches!(e, Error::TooLarge(99)), "{e:?}");
    let e = said("invalid-request", 422, "invalid cursor", none);
    assert!(matches!(e, Error::Store(StoreError::Invalid(_))), "{e:?}");
    let e = said("tasks/conflict", 409, "uid taken", none);
    assert!(
        matches!(&e, Error::Conflict(t) if t == "uid taken"),
        "{e:?}"
    );
    let e = said("tasks/not-found", 404, "something", Subject::Task(id));
    assert!(matches!(e, Error::NotFound(i) if i == id), "{e:?}");
    let e = said(
        "tasks/not-found",
        404,
        &format!("task {id} not found"),
        none,
    );
    assert!(
        matches!(e, Error::NotFound(i) if i == id),
        "named by the text: {e:?}"
    );
    let e = said("tasks/not-found", 404, "gone", none);
    assert!(matches!(e, Error::Store(StoreError::Backend(_))), "{e:?}");
    let e = said("tasks/invalid", 422, "time: no such zone", none);
    assert!(matches!(&e, Error::Time(t) if t == "no such zone"), "{e:?}");
    let e = said("tasks/invalid", 422, "iCalendar: no end", none);
    assert!(matches!(&e, Error::ICalendar(t) if t == "no end"), "{e:?}");
    let e = said("tasks/invalid", 422, "expected value at line 1", none);
    assert!(matches!(e, Error::Store(StoreError::Invalid(_))), "{e:?}");
    let e = said("whatever", 500, "boom", none);
    assert!(matches!(e, Error::Store(StoreError::Backend(_))), "{e:?}");
}

#[test]
fn unsupported_keeps_capability_and_reason() {
    let problem = Problem::new(
        "https://freesynergy.net/problems/unsupported",
        "t",
        "task.import: not here",
        501,
    );
    let e = error_of(&problem.into_error(), Subject::None);
    assert!(
        matches!(&e, Error::Unsupported(u) if u.capability() == "task.import" && u.reason() == "not here"),
        "{e:?}"
    );
}

#[test]
fn issues_come_back_as_the_core_said_them() {
    let text = "JSCalendar 2.0 violation: /progress: must be one of a; b; /due: bad";
    let e = said("tasks/invalid", 422, text, Subject::None);
    let Error::Invalid(issues) = &e else {
        panic!("{e:?}")
    };
    assert_eq!(issues.len(), 2, "a message with a semicolon stays whole");
    assert_eq!(issues[0].path, "/progress");
    assert_eq!(issues[0].message, "must be one of a; b");
    assert_eq!(e.to_string(), text);
}
