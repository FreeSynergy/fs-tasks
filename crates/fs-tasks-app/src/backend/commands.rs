//! The commands of the support table that the core does, one function each.
//!
//! Pattern: Command (one function per command, selected by name in `mod.rs`), each a thin step over `TaskItems`; the
//! value mapping is in `read.rs` and `write.rs`, the errors in `issues.rs`.

use std::sync::Arc;

use fs_admin_frontend::{BackendError, Outcome, Record, Values};
use fs_objects::ObjectId;
use fs_tasks_core::{Revision, Task, TaskItems, TaskQuery, validate};
use fs_types::TypeProblem;
use tokio::runtime::Handle;

use super::ids::{object_id, record_id};
use super::instants::utc_of;
use super::issues::{backend_error, issue};
use super::read::record;
use super::write::merge;

/// What the commands work on: the provider and the runtime that drives it.
pub struct Ctx<'a> {
    pub items: &'a Arc<dyn TaskItems>,
    pub runtime: &'a Handle,
}

fn first<'v>(input: &'v Values, name: &str) -> &'v str {
    input
        .get(name)
        .and_then(|v| v.iter().find(|t| !t.trim().is_empty()))
        .map_or("", |t| t.trim())
}

fn id_of(input: &Values, name: &str) -> Result<ObjectId, BackendError> {
    object_id(first(input, name)).ok_or(BackendError::NotFound)
}

fn done(id: ObjectId) -> Outcome {
    Outcome {
        id: Some(record_id(id)),
        values: Values::new(),
    }
}

fn answer(name: &str, texts: Vec<String>) -> Outcome {
    Outcome {
        id: None,
        values: Values::from([(name.to_owned(), texts)]),
    }
}

impl Ctx<'_> {
    fn wait<T>(
        &self,
        f: impl Future<Output = fs_tasks_core::Result<T>>,
    ) -> Result<T, BackendError> {
        self.runtime.block_on(f).map_err(|e| backend_error(&e))
    }

    /// The records the filters of `find` select.
    pub fn find(&self, query: &Values) -> Result<Vec<Record>, BackendError> {
        let mut q = TaskQuery::new();
        let bad =
            |field: &str| BackendError::Rejected(vec![issue(field, TypeProblem::BadDateTime)]);
        match (first(query, "dueFrom"), first(query, "dueTo")) {
            ("", "") => {}
            (from, to) => {
                q.due_from = if from.is_empty() {
                    None
                } else {
                    Some(utc_of(from).ok_or_else(|| bad("dueFrom"))?)
                };
                q.due_to = if to.is_empty() {
                    None
                } else {
                    Some(utc_of(to).ok_or_else(|| bad("dueTo"))?)
                };
            }
        }
        let text = |name: &str| Some(first(query, name).to_owned()).filter(|t| !t.is_empty());
        q.term = text("text");
        q.progress = text("progress");
        q.label = text("label");
        q.open = !first(query, "open").is_empty();
        Ok(self
            .wait(self.items.query(&q))?
            .iter()
            .map(record)
            .collect())
    }

    /// `create`.
    pub fn create(&self, input: &Values) -> Result<Outcome, BackendError> {
        let task = new_task(input)?;
        Ok(done(self.wait(self.items.create(task))?.id))
    }

    /// `change`: read, merge what was sent, write if nobody wrote in between.
    pub fn change(&self, input: &Values) -> Result<Outcome, BackendError> {
        let id = id_of(input, "id")?;
        let entry = self
            .wait(self.items.get(id))?
            .ok_or(BackendError::NotFound)?;
        let revision = Revision::of(&entry);
        let mut json = entry.task.to_value();
        merge(&mut json, input).map_err(BackendError::Rejected)?;
        let task = Task::from_value(json).map_err(|e| backend_error(&e.into()))?;
        Ok(done(
            self.wait(self.items.edit_checked(id, revision, task))?.id,
        ))
    }

    /// `delete`.
    pub fn delete(&self, input: &Values) -> Result<Outcome, BackendError> {
        let id = id_of(input, "id")?;
        self.wait(self.items.delete(id))?;
        Ok(Outcome::default())
    }

    /// `assign`.
    pub fn assign(&self, input: &Values) -> Result<Outcome, BackendError> {
        let id = id_of(input, "id")?;
        Ok(done(
            self.wait(
                self.items
                    .assign(id, first(input, "name"), first(input, "email")),
            )?
            .id,
        ))
    }

    /// `label`.
    pub fn label(&self, input: &Values) -> Result<Outcome, BackendError> {
        let id = id_of(input, "id")?;
        Ok(done(
            self.wait(self.items.label(id, first(input, "label")))?.id,
        ))
    }

    /// `relate`: `other` is a record id of ours, or a `uid` as it stands.
    pub fn relate(&self, input: &Values) -> Result<Outcome, BackendError> {
        let id = id_of(input, "id")?;
        let other = first(input, "other");
        let uid = match object_id(other) {
            Some(oid) => self.wait(self.items.get(oid))?.map(|e| e.task.uid),
            None => None,
        }
        .unwrap_or_else(|| other.to_owned());
        let relation =
            fs_tasks_core::Relation::parse(first(input, "relation")).ok_or_else(|| {
                BackendError::Rejected(vec![issue("relation", TypeProblem::NotInList)])
            })?;
        Ok(done(self.wait(self.items.relate(id, &uid, relation))?.id))
    }

    /// `importIcs`.
    pub fn import_ics(&self, input: &Values) -> Result<Outcome, BackendError> {
        let report = self.wait(self.items.import_ics(first(input, "ics")))?;
        let created = report.created.iter().map(|i| record_id(*i)).collect();
        Ok(Outcome {
            id: None,
            values: Values::from([
                ("created".to_owned(), created),
                ("skipped".to_owned(), report.skipped),
                ("failed".to_owned(), report.failed),
                ("notes".to_owned(), report.notes),
            ]),
        })
    }

    /// `exportIcs`.
    pub fn export_ics(&self, input: &Values) -> Result<Outcome, BackendError> {
        let id = id_of(input, "id")?;
        Ok(answer(
            "ics",
            vec![self.wait(self.items.export_ics(&[id]))?],
        ))
    }

    /// `check`: the findings of the core for a task made of these values; nothing is stored.
    pub fn check(&self, input: &Values) -> Result<Outcome, BackendError> {
        let task = new_task(input)?;
        let findings = validate::issues(&task)
            .iter()
            .map(ToString::to_string)
            .collect();
        Ok(answer("issues", findings))
    }

    /// `suggest`: labels of the stored tasks that contain what was typed.
    pub fn suggest(&self, input: &Values) -> Result<Outcome, BackendError> {
        let typed = first(input, "typed").to_lowercase();
        let mut words: Vec<String> = self
            .wait(self.items.list())?
            .iter()
            .flat_map(|e| e.task.keywords.keys().cloned())
            .filter(|k| k.to_lowercase().contains(&typed))
            .collect();
        words.sort();
        words.dedup();
        Ok(answer("keywords", words))
    }
}

/// A new task from the values of a form.
fn new_task(input: &Values) -> Result<Task, BackendError> {
    let mut json = Task::new(first(input, "title")).to_value();
    merge(&mut json, input).map_err(BackendError::Rejected)?;
    Task::from_value(json).map_err(|e| backend_error(&e.into()))
}
