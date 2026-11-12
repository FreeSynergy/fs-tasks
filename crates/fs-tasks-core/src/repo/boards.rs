//! Boards as stored objects.
//!
//! Pattern: Repository — `TaskRepo` is the provider of the `Boards` contract. A board is an object of type `Board` in the
//! context of the tasks; a query of tasks (type `Task`) does not see it. Writes take the same turn as task writes.

use fs_objects::{Envelope, ObjectId, Order, Query, StoredObject};

use super::consts::CONTEXT;
use super::handle::TaskRepo;
use crate::error::{Error, Result};
use crate::interface::{BoardEntry, TaskQuery};
use crate::model::{Board, BoardTask, Layout};

/// AS2-like type of a board object (AS2 has none).
pub(super) const BOARD_TYPE: &str = "Board";
const CONTENT_TYPE: &str = "application/vnd.freesynergy.board+json";
/// Most boards a store holds.
const MAX_BOARDS: usize = 1000;

fn read(object: StoredObject) -> Result<BoardEntry> {
    let board: Board = serde_json::from_value(object.content)
        .map_err(|e| Error::Stored(format!("board {}: {e}", object.envelope.id)))?;
    Ok(BoardEntry {
        id: object.envelope.id,
        envelope: object.envelope,
        board,
    })
}

fn checked(board: &Board) -> Result<()> {
    match board.problem() {
        Some(p) => Err(Error::Rejected(p)),
        None => Ok(()),
    }
}

impl TaskRepo {
    async fn board_object(&self, id: ObjectId) -> Result<Option<StoredObject>> {
        Ok(self
            .objects
            .get(id)
            .await?
            .filter(|o| o.envelope.object_type == BOARD_TYPE))
    }

    pub(super) async fn board_create_inner(
        &self,
        name: &str,
        label: Option<&str>,
    ) -> Result<BoardEntry> {
        let mut board = Board::standard(name.trim());
        board.label = label.map(str::to_string).filter(|l| !l.trim().is_empty());
        checked(&board)?;
        let _one_at_a_time = self.writes.lock().await;
        let count = self
            .queries
            .count(&Query::new().of_type(BOARD_TYPE).in_context(CONTEXT))
            .await?;
        if count >= MAX_BOARDS {
            return Err(Error::Conflict(format!(
                "a store has at most {MAX_BOARDS} boards"
            )));
        }
        let mut env = Envelope::new(BOARD_TYPE, board.name.clone());
        env.iri = Some(format!("https://{}/boards/{}", self.instance, env.id));
        env.context = Some(CONTEXT.into());
        env.attributed_to = Some(self.actor.clone());
        env.to = vec![self.actor.clone()];
        let object = StoredObject::new(env, CONTENT_TYPE, serde_json::to_value(&board)?);
        let id = object.envelope.id;
        self.objects.put(object, &self.actor).await?;
        read(self.board_object(id).await?.ok_or(Error::NotFound(id))?)
    }

    pub(super) async fn board_get_inner(&self, id: ObjectId) -> Result<Option<BoardEntry>> {
        self.board_object(id).await?.map(read).transpose()
    }

    pub(super) async fn boards_inner(&self) -> Result<Vec<BoardEntry>> {
        let q = Query::new()
            .of_type(BOARD_TYPE)
            .in_context(CONTEXT)
            .ordered(Order::PublishedAsc);
        let mut all: Vec<BoardEntry> = self
            .queries
            .query(&q)
            .await?
            .into_iter()
            .map(read)
            .collect::<Result<_>>()?;
        all.sort_by(|a, b| {
            a.board
                .name
                .to_lowercase()
                .cmp(&b.board.name.to_lowercase())
        });
        Ok(all)
    }

    /// Replace the board with `board` as the caller holds the write turn, after `change` was applied to what is stored.
    async fn write_board(
        &self,
        id: ObjectId,
        mut object: StoredObject,
        board: &Board,
    ) -> Result<BoardEntry> {
        checked(board)?;
        object.envelope.name = board.name.clone();
        object.content = serde_json::to_value(board)?;
        self.objects.put(object, &self.actor).await?;
        read(self.board_object(id).await?.ok_or(Error::NotFound(id))?)
    }

    pub(super) async fn board_edit_inner(
        &self,
        id: ObjectId,
        revision: crate::Revision,
        board: crate::model::Board,
    ) -> Result<BoardEntry> {
        let _one_at_a_time = self.writes.lock().await;
        let object = self.board_object(id).await?.ok_or(Error::NotFound(id))?;
        if read(object.clone())?.revision() != revision {
            return Err(Error::Changed(id));
        }
        self.write_board(id, object, &board).await
    }

    pub(super) async fn board_move_card_inner(
        &self,
        id: ObjectId,
        uid: &str,
        column: &str,
        index: Option<usize>,
    ) -> Result<BoardEntry> {
        let _one_at_a_time = self.writes.lock().await;
        let object = self.board_object(id).await?.ok_or(Error::NotFound(id))?;
        let mut board = read(object.clone())?.board;
        board
            .move_card(uid, column, index)
            .map_err(Error::Rejected)?;
        self.write_board(id, object, &board).await
    }

    pub(super) async fn board_delete_inner(&self, id: ObjectId) -> Result<()> {
        let _one_at_a_time = self.writes.lock().await;
        self.board_object(id).await?.ok_or(Error::NotFound(id))?;
        self.objects.delete(id, &self.actor).await?;
        Ok(())
    }

    pub(super) async fn board_layout_inner(&self, id: ObjectId) -> Result<Layout> {
        let board = self
            .board_get_inner(id)
            .await?
            .ok_or(Error::NotFound(id))?
            .board;
        let mut tasks = match &board.label {
            Some(label) => {
                self.query_inner(&TaskQuery::new().with_label(label))
                    .await?
            }
            None => self.query_inner(&TaskQuery::new()).await?,
        };
        // the placed cards are on the board whatever their labels say
        let placed: Vec<String> = board
            .order
            .values()
            .flatten()
            .filter(|u| !tasks.iter().any(|t| &t.task.uid == *u))
            .cloned()
            .collect();
        tasks.extend(self.entries_by_uids(&placed).await?);
        let cards: Vec<BoardTask> = tasks
            .iter()
            .map(|t| BoardTask {
                uid: t.task.uid.clone(),
                progress: t.task.effective_progress().to_string(),
            })
            .collect();
        Ok(board.layout(&cards))
    }
}
