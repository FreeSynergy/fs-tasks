//! The board of a set of tasks: columns, the order of the cards in them, and a layout.
//!
//! Pattern: Value object + pure functions — a board is its own object (never a member of a task, so a task can be on
//! several boards). A card is a task `uid`. The column of a card without a place of its own comes from `progress`
//! (a column lists the progress values it stands for); a place in `order` wins over it. A limit on the cards of a column
//! (WIP) is only reported by the layout, never enforced: nothing is refused because a column is full.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// One column of a board.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Column {
    /// Stable id inside the board.
    pub id: String,
    /// The name shown.
    pub name: String,
    /// The most cards the column should hold; more is reported, not refused.
    #[serde(default, rename = "wipLimit", skip_serializing_if = "Option::is_none")]
    pub wip_limit: Option<u32>,
    /// The `progress` values whose tasks stand in this column unless they were placed elsewhere.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub progress: Vec<String>,
}

impl Column {
    /// A column for some progress values.
    pub fn new(id: &str, name: &str, progress: &[&str]) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            wip_limit: None,
            progress: progress.iter().map(ToString::to_string).collect(),
        }
    }
}

/// A board.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    /// The name shown.
    pub name: String,
    /// Which tasks are on it: those with this label (JSCalendar keyword); none = every task. Tasks that have a place
    /// in `order` are on it too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The columns, left to right.
    pub columns: Vec<Column>,
    /// The cards that were placed: column id to the task `uid`s in it, top to bottom.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub order: BTreeMap<String, Vec<String>>,
}

/// A task as the layout needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardTask {
    /// The task's `uid`.
    pub uid: String,
    /// Its progress (the effective one).
    pub progress: String,
}

/// A column with the cards in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnLayout {
    /// The column.
    pub column: Column,
    /// The `uid`s, top to bottom: the placed ones first, then the others in the order the tasks were given.
    pub cards: Vec<String>,
    /// More cards than the limit says (`false` without a limit). A report, never a refusal.
    pub over_limit: bool,
}

/// The layout of a board.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// The columns with their cards.
    pub columns: Vec<ColumnLayout>,
}

impl Board {
    /// The usual four columns: to do, in progress, done, closed (failed and cancelled).
    pub fn standard(name: &str) -> Self {
        Self {
            name: name.into(),
            label: None,
            columns: vec![
                Column::new("todo", "To do", &["needs-action"]),
                Column::new("doing", "In progress", &["in-process"]),
                Column::new("done", "Done", &["completed"]),
                Column::new("closed", "Closed", &["failed", "cancelled"]),
            ],
            order: BTreeMap::new(),
        }
    }

    /// What is wrong with the board, if anything: no column, a column id twice or empty, a limit of 0, a place in a
    /// column that does not exist, a card in two columns or twice in one.
    pub fn problem(&self) -> Option<String> {
        if self.name.trim().is_empty() {
            return Some("a board needs a name".into());
        }
        if self.columns.is_empty() {
            return Some("a board needs at least one column".into());
        }
        let mut ids = BTreeSet::new();
        for c in &self.columns {
            if c.id.trim().is_empty() || !ids.insert(c.id.as_str()) {
                return Some(format!("column id {:?} is empty or used twice", c.id));
            }
            if c.wip_limit == Some(0) {
                return Some(format!(
                    "column {:?}: a limit of 0 means no card at all; leave it out for no limit",
                    c.id
                ));
            }
        }
        let mut placed = BTreeSet::new();
        for (column, cards) in &self.order {
            if !ids.contains(column.as_str()) {
                return Some(format!(
                    "cards are placed in the column {column:?}, which does not exist"
                ));
            }
            for uid in cards {
                if !placed.insert(uid.as_str()) {
                    return Some(format!("the card {uid:?} is placed more than once"));
                }
            }
        }
        None
    }

    /// The column a task with this progress stands in without a place of its own: the first column that lists it, else
    /// the first column.
    pub fn default_column(&self, progress: &str) -> Option<&Column> {
        self.columns
            .iter()
            .find(|c| c.progress.iter().any(|p| p == progress))
            .or_else(|| self.columns.first())
    }

    /// Place a card in a column at `index` (end if `None` or too large), taking it out of any other place. The progress
    /// of the task is not changed (that is an edit of the task), and a full column does not refuse the card.
    ///
    /// # Errors
    /// The column does not exist.
    pub fn move_card(
        &mut self,
        uid: &str,
        column: &str,
        index: Option<usize>,
    ) -> Result<(), String> {
        if !self.columns.iter().any(|c| c.id == column) {
            return Err(format!("no column {column:?}"));
        }
        for cards in self.order.values_mut() {
            cards.retain(|u| u != uid);
        }
        self.order.retain(|_, cards| !cards.is_empty());
        let cards = self.order.entry(column.to_string()).or_default();
        let at = index.map_or(cards.len(), |i| i.min(cards.len()));
        cards.insert(at, uid.to_string());
        Ok(())
    }

    /// Take a column away; its placed cards stand where their progress says again.
    pub fn remove_column(&mut self, column: &str) {
        self.columns.retain(|c| c.id != column);
        self.order.remove(column);
    }

    /// The layout for these tasks (those that are on the board; the caller picked them by label or place).
    pub fn layout(&self, tasks: &[BoardTask]) -> Layout {
        let known: BTreeMap<&str, &BoardTask> = tasks.iter().map(|t| (t.uid.as_str(), t)).collect();
        let placed: BTreeSet<&str> = self
            .order
            .values()
            .flatten()
            .map(String::as_str)
            .filter(|u| known.contains_key(u))
            .collect();
        let columns = self
            .columns
            .iter()
            .map(|column| {
                let mut cards: Vec<String> = self
                    .order
                    .get(&column.id)
                    .into_iter()
                    .flatten()
                    .filter(|u| known.contains_key(u.as_str()))
                    .cloned()
                    .collect();
                cards.extend(
                    tasks
                        .iter()
                        .filter(|t| !placed.contains(t.uid.as_str()))
                        .filter(|t| {
                            self.default_column(&t.progress)
                                .is_some_and(|c| c.id == column.id)
                        })
                        .map(|t| t.uid.clone()),
                );
                let over_limit = column.wip_limit.is_some_and(|l| cards.len() > l as usize);
                ColumnLayout {
                    column: column.clone(),
                    cards,
                    over_limit,
                }
            })
            .collect();
        Layout { columns }
    }
}
