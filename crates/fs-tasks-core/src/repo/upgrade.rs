//! Repair of the index of tasks stored before 0.7.0.
//!
//! Pattern: Template method (fixed flow: read in chunks, find what lacks the keys, rebuild them
//! through the same `build` as every write, write back, report) — a task written by 0.6.0 has no
//! `order_key`, `id_key`, `description` and the dependency fields (`rel_index`, `precedes`, `depends_on`) in its index, so no page can start after it. The flow is
//! idempotent, and one task that cannot be repaired is named in the report and does not stop the rest.

use fs_objects::{Order, Query, StoredObject};

use super::consts::{CONTEXT, DESCRIPTION, ID_KEY, ORDER_KEY, TASK_TYPE};
use super::deps::{DEPENDS_ON, PRECEDES, REL_INDEX};
use super::handle::TaskRepo;
use crate::error::Result;
use crate::model::Task;

/// Tasks read per query.
const CHUNK: usize = 100;

/// What [`TaskRepo::upgrade_index`] did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IndexReport {
    /// Tasks whose index was completed.
    pub rewritten: usize,
    /// Tasks that could not be repaired, each as `<id>: <reason>`. They stay without a place in
    /// the order; a page says so (`Error::Stored`) instead of leaving them out.
    pub failed: Vec<String>,
}

impl TaskRepo {
    /// Complete the index of every task that lacks `order_key`, `id_key` or the dependency fields. Run it when the
    /// repository is opened; it does nothing for tasks that are in order. The content, the envelope
    /// and the original of a task are not touched, only its index fields are added.
    pub async fn upgrade_index(&self) -> Result<IndexReport> {
        let _one_at_a_time = self.writes.lock().await;
        let mut report = IndexReport::default();
        let mut offset = 0;
        loop {
            let q = Query::new()
                .of_type(TASK_TYPE)
                .in_context(CONTEXT)
                .ordered(Order::PublishedAsc)
                .limit(CHUNK)
                .offset(offset);
            let chunk = self.queries.query(&q).await?;
            if chunk.is_empty() {
                return Ok(report);
            }
            offset += chunk.len();
            for object in chunk {
                if object.index.contains_key(ORDER_KEY)
                    && object.index.contains_key(ID_KEY)
                    && object.index.contains_key(REL_INDEX)
                {
                    continue;
                }
                let id = object.envelope.id;
                match self.complete(object).await {
                    Ok(()) => report.rewritten += 1,
                    Err(e) => report.failed.push(format!("{id}: {e}")),
                }
            }
        }
    }

    async fn complete(&self, mut object: StoredObject) -> Result<()> {
        let task = Task::from_value(object.content.clone())?;
        let fresh = self.build(&task, Some(&object.envelope), None)?;
        for key in [
            ORDER_KEY,
            ID_KEY,
            DESCRIPTION,
            REL_INDEX,
            PRECEDES,
            DEPENDS_ON,
        ] {
            if let Some(values) = fresh.index.get(key) {
                object.index.insert(key.into(), values.clone());
            }
        }
        self.objects.put(object, &self.actor).await?;
        Ok(())
    }
}
