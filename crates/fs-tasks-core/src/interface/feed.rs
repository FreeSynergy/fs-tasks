//! The [`TaskFeed`] contract: live changes of tasks.
//!
//! Pattern: Observer — a [`TaskListener`] subscribes and is told about every change; dropping the
//! [`Subscription`] ends it (as in `common.capnp`). [`Registry`] is the reusable subject.

use std::sync::{Arc, Mutex, PoisonError};

use fs_objects::ObjectId;

/// What happened to a task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskChange {
    /// A task was created.
    Created(ObjectId),
    /// A task was changed (edit, assignment, relation, label).
    Updated(ObjectId),
    /// A task was deleted.
    Deleted(ObjectId),
}

/// Receives changes. Called on the thread that made the change: keep it short, hand work off.
pub trait TaskListener: Send + Sync {
    /// A task changed.
    fn changed(&self, change: &TaskChange);
}

/// Live updates of tasks.
pub trait TaskFeed: Send + Sync {
    /// Tell `listener` about every change until the returned [`Subscription`] is dropped.
    fn subscribe(&self, listener: Arc<dyn TaskListener>) -> Subscription;
}

type Entries = Vec<(u64, Arc<dyn TaskListener>)>;

/// The subject of the Observer: holds the listeners and notifies them.
#[derive(Default)]
pub struct Registry {
    next: Mutex<u64>,
    listeners: Mutex<Entries>,
}

impl Registry {
    /// Add a listener; the subscription removes it again.
    pub fn add(self: &Arc<Self>, listener: Arc<dyn TaskListener>) -> Subscription {
        let mut next = self.next.lock().unwrap_or_else(PoisonError::into_inner);
        *next += 1;
        let id = *next;
        self.entries().push((id, listener));
        Subscription {
            id,
            registry: Arc::clone(self),
        }
    }

    /// Tell every listener.
    pub fn notify(&self, change: &TaskChange) {
        let listeners: Vec<_> = self.entries().iter().map(|(_, l)| l.clone()).collect();
        for l in listeners {
            l.changed(change);
        }
    }

    fn entries(&self) -> std::sync::MutexGuard<'_, Entries> {
        self.listeners
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// Keeps a subscription alive; dropping it unsubscribes.
#[must_use = "dropping the subscription ends it"]
pub struct Subscription {
    id: u64,
    registry: Arc<Registry>,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        let id = self.id;
        self.registry.entries().retain(|(i, _)| *i != id);
    }
}
