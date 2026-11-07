//! A list longer than a page is read to its end.
//!
//! Pattern: Integration tests against the trait; no stand-ins.

mod common;

use common::{Served, due};
use fs_tasks_core::TaskItems;

#[tokio::test(flavor = "multi_thread")]
async fn every_page_is_read_in_the_order_of_the_server() {
    let served = Served::start().await;
    for n in 0..250u32 {
        let day = 1 + n % 28;
        served
            .repo
            .create(due(&format!("uid-{n:03}"), &format!("Task {n:03}"), day))
            .await
            .unwrap();
    }
    let tasks = served.client();
    let over_the_wire: Vec<_> = tasks
        .list()
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.id)
        .collect();
    let at_server: Vec<_> = served
        .repo
        .list()
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.id)
        .collect();
    assert_eq!(over_the_wire.len(), 250, "pages of 100 end at 250");
    assert_eq!(over_the_wire, at_server);
}
