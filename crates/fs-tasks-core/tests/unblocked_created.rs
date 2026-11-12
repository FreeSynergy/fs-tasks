//! Creating or importing the task that a relation already named (z depends on g, g was missing, g is created completed):
//! z is announced as ready, once.

mod common;

use std::sync::{Arc, Mutex};

use common::repo;
use fs_tasks_core::plan::Readiness;
use fs_tasks_core::{Relation, Task, TaskChange, TaskFeed, TaskItems, TaskListener};

struct Log(Mutex<Vec<TaskChange>>);
impl TaskListener for Log {
    fn changed(&self, c: &TaskChange) {
        self.0.lock().unwrap().push(*c);
    }
}

fn unblocked(log: &Log) -> Vec<fs_objects::ObjectId> {
    log.0
        .lock()
        .unwrap()
        .iter()
        .filter_map(|c| match c {
            TaskChange::Unblocked(id) => Some(*id),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn creating_the_task_a_relation_named_frees_the_one_that_waited() {
    let r = repo().await;
    let mut z = Task::new("z");
    z.uid = "z".into();
    z.relate("g", Relation::DependsOn);
    let z = r.create(z).await.unwrap();
    assert!(matches!(
        r.readiness(z.id).await.unwrap(),
        Readiness::Unknown(_)
    ));
    let log = Arc::new(Log(Mutex::default()));
    let _sub = r.subscribe(log.clone());
    let mut g = Task::new("g");
    g.uid = "g".into();
    g.progress = Some("completed".into());
    r.create(g).await.unwrap();
    assert_eq!(r.readiness(z.id).await.unwrap(), Readiness::Ready);
    assert_eq!(unblocked(&log), [z.id]);
    // a task nobody named frees nobody and says nothing
    let mut other = Task::new("other");
    other.uid = "other".into();
    r.create(other).await.unwrap();
    assert_eq!(unblocked(&log), [z.id]);
}

#[tokio::test]
async fn importing_the_task_a_relation_named_frees_the_one_that_waited_few_and_many() {
    for extra in [0, 30] {
        let r = repo().await;
        let mut z = Task::new("z");
        z.uid = "z".into();
        z.relate("g", Relation::DependsOn);
        let z = r.create(z).await.unwrap();
        let log = Arc::new(Log(Mutex::default()));
        let _sub = r.subscribe(log.clone());
        let mut ics = String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\n");
        for uid in std::iter::once("g".to_string()).chain((0..extra).map(|i| format!("e{i}"))) {
            let status = if uid == "g" {
                "COMPLETED"
            } else {
                "NEEDS-ACTION"
            };
            ics.push_str(&format!(
                "BEGIN:VTODO\r\nUID:{uid}\r\nDTSTAMP:20261101T000000Z\r\nSUMMARY:{uid}\r\nSTATUS:{status}\r\nEND:VTODO\r\n"
            ));
        }
        ics.push_str("END:VCALENDAR\r\n");
        let report = r.import_ics(&ics).await.unwrap();
        assert_eq!(report.created.len(), 1 + extra, "{report:?}");
        assert_eq!(
            unblocked(&log),
            [z.id],
            "with {extra} more tasks in the import"
        );
    }
}
