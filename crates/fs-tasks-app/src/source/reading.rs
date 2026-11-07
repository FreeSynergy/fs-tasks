//! `Reading`: where "show more" stands in the list of one search.
//!
//! Pattern: Iterator with State — a [`PageWalk`] (the cursor) plus the part of the last page that was
//! not yet shown (`carry`) and how many rows were shown (`served`). The workbench asks by
//! `offset`/`limit`; a request that starts at `served` or inside the rows just given continues from
//! the cursor, any other one starts the walk again and skips. The search term and "open only" are
//! the app's own reading of a task (the contract filters by due date, progress and label, not text),
//! so they are applied to the rows while the walk goes on (`DESIGN.md`, limits).

use std::collections::VecDeque;
use std::sync::Arc;

use fs_components_workbenches::tasks::RowQuery;
use fs_tasks_client::{PageWalk, Revised, TaskPages};
use fs_tasks_core::{Result, TaskQuery};

use super::rows::{is_open, matches};

/// The reading of one search.
pub(crate) struct Reading {
    pub(crate) query: RowQuery,
    term: String,
    pages: Arc<dyn TaskPages>,
    walk: PageWalk,
    carry: VecDeque<Revised>,
    /// The rows of the last call, which end at `served`: the workbench asks for one row more than
    /// it shows and asks on from what it shows, so the next call may start inside them.
    kept: Vec<Revised>,
    served: usize,
}

impl Reading {
    /// At the start of the list of `query`.
    pub(crate) fn new(pages: Arc<dyn TaskPages>, query: &RowQuery) -> Self {
        Self {
            query: query.clone(),
            term: query.term.to_lowercase(),
            walk: PageWalk::new(pages.clone(), TaskQuery::new()),
            pages,
            carry: VecDeque::new(),
            kept: Vec::new(),
            served: 0,
        }
    }

    /// Does the search want this task?
    fn wants(&self, read: &Revised) -> bool {
        let task = &read.entry.task;
        matches(task, &self.term) && (!self.query.open_only || is_open(task))
    }

    /// The next wanted row, reading a page only when the last one is used up.
    async fn pop(&mut self) -> Result<Option<Revised>> {
        loop {
            while let Some(read) = self.carry.pop_front() {
                if self.wants(&read) {
                    return Ok(Some(read));
                }
            }
            match self.walk.next_page().await? {
                Some(page) => self.carry.extend(page.items),
                None => return Ok(None),
            }
        }
    }

    /// Put back the last `back` rows delivered, so they are delivered again.
    fn rewind(&mut self, back: usize) {
        let from = self.kept.len() - back;
        for row in self.kept.drain(from..).rev() {
            self.carry.push_front(row);
        }
        self.served -= back;
    }

    /// `limit` rows from `offset`. Continues from the cursor when `offset` is where the last call
    /// ended, or inside the rows the last call delivered (they are given again); from 0 or anywhere
    /// else it starts again (a reload must see changes) and skips.
    pub(crate) async fn take(&mut self, offset: usize, limit: usize) -> Result<Vec<Revised>> {
        let back = self.served.saturating_sub(offset);
        if offset > 0 && back > 0 && back <= self.kept.len() {
            self.rewind(back);
        } else if offset == 0 || offset != self.served {
            *self = Self::new(self.pages.clone(), &self.query.clone());
            while self.served < offset && self.pop().await?.is_some() {
                self.served += 1;
            }
            if self.served < offset {
                return Ok(Vec::new());
            }
        }
        let mut rows = Vec::new();
        while rows.len() < limit {
            let Some(row) = self.pop().await? else { break };
            rows.push(row);
        }
        self.served += rows.len();
        if !rows.is_empty() {
            self.kept.clone_from(&rows);
        }
        Ok(rows)
    }
}
