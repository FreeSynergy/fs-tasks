//! Who became ready because many tasks were created at once (an import): one judgement for all of them.
//!
//! Pattern: Observer (the announcing side), batched — the graph is read once (one scan), readiness is judged without the
//! created tasks and with them, and the tasks that waited before and are ready now are announced. Asking for each created
//! task that some other one names (`announce_unblocked`) would read the whole graph again each time: quadratic on a chain.

use std::collections::BTreeSet;

use super::handle::TaskRepo;
use super::unblocked::freed;
use crate::model::{Relation, Task};
use crate::plan::{Readiness, readiness_of};

impl TaskRepo {
    /// Announce who is ready now and waited when none of the `created` tasks (by `uid`) was there. A failed scan tells
    /// nothing: the write is done, and the readiness of each task can still be asked.
    pub(super) async fn announce_created_together(&self, created: &[String]) {
        let Ok(graph) = self.scan_graph().await else {
            return;
        };
        let new: BTreeSet<&str> = created.iter().map(String::as_str).collect();
        let before_tasks: Vec<Task> = graph
            .iter()
            .filter(|(uid, _)| !new.contains(uid.as_str()))
            .map(|(_, t)| t.clone())
            .collect();
        let after_tasks: Vec<Task> = graph.into_values().collect();
        let (before, after) = (readiness_of(&before_tasks), readiness_of(&after_tasks));
        let mut freed = freed(&before, &after);
        // A created task that waited for another created one is announced too (as when each is asked for on its own).
        freed.extend(after_tasks.iter().filter_map(|t| {
            let waited = t
                .relations()
                .iter()
                .any(|(other, r)| *r == Relation::DependsOn && new.contains(other.as_str()));
            (waited && new.contains(t.uid.as_str()) && after.get(&t.uid) == Some(&Readiness::Ready))
                .then(|| t.uid.clone())
        }));
        let names = self.not_announced(freed, &after, "", false);
        self.notify_unblocked(&names).await;
    }
}
