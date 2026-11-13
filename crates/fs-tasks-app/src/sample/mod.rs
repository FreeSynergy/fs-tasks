//! Sample tasks for pictures, tests and speed checks: a small believable set that has every case the two views show,
//! and a long chain.
//!
//! Pattern: Facade — declarations and re-exports only; the data takes the road of real data (an iCalendar import).

mod chain;
mod demo;
mod named;
mod parts;

pub use chain::chain;
pub use demo::demo;
pub use named::by_name;
