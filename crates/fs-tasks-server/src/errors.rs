//! Errors of the core as RPC errors.
//!
//! Pattern: Adapter — each error the core can name becomes a typed RFC 9457 problem, so a consumer
//! can tell "not found" from "not possible" without reading a message.

use fs_interfaces::rpc::{Problem, failed, problem_keys as keys, too_large_by, unsupported};
use fs_tasks_core::{Error, MAX_TASK_BYTES};

use crate::facts::with_facts;

const NOT_FOUND: &str = "https://freesynergy.net/problems/tasks/not-found";
const CONFLICT: &str = "https://freesynergy.net/problems/tasks/conflict";
const INVALID: &str = "https://freesynergy.net/problems/tasks/invalid";

/// A core error as an RPC error. `Json` means data already stored no longer reads: not the caller's
/// fault. What a client sends is read before the core sees it (`wire::read_task`).
#[allow(clippy::needless_pass_by_value)] // used as `map_err(rpc_error)`
pub(crate) fn rpc_error(e: Error) -> capnp::Error {
    use fs_objects::Error as Store;
    let text = e.to_string();
    let problem = |kind: &str, title: &str, status: u16| {
        with_facts(Problem::new(kind, title, text.clone(), status), &e).into_error()
    };
    match &e {
        Error::Unsupported(not_possible) => unsupported(not_possible),
        Error::NotFound(_) | Error::Store(Store::NotFound(_)) => {
            problem(NOT_FOUND, "Task not found", 404)
        }
        Error::Conflict(_) | Error::Changed(_) | Error::Store(Store::Conflict(_)) => {
            problem(CONFLICT, "Conflict", 409)
        }
        Error::Invalid(_)
        | Error::Time(_)
        | Error::ICalendar(_)
        | Error::Store(Store::Invalid(_)) => problem(INVALID, "Invalid task", 422),
        Error::TooLarge(bytes) => too_large_by(text.clone(), *bytes as u64, MAX_TASK_BYTES as u64),
        Error::Json(_) | Error::Stored(_) | Error::Store(_) => failed(text.clone()),
    }
}

/// The request itself is not acceptable: `detail` for people, `issue` for a program (the field or
/// the reason), as the extension `issue`.
pub(crate) fn invalid(detail: impl Into<String>, issue: &str) -> capnp::Error {
    Problem::new(INVALID, "Invalid task", detail, 422)
        .with_ext(keys::ISSUE, issue)
        .into_error()
}
