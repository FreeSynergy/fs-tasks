//! `TaskItems` for [`RemoteItems`]: each method is one call (or one walk over the pages) of `Items`.
//!
//! Pattern: Proxy — what the wire does not carry (`claim`, `assign_actor`, `page`, `readiness_many` in one call)
//! keeps the trait's explicit "not possible" or its default (`readiness_many` asks one by one).

use async_trait::async_trait;
use fs_interfaces::rpc::write_uuid;
use fs_interfaces::tasks_capnp::items;
use fs_interfaces::{Capabilities, Unsupported};
use fs_objects::ObjectId;
use fs_tasks_core::contract::{readiness_from_wire, relation_to_wire};
use fs_tasks_core::plan::Readiness;
use fs_tasks_core::{
    Error, ImportReport, Place, Relation, Result, Revision, Task, TaskEntry, TaskItems, TaskPage,
    TaskQuery,
};

use super::decode::{entries, entry, report, strings};
use super::walk::all;
use super::RemoteItems;

fn wire(e: &capnp::Error) -> Error {
    super::decode::wire(e)
}

/// The task of an answer: `$get` is the reader's `get_task`.
macro_rules! task_of {
    ($req:expr, $get:path) => {{
        let answer = $req.send().promise.await.map_err(|e| wire(&e))?;
        entry(answer.get().and_then($get).map_err(|e| wire(&e))?)
    }};
}

fn related(r: Relation) -> fs_interfaces::tasks_capnp::Relation {
    relation_to_wire(r)
}

#[async_trait]
impl TaskItems for RemoteItems {
    fn capabilities(&self) -> Capabilities {
        let mut caps = Capabilities::new("fs-tasks");
        for id in &self.capability_ids {
            caps.supported(id);
        }
        caps
    }

    async fn get(&self, id: ObjectId) -> Result<Option<TaskEntry>> {
        self.call(move |c| async move {
            let mut req = c.get_request();
            write_uuid(req.get().init_id(), Some(id.as_uuid()));
            let answer = req.send().promise.await.map_err(|e| wire(&e))?;
            let r = answer.get().map_err(|e| wire(&e))?;
            if !r.get_found() {
                return Ok(None);
            }
            entry(r.get_task().map_err(|e| wire(&e))?).map(Some)
        })
        .await
    }

    async fn get_by_uid(&self, uid: &str) -> Result<Option<TaskEntry>> {
        let uid = uid.to_string();
        self.call(move |c| async move {
            let mut req = c.get_by_uid_request();
            req.get().set_uid(uid.as_str());
            let answer = req.send().promise.await.map_err(|e| wire(&e))?;
            let r = answer.get().map_err(|e| wire(&e))?;
            if !r.get_found() {
                return Ok(None);
            }
            entry(r.get_task().map_err(|e| wire(&e))?).map(Some)
        })
        .await
    }

    async fn list(&self) -> Result<Vec<TaskEntry>> {
        self.call(|c| all(c, None)).await
    }

    async fn query(&self, query: &TaskQuery) -> Result<Vec<TaskEntry>> {
        let query = query.clone();
        self.call(move |c| all(c, Some(query))).await
    }

    /// The places of a page are the store's own; they do not travel. Read with `list` or `query`.
    async fn page(&self, _: &TaskQuery, _: Option<&Place>, _: usize) -> Result<TaskPage> {
        Err(Unsupported::new("task.query", "pages by place are not on the wire; use list or query").into())
    }

    async fn create(&self, task: Task) -> Result<TaskEntry> {
        self.call(move |c| async move {
            let mut req = c.create_request();
            req.get().set_jscalendar(task.to_value().to_string().as_str());
            task_of!(req, items::create_results::Reader::get_task)
        })
        .await
    }

    async fn edit(&self, id: ObjectId, task: Task) -> Result<TaskEntry> {
        self.call(move |c| async move {
            let mut req = c.edit_request();
            write_uuid(req.get().init_id(), Some(id.as_uuid()));
            req.get().set_jscalendar(task.to_value().to_string().as_str());
            task_of!(req, items::edit_results::Reader::get_task)
        })
        .await
    }

    async fn edit_checked(&self, id: ObjectId, expected: Revision, task: Task) -> Result<TaskEntry> {
        self.call(move |c| async move {
            let mut req = c.edit_if_request();
            write_uuid(req.get().init_id(), Some(id.as_uuid()));
            req.get().set_revision(expected.get());
            req.get().set_jscalendar(task.to_value().to_string().as_str());
            let answer = req.send().promise.await.map_err(|e| wire(&e))?;
            match answer.get().and_then(items::edit_if_results::Reader::get_task) {
                Ok(t) => entry(t),
                Err(e) => Err(wire(&e)),
            }
        })
        .await
        .map_err(|e| if e.is_conflict() { Error::Changed(id) } else { e })
    }

