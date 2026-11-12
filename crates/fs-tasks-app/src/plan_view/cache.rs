//! The latest picture, kept up to date away from the interface thread.
//!
//! Pattern: Observer + Memento holder — a load runs on the runtime, the finished `Snapshot` replaces the old one
//! under a short lock, and whoever listens (the window) is told. Drawing reads the current picture and never
//! waits for the core. A small store is reloaded at once after an own change (so the card moves at once), a large
//! one in the background (readiness of 5000 tasks takes time), and the old picture is shown until it is ready.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::Duration;

use fs_tasks_core::TaskItems;
use tokio::runtime::Handle;

use super::snapshot::Snapshot;
use crate::day::Day;

/// Up to this many tasks an own change reloads the picture before the list is drawn again.
const RELOAD_AT_ONCE: usize = 600;

type Listener = Arc<dyn Fn() + Send + Sync>;

/// The current picture and the way to renew it.
pub struct Cache {
    items: Arc<dyn TaskItems>,
    runtime: Handle,
    today: Day,
    current: Mutex<Option<Arc<Snapshot>>>,
    problem: Mutex<Option<String>>,
    ready: Condvar,
    listener: Mutex<Option<Listener>>,
    loading: AtomicBool,
    again: AtomicBool,
    loads: AtomicU64,
}

fn held<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Cache {
    /// A cache over `items`; the first load starts at once, in the background.
    pub fn start(items: Arc<dyn TaskItems>, runtime: Handle, today: Day) -> Arc<Self> {
        let cache = Arc::new(Self {
            items,
            runtime,
            today,
            current: Mutex::new(None),
            problem: Mutex::new(None),
            ready: Condvar::new(),
            listener: Mutex::new(None),
            loading: AtomicBool::new(false),
            again: AtomicBool::new(false),
            loads: AtomicU64::new(0),
        });
        cache.reload_in_background();
        cache
    }

    /// Call `f` whenever a newer picture has arrived (from a background thread).
    pub fn on_change(&self, f: impl Fn() + Send + Sync + 'static) {
        *held(&self.listener) = Some(Arc::new(f));
    }

    /// The picture now, if there is one yet.
    pub fn current(&self) -> Option<Arc<Snapshot>> {
        held(&self.current).clone()
    }

    /// Why there is no picture, in words, if the load failed.
    pub fn problem(&self) -> Option<String> {
        held(&self.problem).clone()
    }

    /// How many loads have finished.
    pub fn loads(&self) -> u64 {
        self.loads.load(Ordering::Relaxed)
    }

    /// Wait (at most `limit`) for the first picture; for a program that draws once.
    pub fn wait(&self, limit: Duration) -> Option<Arc<Snapshot>> {
        let guard = held(&self.current);
        let (guard, _) = self
            .ready
            .wait_timeout_while(guard, limit, |c| {
                c.is_none() && held(&self.problem).is_none()
            })
            .unwrap_or_else(PoisonError::into_inner);
        guard.clone()
    }

    /// After an own change: reload now if the store is small, else in the background.
    pub fn changed(self: &Arc<Self>) {
        let small = self
            .current()
            .is_none_or(|s| s.entries.len() <= RELOAD_AT_ONCE);
        if small {
            self.reload_blocking();
        } else {
            self.reload_in_background();
        }
    }

    fn store(&self, result: Result<Snapshot, fs_tasks_core::Error>) {
        match result {
            Ok(snapshot) => {
                *held(&self.problem) = None;
                *held(&self.current) = Some(Arc::new(snapshot));
            }
            Err(e) => *held(&self.problem) = Some(e.to_string()),
        }
        self.loads.fetch_add(1, Ordering::Relaxed);
        self.ready.notify_all();
    }

    /// Only from the interface thread, never from inside the runtime.
    fn reload_blocking(&self) {
        let result = self
            .runtime
            .block_on(Snapshot::load(&self.items, self.today));
        self.store(result);
    }

    fn reload_in_background(self: &Arc<Self>) {
        if self.loading.swap(true, Ordering::AcqRel) {
            self.again.store(true, Ordering::Release); // the running load may have read the store before this change
            return;
        }
        let me = Arc::clone(self);
        self.runtime.spawn(async move {
            loop {
                let result = Snapshot::load(&me.items, me.today).await;
                if me.again.swap(false, Ordering::AcqRel) {
                    continue;
                }
                me.loading.store(false, Ordering::Release);
                me.store(result);
                break;
            }
            let listener = held(&me.listener).clone();
            if let Some(tell) = listener {
                tell();
            }
        });
    }
}
