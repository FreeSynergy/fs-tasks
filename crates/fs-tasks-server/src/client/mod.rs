//! The window's side of the socket: `TaskItems` over `tasks.capnp`, with the change feed.
//!
//! Pattern: Facade — the pieces live in `connection` (proxy thread, change feed), `items` (the `TaskItems` impl),
//! `decode` and `walk`; only the two public types are re-exported.

mod connection;
mod decode;
mod items;
mod walk;

pub use connection::{Changes, RemoteItems};