    async fn assign(&self, id: ObjectId, name: &str, email: &str) -> Result<TaskEntry> {
        let (name, email) = (name.to_string(), email.to_string());
        self.call(move |c| async move {
            let mut req = c.assign_request();
            write_uuid(req.get().init_id(), Some(id.as_uuid()));
            req.get().set_name(name.as_str());
            req.get().set_email(email.as_str());
            task_of!(req, items::assign_results::Reader::get_task)
        })
        .await
    }

    async fn relate(&self, id: ObjectId, other_uid: &str, relation: Relation) -> Result<TaskEntry> {
        let other = other_uid.to_string();
        self.call(move |c| async move {
            let mut req = c.create_relation_request();
            write_uuid(req.get().init_id(), Some(id.as_uuid()));
            req.get().set_other_uid(other.as_str());
            req.get().set_relation(related(relation));
            task_of!(req, items::create_relation_results::Reader::get_task)
        })
        .await
    }

    async fn relate_with_gap(
        &self,
        id: ObjectId,
        other_uid: &str,
        relation: Relation,
        gap: &str,
    ) -> Result<TaskEntry> {
        let (other, gap) = (other_uid.to_string(), gap.to_string());
        self.call(move |c| async move {
            let mut req = c.create_relation_with_gap_request();
            write_uuid(req.get().init_id(), Some(id.as_uuid()));
            req.get().set_other_uid(other.as_str());
            req.get().set_relation(related(relation));
            req.get().set_gap(gap.as_str());
            task_of!(req, items::create_relation_with_gap_results::Reader::get_task)
        })
        .await
    }

    async fn readiness(&self, id: ObjectId) -> Result<Readiness> {
        self.call(move |c| async move {
            let mut req = c.readiness_request();
            write_uuid(req.get().init_id(), Some(id.as_uuid()));
            let answer = req.send().promise.await.map_err(|e| wire(&e))?;
            let r = answer
                .get()
                .and_then(items::readiness_results::Reader::get_report)
                .map_err(|e| wire(&e))?;
            let by = strings(r.get_by().map_err(|e| wire(&e))?)?;
            let state = r
                .get_readiness()
                .map_err(|e| wire(&capnp::Error::from(e)))?;
            Ok(readiness_from_wire(state, by))
        })
        .await
    }

    async fn blocked_by(&self, id: ObjectId) -> Result<Vec<TaskEntry>> {
        self.call(move |c| async move {
            let mut req = c.blocked_by_request();
            write_uuid(req.get().init_id(), Some(id.as_uuid()));
            let answer = req.send().promise.await.map_err(|e| wire(&e))?;
            let list = answer
                .get()
                .and_then(items::blocked_by_results::Reader::get_tasks)
                .map_err(|e| wire(&e))?;
            entries(list)
        })
        .await
    }

    async fn dependents(&self, id: ObjectId) -> Result<Vec<TaskEntry>> {
        self.call(move |c| async move {
            let mut req = c.dependents_request();
            write_uuid(req.get().init_id(), Some(id.as_uuid()));
            let answer = req.send().promise.await.map_err(|e| wire(&e))?;
            let list = answer
                .get()
                .and_then(items::dependents_results::Reader::get_tasks)
                .map_err(|e| wire(&e))?;
            entries(list)
        })
        .await
    }

    async fn label(&self, id: ObjectId, label: &str) -> Result<TaskEntry> {
        let label = label.to_string();
        self.call(move |c| async move {
            let mut req = c.assign_label_request();
            write_uuid(req.get().init_id(), Some(id.as_uuid()));
            req.get().set_label(label.as_str());
            task_of!(req, items::assign_label_results::Reader::get_task)
        })
        .await
    }

    async fn delete(&self, id: ObjectId) -> Result<()> {
        self.call(move |c| async move {
            let mut req = c.delete_request();
            write_uuid(req.get().init_id(), Some(id.as_uuid()));
            req.send().promise.await.map_err(|e| wire(&e))?;
            Ok(())
        })
        .await
    }

    async fn import_ics(&self, text: &str) -> Result<ImportReport> {
        let text = text.to_string();
        self.call(move |c| async move {
            let mut req = c.import_ics_request();
            req.get().set_text(text.as_str());
            let answer = req.send().promise.await.map_err(|e| wire(&e))?;
            report(
                answer
                    .get()
                    .and_then(items::import_ics_results::Reader::get_report)
                    .map_err(|e| wire(&e))?,
            )
        })
        .await
    }

    async fn export_ics(&self, ids: &[ObjectId]) -> Result<String> {
        let ids = ids.to_vec();
        self.call(move |c| async move {
            let mut req = c.export_ics_request();
            let len = u32::try_from(ids.len()).map_err(|_| Error::Rejected("too many tasks".into()))?;
            let mut list = req.get().init_ids(len);
            for (i, id) in (0u32..).zip(&ids) {
                write_uuid(list.reborrow().get(i), Some(id.as_uuid()));
            }
            let answer = req.send().promise.await.map_err(|e| wire(&e))?;
            answer
                .get()
                .and_then(items::export_ics_results::Reader::get_text)
                .map_err(|e| wire(&e))?
                .to_string()
                .map_err(|e| Error::Stored(e.to_string()))
        })
        .await
    }
}
