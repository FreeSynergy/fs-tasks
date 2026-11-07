//! `PageWalk`: the pages of a query, one `next_page` after the other.
//!
//! Pattern: Iterator (async, so not `std::iter::Iterator`) over [`TaskPages`], with State: the
//! walk is at the start, at a cursor, or at the end.

use std::sync::Arc;

use fs_tasks_core::{Result, TaskQuery};

use crate::pages::{Cursor, TaskPage, TaskPages};

/// Where the walk stands.
enum Place {
    Start,
    At(Cursor),
    End,
}

/// A walk over the pages of one query. Dropping it forgets the place; a new query is a new walk.
pub struct PageWalk {
    pages: Arc<dyn TaskPages>,
    query: TaskQuery,
    place: Place,
}

impl PageWalk {
    /// A walk at the start of the list `query` selects.
    #[must_use]
    pub fn new(pages: Arc<dyn TaskPages>, query: TaskQuery) -> Self {
        Self {
            pages,
            query,
            place: Place::Start,
        }
    }

    /// The next page, or `None` after the last. After an error the walk stays where it was.
    ///
    /// # Errors
    /// As [`TaskPages::first`] and [`TaskPages::next`].
    pub async fn next_page(&mut self) -> Result<Option<TaskPage>> {
        let page = match &self.place {
            Place::End => return Ok(None),
            Place::Start => self.pages.first(&self.query).await?,
            Place::At(cursor) => self.pages.next(&self.query, cursor).await?,
        };
        self.place = page.next.clone().map_or(Place::End, Place::At);
        Ok(Some(page))
    }
}
