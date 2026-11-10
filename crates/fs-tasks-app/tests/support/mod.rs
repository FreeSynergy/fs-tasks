//! Test support: the real core over an in-memory store behind `TaskBackend`.
//!
//! Pattern: Facade over the set-up. Nothing here pretends to be the core.
#![allow(dead_code, clippy::pedantic)]

use std::sync::Arc;

use fs_admin_frontend::{Backend, Values};
use fs_objects::{Store, TursoStore};
use fs_tasks_app::TaskBackend;
use fs_tasks_core::TaskRepo;
use tokio::runtime::Runtime;

pub struct World {
    pub runtime: Runtime,
    pub repo: Arc<TaskRepo>,
    pub backend: TaskBackend,
}

pub fn world() -> World {
    let runtime = Runtime::new().unwrap();
    let store: Arc<dyn Store> = Arc::new(runtime.block_on(TursoStore::open_in_memory()).unwrap());
    let repo = Arc::new(TaskRepo::new(store, "https://local.invalid/users/owner"));
    let backend = TaskBackend::new(repo.clone(), runtime.handle().clone());
    World {
        runtime,
        repo,
        backend,
    }
}

pub fn v(pairs: &[(&str, &[&str])]) -> Values {
    pairs
        .iter()
        .map(|(k, x)| ((*k).to_owned(), x.iter().map(|s| (*s).to_owned()).collect()))
        .collect()
}

pub fn create(w: &World, pairs: &[(&str, &[&str])]) -> String {
    w.backend.run("create", &v(pairs)).unwrap().id.unwrap()
}

pub fn first(w: &World, id: &str, field: &str) -> Option<String> {
    let all = w.backend.list(&Values::new()).unwrap();
    all.into_iter()
        .find(|r| r.id == id)
        .and_then(|r| r.values.get(field).and_then(|x| x.first().cloned()))
}
