//! The problems of the server as the errors of the core.
//!
//! Pattern: Adapter — the inverse of `errors.rs` of `fs-tasks-server`: each RFC 9457 type becomes the
//! variant of `fs_tasks_core::Error` it came from, so the frontend's own reading of an error needs no
//! change. The wire carries a type, a status and a text, not the variant's fields; what can be read
//! back from the text is, the rest comes from the call ([`Subject`]).

use fs_interfaces::Unsupported;
use fs_interfaces::rpc::{Problem, is_unimplemented};
use fs_store::{Error as StoreError, ObjectId};
use fs_tasks_core::{Error, Issue};

pub(crate) const BASE: &str = "https://freesynergy.net/problems/";

/// What a call was about, for the variants that name it.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Subject {
    /// Nothing in particular (a list, an import).
    None,
    /// This task.
    Task(ObjectId),
}

/// The error of the core for what the server (or the wire) said.
pub(crate) fn error_of(e: &capnp::Error, subject: Subject) -> Error {
    match Problem::from_error(e) {
        Some(problem) => from_problem(&problem, subject),
        None if is_unimplemented(e) => Unsupported::new(
            "tasks",
            "the server does not know this method of the contract",
        )
        .into(),
        None => backend(format!("tasks server: {e}")),
    }
}

fn backend(text: String) -> Error {
    Error::Store(StoreError::Backend(text))
}

fn from_problem(p: &Problem, subject: Subject) -> Error {
    let detail = p.detail.as_str();
    if p.is_unsupported() {
        return unsupported(detail);
    }
    let kind = p.problem_type.strip_prefix(BASE).unwrap_or(&p.problem_type);
    match kind {
        "tasks/not-found" => not_found(detail, subject),
        "tasks/conflict" => Error::Conflict(detail.to_string()),
        "tasks/invalid" => invalid(detail),
        "too-large" => Error::TooLarge(first_number(detail)),
        "invalid-request" => Error::Store(StoreError::Invalid(detail.to_string())),
        _ => backend(format!("tasks server ({}): {detail}", p.status)),
    }
}

fn unsupported(detail: &str) -> Error {
    let (capability, reason) = detail.split_once(": ").unwrap_or(("tasks", detail));
    Unsupported::new(capability, reason).into()
}

/// `task <id> not found` names the task; if the text does not, the call does.
fn not_found(detail: &str, subject: Subject) -> Error {
    let named = detail
        .split_whitespace()
        .find_map(|word| word.parse::<ObjectId>().ok());
    match (named, subject) {
        (Some(id), _) | (None, Subject::Task(id)) => Error::NotFound(id),
        (None, Subject::None) => backend(format!("tasks server: something is missing: {detail}")),
    }
}

/// The three texts the core's errors have, read back into their variants; any other text the
/// server found invalid stays a text.
fn invalid(detail: &str) -> Error {
    if let Some(list) = detail.strip_prefix("JSCalendar 2.0 violation: ") {
        return Error::Invalid(issues(list));
    }
    if let Some(what) = detail.strip_prefix("time: ") {
        return Error::Time(what.to_string());
    }
    if let Some(what) = detail.strip_prefix("iCalendar: ") {
        return Error::ICalendar(what.to_string());
    }
    Error::Store(StoreError::Invalid(detail.to_string()))
}

/// `/path: message; /other: message`. A piece that does not start a new `/path: ` belongs to the
/// message before it, which had a `; ` of its own.
fn issues(list: &str) -> Vec<Issue> {
    let mut out: Vec<Issue> = Vec::new();
    for piece in list.split("; ") {
        match piece.split_once(": ") {
            Some((path, message)) if path.starts_with('/') || out.is_empty() => {
                out.push(Issue {
                    path: path.to_string(),
                    message: message.to_string(),
                });
            }
            _ => {
                if let Some(last) = out.last_mut() {
                    last.message.push_str("; ");
                    last.message.push_str(piece);
                }
            }
        }
    }
    out
}

/// The first run of digits, 0 if there is none.
fn first_number(text: &str) -> usize {
    let digits: String = text
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().unwrap_or(0)
}
