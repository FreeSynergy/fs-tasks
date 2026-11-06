//! Task -> stored object: envelope and index fields.
//!
//! Pattern: Adapter — maps the JSCalendar task onto the AS2 `Task` envelope and the index fields
//! `fs-store` can query (ADR 0020, 0032).

use fs_store::{Blob, Envelope, ObjectId, StoredObject};

use super::consts::{CONTENT_TYPE, CONTEXT, TASK_TYPE};
use super::handle::TaskRepo;
use crate::error::Result;
use crate::model::Task;
use crate::time::fmt_utc;
use crate::validate::validate;

impl TaskRepo {
    pub(super) fn build(
        &self,
        task: &Task,
        existing: Option<&Envelope>,
        original: Option<Blob>,
    ) -> Result<StoredObject> {
        validate(task)?;
        let mut env = existing
            .cloned()
            .unwrap_or_else(|| self.fresh_envelope(task));
        env.name = task.display_title(); // ADR 0032: JSCalendar `title`
        let span = self.time_index(task)?;
        env.start_time = span.start;
        env.end_time = span.end;
        let mut o = StoredObject::new(env, CONTENT_TYPE, task.to_value());
        o.original = original;
        let mut put = |k: &str, v: String| o.index.entry(k.into()).or_default().push(v);
        put("uid", task.uid.clone());
        put("type", "task".into());
        if let Some(s) = span.start {
            put("start_utc", fmt_utc(s));
        }
        if let Some(e) = span.end {
            put("end_utc", fmt_utc(e));
        }
        if let Some(d) = span.due {
            put("due_utc", fmt_utc(d));
        }
        put("progress", task.effective_progress().to_string());
        if let Some(s) = task.task_status() {
            put("task_status", s.to_string());
        }
        for label in task.keywords.iter().filter(|(_, on)| **on).map(|(k, _)| k) {
            put("label", label.clone());
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
