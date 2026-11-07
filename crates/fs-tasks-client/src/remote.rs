//! What the tasks client takes from the root of the server.
//!
//! Pattern: Adapter — the root `tasks` of `tasks.capnp` to the one capability the calls use
//! (`Items`), in the form the shared `Worker` asks for ([`Remote`]). Skeleton: `open` comes next.

use fs_interfaces::Capabilities;
use fs_interfaces::rpc::client::Remote;
use fs_interfaces::tasks_capnp::{items, tasks};

/// The capability of the tasks server a call may use.
#[derive(Clone)]
#[allow(dead_code)] // the worker switches to it in the next step
pub(crate) struct TasksRemote {
    pub(crate) items: items::Client,
}

impl Remote for TasksRemote {
    type Root = tasks::Client;

    #[allow(clippy::unused_async_trait_impl)] // skeleton only; the real body awaits
    async fn open(_root: tasks::Client) -> Result<(Self, Capabilities), capnp::Error> {
        Err(capnp::Error::unimplemented("TasksRemote::open".into()))
    }
}
