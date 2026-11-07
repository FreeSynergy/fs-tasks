//! A server that is not there, or goes away: an error, not a hang.
//!
//! Integration tests against the trait; nothing stands in for the server or the store.

mod common;

use std::time::Duration;

use common::{Served, task};
use fs_tasks_client::RpcTasks;
use fs_tasks_core::{Error, TaskItems};

#[test]
fn nothing_listening_is_an_error_at_connect() {
    let dir = tempfile::tempdir().unwrap();
    assert!(RpcTasks::connect(&dir.path().join("none.sock")).is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_that_stopped_answers_with_an_error_every_time() {
    let mut served = Served::start().await;
    let tasks = served.client();
    tasks.create(task("a", "A")).await.unwrap();
    served.stop();
    for _ in 0..2 {
        let got = tokio::time::timeout(Duration::from_secs(10), tasks.list())
            .await
            .expect("no hang");
        assert!(
            matches!(got, Err(Error::Store(fs_objects::Error::Backend(_)))),
            "{got:?}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn two_calls_are_under_way_at_the_same_time() {
    let served = Served::start_meeting().await;
    let tasks = std::sync::Arc::new(served.client());
    let (a, b) = (tasks.clone(), tasks.clone());
    let both = async move { tokio::join!(a.list(), b.list()) };
    let (first, second) = tokio::time::timeout(Duration::from_secs(10), both)
        .await
        .expect("the server answers only when both calls are in; one after the other never ends");
    assert!(first.unwrap().is_empty() && second.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn calls_from_several_tasks_all_arrive() {
    let served = Served::start().await;
    let tasks = std::sync::Arc::new(served.client());
    let mut joins = Vec::new();
    for n in 0..8 {
        let tasks = tasks.clone();
        joins.push(tokio::spawn(async move {
            tasks
                .create(task(&format!("u{n}"), &format!("T{n}")))
                .await
                .unwrap();
        }));
    }
    for j in joins {
        j.await.unwrap();
    }
    assert_eq!(tasks.list().await.unwrap().len(), 8);
}
