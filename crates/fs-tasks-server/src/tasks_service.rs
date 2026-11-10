//! `Tasks` root interface: capabilities, items, subscribe, boards, support report.
//!
//! Pattern: Adapter + Factory — the root hands out the `Items` capability. [`TasksServer`] is what a
//! program serves; it depends on the two traits, never on `TaskRepo` (ADR 0043).

use std::rc::Rc;
use std::sync::Arc;

use capnp_rpc::new_client;
use fs_interfaces::Unsupported;
use fs_interfaces::rpc::{DEFAULT_PAGE_SIZE, unsupported, write_uuid};
use fs_interfaces::tasks_capnp::tasks;
use fs_tasks_core::contract::CONTRACT_VERSION;
use fs_tasks_core::{DescribesType, Task, TaskFeed, TaskItems};
use fs_types::{TypeRegistry, describe::CatalogServer};

use crate::feed::subscribe;
use crate::items_service::ItemsImpl;

/// The root capability of the tasks program.
pub struct TasksServer {
    items: Arc<dyn TaskItems>,
    feed: Arc<dyn TaskFeed>,
    page_size: usize,
}

impl TasksServer {
    /// Serve this provider.
    pub fn new(items: Arc<dyn TaskItems>, feed: Arc<dyn TaskFeed>) -> Self {
        Self {
            items,
            feed,
            page_size: DEFAULT_PAGE_SIZE,
        }
    }

    /// Cut pages at `size` items instead of the default (a test, or a small device).
    #[must_use]
    pub fn with_page_size(mut self, size: usize) -> Self {
        self.page_size = size.max(1);
        self
    }

    /// The capability to hand to `fs_interfaces::rpc::serve`.
    #[must_use]
    pub fn into_client(self) -> tasks::Client {
        new_client(self)
    }
}

type Done = Result<(), capnp::Error>;

impl tasks::Server for TasksServer {
    async fn capabilities(
        self: Rc<Self>,
        _: tasks::CapabilitiesParams,
        mut results: tasks::CapabilitiesResults,
    ) -> Done {
        let report = self.items.capabilities();
        report.write_list(CONTRACT_VERSION, results.get().init_capabilities());
        Ok(())
    }

    async fn items(
        self: Rc<Self>,
        _: tasks::ItemsParams,
        mut results: tasks::ItemsResults,
    ) -> Done {
        let items = ItemsImpl {
            items: self.items.clone(),
            page_size: self.page_size,
        };
        results.get().set_items(new_client(items));
        Ok(())
    }

    async fn subscribe(
        self: Rc<Self>,
        params: tasks::SubscribeParams,
        mut results: tasks::SubscribeResults,
    ) -> Done {
        let listener = params.get()?.get_listener()?;
        let subscription = subscribe(&self.feed, self.items.clone(), listener);
        results.get().set_subscription(subscription);
        Ok(())
    }

    /// `Boards` has no methods yet in the contract: say so instead of handing out an empty capability.
    async fn boards(self: Rc<Self>, _: tasks::BoardsParams, _: tasks::BoardsResults) -> Done {
        Err(unsupported(&Unsupported::new(
            "board.open",
            "Boards is a placeholder in the contract; not built yet, it is possible",
        )))
    }

    /// The data types this program owns (`Task`) with every command, the unsupported ones flagged (ADR 0066).
    async fn types(
        self: Rc<Self>,
        _: tasks::TypesParams,
        mut results: tasks::TypesResults,
    ) -> Done {
        let mut registry = TypeRegistry::new();
        registry
            .register::<Task>()
            .map_err(|e| capnp::Error::failed(format!("types.unsound: {e:?}")))?;
        results
            .get()
            .set_catalog(new_client(CatalogServer::new(registry)));
        Ok(())
    }

    async fn support_report(
        self: Rc<Self>,
        _: tasks::SupportReportParams,
        mut results: tasks::SupportReportResults,
    ) -> Done {
        let report = self.items.capabilities();
        let mut out = results.get().init_report();
        report.write(CONTRACT_VERSION, out.reborrow());
        // The store's id, as the store says it: empty where the provider has no store.
        write_uuid(
            out.init_instance(),
            self.items.identity().map(|id| id.as_uuid()),
        );
        Ok(())
    }
}
