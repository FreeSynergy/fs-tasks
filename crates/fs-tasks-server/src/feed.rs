//! Changes of the provider to a subscribed consumer.
//!
//! Pattern: Observer + Adapter — the core calls its listener on the thread that made the change; this
//! one only puts the change into a channel. The shared forwarder adds the envelope and sends it.

use std::sync::Arc;

use fs_interfaces::common_capnp::{change_listener, subscription};
use fs_interfaces::rpc::{Notice, forward};
use fs_tasks_core::{TaskChange, TaskFeed, TaskItems, TaskListener};
use tokio::sync::mpsc;

struct ToChannel(mpsc::UnboundedSender<TaskChange>);

impl TaskListener for ToChannel {
    fn changed(&self, change: &TaskChange) {
        // A closed channel means the subscription is ending; nothing is left to tell.
        let _ = self.0.send(*change);
    }
}

/// Subscribe `listener` until the returned capability is dropped.
pub(crate) fn subscribe(
    feed: &Arc<dyn TaskFeed>,
    items: Arc<dyn TaskItems>,
    listener: change_listener::Client,
) -> subscription::Client {
    let (tx, rx) = mpsc::unbounded_channel();
    let core_subscription = feed.subscribe(Arc::new(ToChannel(tx)));
    forward(rx, core_subscription, listener, move |change| {
        let items = items.clone();
        async move { notice(items.as_ref(), change).await }
    })
}

/// A task that is gone again by now has nothing to report but its own `Deleted`, which follows.
async fn notice(items: &dyn TaskItems, change: TaskChange) -> Option<Notice> {
    match change {
        TaskChange::Created(id) => Some(Notice::Created(items.get(id).await.ok()??.envelope)),
        TaskChange::Updated(id) => Some(Notice::Updated(items.get(id).await.ok()??.envelope)),
        TaskChange::Deleted(id) => Some(Notice::Deleted(id)),
    }
}
