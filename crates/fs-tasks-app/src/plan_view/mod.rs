//! The two plan views of the tasks program: the board (A) and "what next" (B), over the same picture of the store.
//!
//! Pattern: Facade — `snapshot` (tasks and their readiness, judged once), `cache` (renewed away from the
//! interface thread), `look` (the words and rules both views share), `layout` (the shared source and the Strategy
//! each view fills in), `board` and `next`.

mod board;
mod cache;
mod layout;
mod look;
mod next;
mod snapshot;

pub use board::Board;
pub use cache::Cache;
pub use layout::{Layout, PlanSource};
pub use look::{Standing, Urgency, due_phrase, standing, urgency, why};
pub use next::Next;
pub use snapshot::Snapshot;
