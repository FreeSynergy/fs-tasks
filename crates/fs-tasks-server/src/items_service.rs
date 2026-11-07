//! `Items` interface: every method of the contract, each one call of the provider.
//! Pattern: Adapter — wire request in, `TaskItems` call, wire response out. What a provider cannot
//! do arrives as its explicit `Unsupported` and leaves as a typed problem.

use std::rc::Rc;
use std::sync::Arc;

use fs_interfaces::rpc::{check_answer, check_intake, read_object_id, run_to_end};
use fs_interfaces::tasks_capnp::items;
use fs_tasks_core::contract::relation_from_wire;
use fs_tasks_core::{TaskItems, TaskQuery};

use crate::errors::rpc_error;
use crate::paging::{read_page, write_page};
use crate::wire::{filter_of, read_ids, read_task, required, write_report, write_task};

pub(crate) struct ItemsImpl {
    pub(crate) items: Arc<dyn TaskItems>,
    pub(crate) page_size: usize,
}

type Done = Result<(), capnp::Error>;

impl items::Server for ItemsImpl {
    async fn create(
        self: Rc<Self>,
        params: items::CreateParams,
        mut results: items::CreateResults,
    ) -> Done {
        let task = read_task(params.get()?.get_jscalendar()?)?;
        let items = self.items.clone();
        let entry = run_to_end(async move { items.create(task).await })
            .await?
            .map_err(rpc_error)?;
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
        let items = self.items.clone();
        let entry = run_to_end(async move { items.edit(id, task).await })
            .await?
            .map_err(rpc_error)?;
        write_task(results.get().init_task(), &entry);
        Ok(())
    }

    async fn edit_if(
        self: Rc<Self>,
        params: items::EditIfParams,
        results: items::EditIfResults,
    ) -> Done {
        crate::edit_if::edit_if(&self.items, params, results).await
    }

    async fn assign(
        self: Rc<Self>,
        params: items::AssignParams,
        mut results: items::AssignResults,
    ) -> Done {
        let p = params.get()?;
        let id = read_object_id(p.get_id()?)?;
        let name = p.get_name()?.to_str()?;
        check_intake(name.len())?;
        let email = required(p.get_email()?, "email")?;
        let (items, name, email) = (self.items.clone(), name.to_string(), email.to_string());
        let entry = run_to_end(async move { items.assign(id, &name, &email).await })
            .await?
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
        let other = required(p.get_other_uid()?, "otherUid")?;
        let (items, other) = (self.items.clone(), other.to_string());
        let entry = run_to_end(async move { items.relate(id, &other, relation).await })
            .await?
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
        let label = required(p.get_label()?, "label")?;
        let (items, label) = (self.items.clone(), label.to_string());
        let entry = run_to_end(async move { items.label(id, &label).await })
            .await?
            .map_err(rpc_error)?;
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
        let items = self.items.clone();
        run_to_end(async move { items.delete(id).await })
            .await?
            .map_err(rpc_error)
    }

    async fn list(
        self: Rc<Self>,
        params: items::ListParams,
        mut results: items::ListResults,
    ) -> Done {
        let cursor = params.get()?.get_cursor()?.get_opaque()?;
        let page = read_page(&*self.items, &TaskQuery::new(), cursor, self.page_size).await?;
        write_page(results.get().init_page(), &page)
    }

    async fn query(
        self: Rc<Self>,
        params: items::QueryParams,
        mut results: items::QueryResults,
    ) -> Done {
        let p = params.get()?;
        let filter = filter_of(&p)?;
        let cursor = p.get_cursor()?.get_opaque()?;
        let page = read_page(&*self.items, &filter, cursor, self.page_size).await?;
        write_page(results.get().init_page(), &page)
    }

    async fn import_ics(
        self: Rc<Self>,
        params: items::ImportIcsParams,
        mut results: items::ImportIcsResults,
    ) -> Done {
        let p = params.get()?;
        let text = p.get_text()?.to_str()?;
        check_intake(text.len())?;
        let (items, text) = (self.items.clone(), text.to_string());
        let report = run_to_end(async move { items.import_ics(&text).await })
            .await?
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
        check_answer(text.len())?;
        results.get().set_text(text.as_str());
        Ok(())
    }
}
