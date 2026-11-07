//! [`TaskRepo`]: the store-backed implementation of the task interface.
//!
//! Pattern: Repository + Facade — one small type hides the object store, the envelope and index
//! rules and the iCalendar format behind `TaskItems` / `TaskFeed`. Its collaborators are
//! injected as the two narrowest store traits (`ObjectRepository`, `ObjectQuery`).

use std::sync::Arc;

use fs_store::traits::{ObjectQuery, ObjectRepository};
use fs_store::{Store, StoreInfo};

use super::backend::StoreAdapter;
use crate::interface::Registry;
use crate::time::Zone;

/// Tasks over an object store. Cheap to clone; clones share their subscribers.
#[derive(Clone)]
pub struct TaskRepo {
    pub(crate) objects: Arc<dyn ObjectRepository>,
    pub(crate) queries: Arc<dyn ObjectQuery>,
    pub(crate) info: Option<Arc<dyn StoreInfo>>,
    pub(crate) feed: Arc<Registry>,
    pub(crate) actor: String,
    pub(crate) instance: String,
    pub(crate) floating: Zone,
    /// One write flow at a time: each reads, decides and writes, and a backend may yield in between.
    pub(crate) writes: Arc<tokio::sync::Mutex<()>>,
}

impl TaskRepo {
    /// Tasks over a whole [`Store`]. `actor` is the IRI of the acting person / program
    /// (written to the log).
    pub fn new(store: Arc<dyn Store>, actor: impl Into<String>) -> Self {
        let adapter = Arc::new(StoreAdapter(store));
        Self::with_backend(adapter.clone(), adapter.clone(), actor).with_info(adapter)
    }

    /// Tasks over the narrow store traits only (a remote or fake backend can supply them).
    pub fn with_backend(
        objects: Arc<dyn ObjectRepository>,
        queries: Arc<dyn ObjectQuery>,
        actor: impl Into<String>,
    ) -> Self {
        Self {
            objects,
            queries,
            info: None,
            feed: Arc::default(),
            actor: actor.into(),
            instance: "local.invalid".into(),
            floating: Zone::Utc,
            writes: Arc::default(),
        }
    }

    /// Say whose store this is: [`TaskItems::identity`](crate::TaskItems::identity) answers from it.
    #[must_use]
    pub fn with_info(mut self, info: Arc<dyn StoreInfo>) -> Self {
        self.info = Some(info);
        self
    }

    /// Instance domain for IRIs `https://<instance>/tasks/<id>` (ADR 0027).
    #[must_use]
    pub fn with_instance(mut self, instance: impl Into<String>) -> Self {
        self.instance = instance.into();
        self
    }

    /// Zone in which floating tasks (no `timeZone`) are interpreted for indexing and queries.
    #[must_use]
    pub fn with_floating_zone(mut self, zone: Zone) -> Self {
        self.floating = zone;
        self
    }
}
