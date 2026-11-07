//! What the tasks client takes from the root of the server.
//!
//! Pattern: Adapter — the root `tasks` of `tasks.capnp` to the one capability the calls use
//! (`Items`), in the form the shared `Worker` asks for ([`Remote`]).

use fs_interfaces::Capabilities;
use fs_interfaces::rpc::client::Remote;
use fs_interfaces::tasks_capnp::{items, tasks};

/// The capability of the tasks server a call may use.
#[derive(Clone)]
pub(crate) struct TasksRemote {
    pub(crate) items: items::Client,
}

impl Remote for TasksRemote {
    type Root = tasks::Client;

    async fn open(root: tasks::Client) -> Result<(Self, Capabilities), capnp::Error> {
        let report = root.support_report_request().send().promise.await?;
        let capabilities = Capabilities::read(report.get()?.get_report()?)?;
        let items = root.items_request().send().promise.await?;
        let items = items.get()?.get_items()?;
        Ok((Self { items }, capabilities))
    }
}
