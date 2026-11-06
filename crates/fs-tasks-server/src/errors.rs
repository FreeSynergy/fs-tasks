//! Errors of the core as RPC errors.
//!
//! Pattern: Adapter — each error the core can name becomes a typed RFC 9457 problem, so a consumer
//! can tell "not found" from "not possible" without reading a message.

use fs_interfaces::rpc::{Problem, failed, unsupported};
use fs_tasks_core::Error;

const NOT_FOUND: &str = "https://freesynergy.net/problems/tasks/not-found";
const CONFLICT: &str = "https://freesynergy.net/problems/tasks/conflict";
const INVALID: &str = "https://freesynergy.net/problems/tasks/invalid";

/// A core error as an RPC error.
pub(crate) fn rpc_error(e: Error) -> capnp::Error {
    let text = e.to_string();
    match e {
        Error::Unsupported(not_possible) => unsupported(&not_possible),
        Error::NotFound(_) => Problem::new(NOT_FOUND, "Task not found", text, 404).into_error(),
        Error::Conflict(_) => Problem::new(CONFLICT, "Conflict", text, 409).into_error(),
        Error::Invalid(_) | Error::Time(_) | Error::ICalendar(_) | Error::Json(_) => invalid(text),
        Error::Store(_) => failed(text),
    }
}

/// The request itself is not acceptable.
pub(crate) fn invalid(detail: impl Into<String>) -> capnp::Error {
    Problem::new(INVALID, "Invalid task", detail, 422).into_error()
}
