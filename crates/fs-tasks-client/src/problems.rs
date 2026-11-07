//! The problems of the server as the errors of the core.
//!
//! Pattern: Adapter — the inverse of `errors.rs` of `fs-tasks-server`: each RFC 9457 type becomes the
//! variant of `fs_tasks_core::Error` it came from, so the frontend's own reading of an error needs no
//! change. The wire carries a type, a status and a text, not the variant's fields; what can be read
//! back from the text is (all of it in `detail.rs`, behind one function), the rest comes from the call ([`Subject`]). The generic readers (type prefix, id in a text, store prefix, `Unsupported`, the `Failure` of an RPC error) are those of `fs_interfaces::rpc::client::problems`.

use fs_interfaces::Unsupported;
use fs_interfaces::rpc::Problem;
use fs_interfaces::rpc::client::problems::{Failure, local_type, unsupported_of};
use fs_objects::{Error as StoreError, ObjectId};
use fs_tasks_core::Error;

use crate::detail::read_detail;

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
    match Failure::of(e) {
        Failure::Problem(problem) => from_problem(&problem, subject),
        Failure::Unimplemented => Unsupported::new(
            "tasks",
            "the server does not know this method of the contract",
        )
        .into(),
        Failure::Other(text) => backend(format!("tasks server: {text}")),
    }
}

pub(crate) fn backend(text: String) -> Error {
    Error::Store(StoreError::Backend(text))
}

fn from_problem(p: &Problem, subject: Subject) -> Error {
    let detail = p.detail.as_str();
    if p.is_unsupported() {
        return unsupported_of(detail, "tasks").into();
    }
    match local_type(p) {
        "invalid-request" => Error::Store(StoreError::Invalid(detail.to_string())),
        kind => read_detail(kind, detail, subject)
            .unwrap_or_else(|| backend(format!("tasks server ({}): {detail}", p.status))),
    }
}
