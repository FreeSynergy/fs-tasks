//! The Cap'n Proto contract `tasks.capnp` in fs-interfaces (draft) and its conversions: module map.
//!
//! Pattern: Adapter (declarations) — the generated code (`tasks_capnp`, `common_capnp` at the crate
//! root, committed, ADR 0040) is wrapped by `convert`, which maps wire types to the Rust
//! interface types. Interface status: draft (ADR 0045).

mod convert;

pub use convert::{
    read_filter, relation_from_wire, relation_to_wire, write_capabilities, write_filter,
};
