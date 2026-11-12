//! Task -> stored object: envelope and index fields.
//!
//! Pattern: Adapter — maps the JSCalendar task onto the AS2 `Task` envelope and the index fields
//! `fs-objects` can query (ADR 0020, 0032).

use fs_objects::{Blob, Envelope, ObjectId, StoredObject};

use super::consts::{CONTENT_TYPE, CONTEXT, DESCRIPTION, ID_KEY, NO_DUE, ORDER_KEY, TASK_TYPE};
use super::deps::{DEPENDS_ON, PRECEDES, REL_INDEX};
use super::handle::TaskRepo;
use super::span::index_time;
use crate::error::{Error, MAX_TASK_BYTES, Result};
use crate::model::Task;
use crate::validate::validate;

impl TaskRepo {
    pub(super) fn build(
        &self,
        task: &Task,
        existing: Option<&Envelope>,
        original: Option<Blob>,
    ) -> Result<StoredObject> {
        validate(task)?;
        // Checked as it will be answered: small edits add up, escaping can multiply the text, and the
        // title also travels as the envelope's name.
        let bytes = task.to_value().to_string().len() + task.display_title().len();
        if bytes > MAX_TASK_BYTES {
            return Err(Error::TooLarge(bytes));
        }
        let mut env = existing
            .cloned()
            .unwrap_or_else(|| self.fresh_envelope(task));
        env.name = task.display_title(); // ADR 0032: JSCalendar `title`
        let span = self.time_index(task)?;
        env.start_time = span.start;
        env.end_time = span.end;
        let env_id = env.id;
        let mut o = StoredObject::new(env, CONTENT_TYPE, task.to_value());
        o.original = original;
        // The store refuses empty index values, and an empty one says nothing anyway.
        let mut put = |k: &str, v: String| {
            if !v.is_empty() {
                o.index.entry(k.into()).or_default().push(v);
            }
        };
        put("uid", task.uid.clone());
        put("type", "task".into());
        if let Some(s) = span.start {
            put("start_utc", index_time(s));
        }
        if let Some(e) = span.end {
            put("end_utc", index_time(e));
        }
        put(
            ORDER_KEY,
            span.due.map_or_else(|| NO_DUE.into(), index_time),
        );
        if let Some(d) = span.due {
            put("due_utc", index_time(d));
        }
        put(ID_KEY, env_id.to_string());
        if let Some(text) = &task.description {
            put(DESCRIPTION, text.clone());
        }
        put("progress", task.effective_progress().to_string());
        if let Some(s) = task.task_status() {
            put("task_status", s.to_string());
        }
        for label in task.keywords.iter().filter(|(_, on)| **on).map(|(k, _)| k) {
            put("label", label.clone());
        }
        // The dependency fields: who is a successor of this task, and what it depends on, so a reverse lookup is a query.
        put(REL_INDEX, "1".into());
        for (other, relation) in task.relations() {
            if relation.is_temporal() {
                put(PRECEDES, other);
            } else if relation == crate::model::Relation::DependsOn {
                put(DEPENDS_ON, other);
            }
        }
        for actor in task.actors() {
            put("assignee_actor", actor.to_string());
        }
        for p in task.participants.values() {
            if let Some(m) = p.email.as_deref().or(p.calendar_address.as_deref()) {
                put(
                    "participant_email",
                    m.trim().trim_start_matches("mailto:").to_lowercase(),
                );
            }
        }
        Ok(o)
    }

    fn fresh_envelope(&self, task: &Task) -> Envelope {
        let mut e = Envelope::new(TASK_TYPE, task.display_title());
        if let Ok(id) = task.uid.parse::<ObjectId>() {
            e.id = id;
        }
        e.iri = Some(format!("https://{}/tasks/{}", self.instance, e.id));
        e.context = Some(CONTEXT.into());
        e.attributed_to = Some(self.actor.clone());
        e.to = vec![self.actor.clone()];
        e
    }
}
