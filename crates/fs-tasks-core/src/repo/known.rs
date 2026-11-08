//! The `uid`s of the stored tasks, read once for an import.
//!
//! Pattern: Value object — a set of `uid`s with the two things an import asks of it. Why it exists:
//! the store answers "is this `uid` taken" by looking at every task of the type (a lookup by an index
//! field is an `EXISTS` for each row), so asking for each task of an import costs the square of
//! the count. The set is read in pages with the cheapest query there is (no condition), once, and kept
//! up to date by the import itself, which holds the write lock (`writes`).

use std::collections::HashSet;

use fs_objects::{Order, Query};

use super::consts::{CONTEXT, TASK_TYPE};
use super::handle::TaskRepo;
use crate::error::Result;

/// Tasks read per query while the set is built; the memory held at once is this many tasks.
const PAGE: usize = 500;

/// The `uid`s that are stored, and those an import has stored since.
#[derive(Debug, Default)]
pub(super) struct KnownUids(HashSet<String>);

impl KnownUids {
    /// Is a task with this `uid` stored?
    pub(super) fn contains(&self, uid: &str) -> bool {
        self.0.contains(uid)
    }

    /// A task with this `uid` was stored.
    pub(super) fn add(&mut self, uid: String) {
        self.0.insert(uid);
    }
}

impl TaskRepo {
    /// The `uid` index values of all stored tasks, read in pages of [`PAGE`] (oldest first, so a page
    /// does not move while nothing is written; the caller holds the write lock).
    pub(super) async fn known_uids(&self) -> Result<KnownUids> {
        let mut known = KnownUids::default();
        let mut offset = 0;
        loop {
            let page = self
                .queries
                .query(
                    &Query::new()
                        .of_type(TASK_TYPE)
                        .in_context(CONTEXT)
                        .ordered(Order::UpdatedAsc)
                        .limit(PAGE)
                        .offset(offset),
                )
                .await?;
            let read = page.len();
            for object in page {
                for uid in object.index.get("uid").into_iter().flatten() {
                    known.add(uid.clone());
                }
            }
            if read < PAGE {
                return Ok(known);
            }
            offset += read;
        }
    }
}
