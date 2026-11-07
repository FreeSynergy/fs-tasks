//! Reading the words of a problem back into the fields of the core's errors.
//!
//! Pattern: none — one pure function (`read_detail`) and the helpers behind it, for one concept: the
//! wire carries a type, a status and a text, and these errors need what only the text holds (the
//! issues of an invalid task, the size of a too large one, the `conflict:` marker, the id in a
//! not-found). All of it is here, behind that one function, so that the tag `problem-ext` (which
//! brings these as fields) replaces this file and nothing else.

use fs_interfaces::rpc::client::problems::{object_id_in, without_store_prefix};
use fs_store::Error as StoreError;
use fs_tasks_core::{Error, Issue};

use crate::problems::{Subject, backend};

/// The error for a problem of `kind` (`tasks/not-found`, `tasks/conflict`, `tasks/invalid`,
/// `too-large`) from its `detail`; `None` for any other kind.
pub(crate) fn read_detail(kind: &str, detail: &str, subject: Subject) -> Option<Error> {
    Some(match kind {
        "tasks/not-found" => not_found(detail, subject),
        "tasks/conflict" => conflict(detail, subject),
        "tasks/invalid" => invalid(detail),
        "too-large" => too_large(detail),
        _ => return None,
    })
}

/// `tasks/conflict`: the contract says a refused `editIf` has a reason starting `conflict:`; that
/// marker (not a sentence for people) tells it from the other conflicts.
fn conflict(detail: &str, subject: Subject) -> Error {
    match subject {
        Subject::Task(id) if detail.starts_with("conflict:") => Error::Changed(id),
        _ => Error::Conflict(detail.to_string()),
    }
}

/// `task <id> not found` names the task; if the text does not, the call does.
fn not_found(detail: &str, subject: Subject) -> Error {
    match (object_id_in(detail), subject) {
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
    Error::Store(StoreError::Invalid(
        without_store_prefix(detail).to_string(),
    ))
}

/// `/path: message; /other: message`. The core quotes the values it names (`{:?}`), so a `; ` inside a
/// quoted value does not end an issue; a piece that does not start with `/path: ` belongs to the
/// message before it. A key of an id map with `; /` in it (not quoted in the path) cannot be told
/// apart and splits.
fn issues(list: &str) -> Vec<Issue> {
    let mut out: Vec<Issue> = Vec::new();
    for piece in pieces(list) {
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

/// `list` cut at every `; ` outside a double-quoted (and backslash-escaped) value.
fn pieces(list: &str) -> Vec<&str> {
    let (mut out, mut start) = (Vec::new(), 0);
    let (mut quoted, mut escaped) = (false, false);
    for (at, c) in list.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            ';' if !quoted && list[at..].starts_with("; ") => {
                out.push(&list[start..at]);
                start = at + 2;
            }
            _ => {}
        }
    }
    out.push(&list[start..]);
    out
}

/// What was too large, by the three texts the server has: the core's task (`the task and its name have
/// N bytes`) and a request (`N bytes, at most M fit`) are `TooLarge(N)`; an answer
/// (`the answer has N bytes`) is no task of this call, and a text with no number is not guessed at:
/// both stay a store error with the server's words.
fn too_large(detail: &str) -> Error {
    let task = detail.strip_prefix("the task and its name have ");
    let request = detail.split_once(" bytes, at most ").map(|(n, _)| n);
    let number = task
        .and_then(|rest| rest.split_whitespace().next())
        .or(request.filter(|_| !detail.starts_with("the answer")))
        .and_then(|n| n.parse::<usize>().ok());
    match number {
        Some(n) => Error::TooLarge(n),
        None => backend(format!("tasks server: too large: {detail}")),
    }
}
