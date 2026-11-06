//! From the program's contract to the workbench's source.
//!
//! Pattern: Adapter — `adapter.rs` drives the async contract from the interface thread, `provider.rs`
//! is the source itself, `rows.rs` and `sheet.rs` show a task, `draft.rs` makes a form of it, `apply.rs`
//! writes a form back onto it, `marks.rs` tells whether it is marked sensitive.

mod adapter;
mod apply;
mod draft;
mod marks;
mod provider;
mod rows;
mod sheet;

pub use adapter::TaskAdapter;
pub(crate) use adapter::reason_text;
pub use apply::apply_draft;
pub use draft::is_series;
pub use marks::is_marked;
