//! `Tasks` root interface: capabilities, items, subscribe, boards, support report.
//!
//! Pattern: Adapter + Factory — the root hands out the `Items` capability. [`TasksServer`] is what a
//! program serves; it depends on the two traits, never on `TaskRepo` (ADR 0043).

use std::rc::Rc;
use std::sync::Arc;

use capnp_rpc::new_client;
use fs_interfaces::Unsupported;
use fs_interfaces::rpc::unsupported;
use fs_interfaces::tasks_capnp::tasks;
use fs_tasks_core::contract::CONTRACT_VERSION;
use fs_tasks_core::{TaskFeed, TaskItems};

use crate::feed::subscribe;
use crate::items_service::ItemsImpl;

/// The root capability of the tasks program.
pub struct TasksServer {
    items: Arc<dyn TaskItems>,
    feed: Arc<dyn TaskFeed>,
}

impl TasksServer {
    /// Serve this provider.
    pub fn new(items: Arc<dyn TaskItems>, feed: Arc<dyn TaskFeed>) -> Self {
        Self { items, feed }
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

    async fn support_report(
        self: Rc<Self>,
        _: tasks::SupportReportParams,
        mut results: tasks::SupportReportResults,
    ) -> Done {
        let report = self.items.capabilities();
        report.write(CONTRACT_VERSION, results.get().init_report());
        Ok(())
    }
}
