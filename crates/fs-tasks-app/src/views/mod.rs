//! The status board and the timeline of the tasks, as screens of the window (ADR 0049: views of `fs-tasks`).
//!
//! Pattern: Adapter — `fs-components-kanban` and `fs-components-timeline` draw; this module only maps `TaskItems` data
//! to their plain structs and their callbacks to the backend. No render code. The board is a view of `progress`
//! (the status view is not a board of columns: `DESIGN.md`); its column operations stay locked.

mod board;
mod dates;
mod surfaces;
mod timeline;

pub use board::TaskBoard;
pub use dates::today;
pub use surfaces::{BoardSurface, TimelineSurface};
pub use timeline::TaskTimeline;
