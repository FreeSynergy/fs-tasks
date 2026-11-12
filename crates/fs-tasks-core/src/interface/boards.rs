//! The `Boards` contract of the tasks program: boards of tasks (columns, the order of cards, layout).
//!
//! Pattern: Repository (interface) + Null Object — like `TaskItems`. A board is its own object; the workbench only shows
//! it and hands changes to the target the settings name (ADR 0066), it stores nothing itself. Not on the wire yet:
//! `tasks.capnp` has `Boards` as a placeholder without methods.

use async_trait::async_trait;
use fs_objects::{Envelope, ObjectId};

use super::items::not_possible;
use crate::error::Result;
use crate::model::{Board, Layout};
use crate::revision::Revision;

/// A stored board.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardEntry {
    /// The object id.
    pub id: ObjectId,
    /// Its envelope.
    pub envelope: Envelope,
    /// The board.
    pub board: Board,
}

impl BoardEntry {
    /// What the board was when it was read, for compare-and-set (equality only).
    pub fn revision(&self) -> Revision {
        Revision::of_parts(
            &self.envelope.updated,
            &serde_json::to_string(&self.board).unwrap_or_default(),
        )
    }
}

/// Boards of tasks.
#[async_trait]
pub trait Boards: Send + Sync {
    /// `board.create`: a board with the standard columns, on the tasks with `label` (every task if `None`).
    async fn board_create(&self, _name: &str, _label: Option<&str>) -> Result<BoardEntry> {
        not_possible("board.create")
    }
    /// `board.open`: the board, if it exists.
    async fn board_get(&self, _id: ObjectId) -> Result<Option<BoardEntry>> {
        not_possible("board.open")
    }
    /// `board.open`: all boards, by name.
    async fn boards(&self) -> Result<Vec<BoardEntry>> {
        not_possible("board.open")
    }
    /// `column.create`, `column.limit`: replace the board (columns, limits, places) if it still has `revision`
    /// ([`Error::Changed`](crate::Error::Changed) otherwise); a board that is not valid is [`Error::Rejected`](crate::Error::Rejected).
    async fn board_edit(
        &self,
        _id: ObjectId,
        _revision: Revision,
        _board: Board,
    ) -> Result<BoardEntry> {
        not_possible("column.create")
    }
    /// `card.move`: place a task's card in a column at an index (end if `None`). A full column does not refuse it; the
    /// layout reports the excess.
    async fn board_move_card(
        &self,
        _id: ObjectId,
        _uid: &str,
        _column: &str,
        _index: Option<usize>,
    ) -> Result<BoardEntry> {
        not_possible("card.move")
    }
    /// Delete the board (the tasks stay).
    async fn board_delete(&self, _id: ObjectId) -> Result<()> {
        not_possible("board.create")
    }
    /// `board.open`: the columns with their cards now (tasks by label or place; the column of an unplaced card from its
    /// progress) and which columns are over their limit.
    async fn board_layout(&self, _id: ObjectId) -> Result<Layout> {
        not_possible("board.open")
    }
}
