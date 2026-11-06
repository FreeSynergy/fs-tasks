//! `Items` interface: every method of the contract, each one call of the provider.
//!
//! Pattern: Adapter — wire request in, `TaskItems` call, wire response out. What a provider cannot
//! do arrives as its explicit `Unsupported` and leaves as a typed problem.

use std::rc::Rc;
use std::sync::Arc;

use fs_interfaces::rpc::read_object_id;
use fs_interfaces::tasks_capnp::items;
use fs_tasks_core::contract::{read_filter, relation_from_wire};
use fs_tasks_core::{TaskItems, TaskQuery};

use crate::errors::rpc_error;
use crate::wire::{read_ids, read_task, write_page, write_report, write_task};

pub(crate) struct ItemsImpl {
    pub(crate) items: Arc<dyn TaskItems>,
}

type Done = Result<(), capnp::Error>;

impl items::Server for ItemsImpl {
    async fn create(
        self: Rc<Self>,
        params: items::CreateParams,
        mut results: items::CreateResults,
    ) -> Done {
        let task = read_task(params.get()?.get_jscalendar()?)?;
        let entry = self.items.create(task).await.map_err(rpc_error)?;
        write_task(results.get().init_task(), &entry);
        Ok(())
    }

    async fn edit(
        self: Rc<Self>,
        params: items::EditParams,
        mut results: items::EditResults,
    ) -> Done {
        let p = params.get()?;
        let id = read_object_id(p.get_id()?)?;
        let task = read_task(p.get_jscalendar()?)?;
        let entry = self.items.edit(id, task).await.map_err(rpc_error)?;
        write_task(results.get().init_task(), &entry);
        Ok(())
    }

    async fn assign(
        self: Rc<Self>,
        params: items::AssignParams,
        mut results: items::AssignResults,
    ) -> Done {
        let p = params.get()?;
        let id = read_object_id(p.get_id()?)?;
        let (name, email) = (p.get_name()?.to_str()?, p.get_email()?.to_str()?);
        let entry = self
            .items
            .assign(id, name, email)
            .await
            .map_err(rpc_error)?;
        write_task(results.get().init_task(), &entry);
        Ok(())
    }

    async fn create_relation(
        self: Rc<Self>,
        params: items::CreateRelationParams,
        mut results: items::CreateRelationResults,
    ) -> Done {
        let p = params.get()?;
        let id = read_object_id(p.get_id()?)?;
        let relation = relation_from_wire(p.get_relation()?);
        let other = p.get_other_uid()?.to_str()?;
        let entry = self
            .items
            .relate(id, other, relation)
            .await
            .map_err(rpc_error)?;
        write_task(results.get().init_task(), &entry);
        Ok(())
    }

    async fn assign_label(
        self: Rc<Self>,
        params: items::AssignLabelParams,
        mut results: items::AssignLabelResults,
    ) -> Done {
        let p = params.get()?;
        let id = read_object_id(p.get_id()?)?;
        let label = p.get_label()?.to_str()?;
        let entry = self.items.label(id, label).await.map_err(rpc_error)?;
        write_task(results.get().init_task(), &entry);
        Ok(())
    }

    async fn get(self: Rc<Self>, params: items::GetParams, mut results: items::GetResults) -> Done {
        let id = read_object_id(params.get()?.get_id()?)?;
        let found = self.items.get(id).await.map_err(rpc_error)?;
        let mut r = results.get();
        r.set_found(found.is_some());
        if let Some(entry) = found {
            write_task(r.init_task(), &entry);
        }
        Ok(())
    }

    async fn get_by_uid(
        self: Rc<Self>,
        params: items::GetByUidParams,
        mut results: items::GetByUidResults,
    ) -> Done {
        let p = params.get()?;
        let found = self
            .items
            .get_by_uid(p.get_uid()?.to_str()?)
            .await
            .map_err(rpc_error)?;
        let mut r = results.get();
        r.set_found(found.is_some());
        if let Some(entry) = found {
            write_task(r.init_task(), &entry);
        }
        Ok(())
    }

    async fn delete(self: Rc<Self>, params: items::DeleteParams, _: items::DeleteResults) -> Done {
        let id = read_object_id(params.get()?.get_id()?)?;
        self.items.delete(id).await.map_err(rpc_error)
    }

    async fn list(
        self: Rc<Self>,
        params: items::ListParams,
        mut results: items::ListResults,
    ) -> Done {
        let all = self.items.list().await.map_err(rpc_error)?;
        write_page(results.get().init_page(), all, params.get()?.get_cursor())
    }

    async fn query(
        self: Rc<Self>,
        params: items::QueryParams,
        mut results: items::QueryResults,
    ) -> Done {
        let p = params.get()?;
        let filter = if p.has_filter() {
            read_filter(p.get_filter()?)?
        } else {
            TaskQuery::new()
        };
        let all = self.items.query(&filter).await.map_err(rpc_error)?;
        write_page(results.get().init_page(), all, p.get_cursor())
    }

    async fn import_ics(
        self: Rc<Self>,
        params: items::ImportIcsParams,
        mut results: items::ImportIcsResults,
    ) -> Done {
        let p = params.get()?;
        let report = self
            .items
            .import_ics(p.get_text()?.to_str()?)
            .await
            .map_err(rpc_error)?;
        write_report(results.get().init_report(), &report)
    }

    async fn export_ics(
        self: Rc<Self>,
        params: items::ExportIcsParams,
        mut results: items::ExportIcsResults,
    ) -> Done {
        let ids = read_ids(params.get()?.get_ids()?)?;
        let text = self.items.export_ics(&ids).await.map_err(rpc_error)?;
        results.get().set_text(text.as_str());
        Ok(())
    }
}
