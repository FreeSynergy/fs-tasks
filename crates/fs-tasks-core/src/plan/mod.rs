//! Plan of a set of tasks: the dependency graph, readiness ("blocked" / "ready"), cycles and a schedule.
//!
//! Pattern: Facade (declarations) — everything here is a pure function over tasks and edges, with no store, so the
//! same code runs on stored tasks, on the tasks projected after a merge, and in tests. The direction of the
//! edges follows RFC 9253 (see [`graph`]).

mod graph;
mod readiness;
mod schedule;

pub use graph::{Edge, EdgeKind, cycles, edges_of, path_closing_cycle};
pub use readiness::{Readiness, readiness_of};
pub use schedule::{Item, Planned, ScheduleError, schedule};
