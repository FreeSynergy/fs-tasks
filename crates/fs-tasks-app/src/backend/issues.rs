//! What went wrong, in the words of the front end.
//!
//! Pattern: Adapter between the error of the core and `BackendError`. The core lists its findings with a path into the
//! JSCalendar object (`/due`); the front end wants the field and a problem code. `TypeProblem` has no general "value
//! not accepted", so a finding without a closer match counts as `Character` (a value the field does not hold).

use fs_admin_frontend::BackendError;
use fs_tasks_core::{Error, Issue};
use fs_types::{RecordIssue, RecordProblem, TypeProblem};

/// A finding on `field`, value 0.
pub fn issue(field: &str, problem: TypeProblem) -> RecordIssue {
    RecordIssue {
        field: field.to_owned(),
        index: 0,
        problem: RecordProblem::Value(problem),
    }
}

fn of_core(i: &Issue) -> RecordIssue {
    let field = i
        .path
        .trim_start_matches('/')
        .split('/')
        .next()
        .unwrap_or("");
    let problem = match field {
        "progress" => TypeProblem::NotInList,
        "due" | "start" => TypeProblem::BadDateTime,
        _ => TypeProblem::Character,
    };
    issue(field, problem)
}

/// The refusal for these findings of the core.
pub fn rejected(issues: &[Issue]) -> BackendError {
    BackendError::Rejected(issues.iter().map(of_core).collect())
}

/// The error of the core as the error of the front end.
pub fn backend_error(e: &Error) -> BackendError {
    match e {
        Error::Invalid(issues) => rejected(issues),
        Error::NotFound(_) => BackendError::NotFound,
        Error::Changed(_) => BackendError::Unavailable(
            "changed: the task was changed by someone else since it was read".into(),
        ),
        other => BackendError::Unavailable(other.to_string()),
    }
}
