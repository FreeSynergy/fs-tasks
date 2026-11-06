//! Changes of the provider to a subscribed consumer.
//!
//! Pattern: Observer + Adapter — the core calls its listener on the thread that made the change; this
//! one only puts the change into a channel. The shared forwarder adds the envelope and sends it.

use std::sync::Arc;

use fs_interfaces::common_capnp::{change_listener, subscription};
use fs_interfaces::rpc::{ChangeSender, Notice, change_channel, forward};
use fs_tasks_core::{TaskChange, TaskFeed, TaskItems, TaskListener};

struct ToChannel(ChangeSender<TaskChange>);

impl TaskListener for ToChannel {
    fn changed(&self, change: &TaskChange) {
        self.0.send(*change);
    }
}

/// Subscribe `listener` until the returned capability is dropped.
pub(crate) fn subscribe(
    feed: &Arc<dyn TaskFeed>,
    items: Arc<dyn TaskItems>,
    listener: change_listener::Client,
) -> subscription::Client {
    let (tx, rx) = change_channel();
    let core_subscription = feed.subscribe(Arc::new(ToChannel(tx)));
    forward(rx, core_subscription, listener, move |change| {
        let items = items.clone();
        async move { notice(items.as_ref(), change).await }
    })
}

/// A task that is gone again by now has nothing to report but its own `Deleted`, which follows. A
/// failed read is something else: the consumer would miss a change, so it ends the feed.
async fn notice(items: &dyn TaskItems, change: TaskChange) -> Result<Option<Notice>, String> {
    let envelope = async |id| {
        let entry = items.get(id).await.map_err(|e| e.to_string())?;
        Ok::<_, String>(entry.map(|e| e.envelope))
    };
    Ok(match change {
        TaskChange::Created(id) => envelope(id).await?.map(Notice::Created),
        TaskChange::Updated(id) => envelope(id).await?.map(Notice::Updated),
        TaskChange::Deleted(id) => Some(Notice::Deleted(id)),
    })
}
