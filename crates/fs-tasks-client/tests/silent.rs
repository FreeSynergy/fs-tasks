//! A server that says nothing: the wait ends, with an error that names the deadline.
//!
//! Integration tests against the trait; the silent listener and the provider that never
//! answers are the only things here that are not the real server, because the real one answers.

mod common;

use std::os::unix::net::UnixListener;
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::time::Duration;

use async_trait::async_trait;
use common::Served;
use fs_interfaces::Capabilities;
use fs_store::ObjectId;
use fs_tasks_client::{RpcTasks, Timeouts};
use fs_tasks_core::{Error, Result, TaskEntry, TaskItems, TaskQuery};

const SHORT: Duration = Duration::from_millis(300);

/// Reports itself, and never answers a list.
struct Hanging;

#[async_trait]
impl TaskItems for Hanging {
    fn capabilities(&self) -> Capabilities {
        Capabilities::new("hanging")
    }
    async fn get(&self, _: ObjectId) -> Result<Option<TaskEntry>> {
        Ok(None)
    }
    async fn get_by_uid(&self, _: &str) -> Result<Option<TaskEntry>> {
        Ok(None)
    }
    async fn list(&self) -> Result<Vec<TaskEntry>> {
        std::future::pending().await
    }
    async fn query(&self, _: &TaskQuery) -> Result<Vec<TaskEntry>> {
        std::future::pending().await
    }
}

#[test]
fn a_listener_that_never_answers_ends_connect_with_the_deadline() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mute.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let keep = std::thread::spawn(move || listener.accept().map(|(stream, _)| stream));
    let started = std::time::Instant::now();
    let timeouts = Timeouts {
        connect: SHORT,
        ..Timeouts::default()
    };
    // On a thread of its own, so a connect without a deadline fails this test instead of hanging it.
    let (done, answer) = channel();
    std::thread::spawn(move || drop(done.send(RpcTasks::connect_with(&path, timeouts))));
    let got = answer
        .recv_timeout(Duration::from_secs(5))
        .expect("connect keeps its deadline");
    let said = got.err().expect("an error").to_string();
    assert!(said.contains("did not answer within 300ms"), "{said}");
    assert!(started.elapsed() < Duration::from_secs(5));
    drop(keep);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_call_that_is_never_answered_ends_with_the_deadline() {
    let served = Served::start().await;
    let hanging = Served::serving(Arc::new(Hanging), served.repo.clone()).await;
    let timeouts = Timeouts {
        call: SHORT,
        ..Timeouts::default()
    };
    let tasks = RpcTasks::connect_with(&hanging.socket, timeouts).unwrap();
    let outer = Duration::from_secs(10);
    let got = tokio::time::timeout(outer, tasks.list())
        .await
        .expect("the client's own deadline comes first");
    let said = got.unwrap_err();
    assert!(
        matches!(said, Error::Store(fs_store::Error::Backend(_))),
        "{said:?}"
    );
    assert!(
        said.to_string().contains("did not answer within 300ms"),
        "{said}"
    );
    // The connection is still good for what is answered.
    assert!(tasks.get(ObjectId::new()).await.unwrap().is_none());
}
