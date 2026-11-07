//! `PageWalk`: the pages of a query, one `next_page` after the other.
//!
//! Pattern: Iterator (async, so not `std::iter::Iterator`) over [`TaskPages`], with State: the
//! walk is at the start, at a cursor, or at the end.

use std::sync::Arc;

use fs_tasks_core::{Result, TaskQuery};

use crate::pages::{TaskPage, TaskPages};

/// A walk over the pages of one query. Dropping it forgets the place; a new query is a new walk.
pub struct PageWalk {
    _pages: Arc<dyn TaskPages>,
    _query: TaskQuery,
}

impl PageWalk {
    /// A walk at the start of the list `query` selects.
    #[must_use]
    pub fn new(pages: Arc<dyn TaskPages>, query: TaskQuery) -> Self {
        Self {
            _pages: pages,
            _query: query,
        }
    }

    /// The next page, or `None` after the last. After an error the walk stays where it was.
    ///
    /// # Errors
    /// As [`TaskPages::first`] and [`TaskPages::next`].
    #[allow(clippy::unused_async)]
    pub async fn next_page(&mut self) -> Result<Option<TaskPage>> {
        todo!("step 3: walk")
    }
}
