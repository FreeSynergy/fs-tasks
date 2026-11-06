//! Failures and the provider's report, as a consumer sees them.

mod common;

use common::{create, items_of, json, with_server};
use fs_interfaces::rpc::{Problem, write_uuid};

#[tokio::test(flavor = "current_thread")]
async fn errors_arrive_typed() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        create(&items, "uid-1", "First").await;

        let mut twice = items.create_request();
        twice.get().set_jscalendar(json("uid-1", "Again").as_str());
        let e = twice.send().promise.await.err().unwrap();
        assert_eq!(Problem::from_error(&e).unwrap().status, 409);

        let mut broken = items.create_request();
        broken.get().set_jscalendar("{ not json");
        let e = broken.send().promise.await.err().unwrap();
        assert_eq!(Problem::from_error(&e).unwrap().status, 422);

        let mut unknown = items.delete_request();
        let id = fs_store::Envelope::new("Task", "x").id.as_uuid();
        write_uuid(unknown.get().init_id(), Some(id));
        let e = unknown.send().promise.await.err().unwrap();
        assert_eq!(Problem::from_error(&e).unwrap().status, 404);

        let e = root.boards_request().send().promise.await.err().unwrap();
        assert!(Problem::from_error(&e).unwrap().is_unsupported());
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn capabilities_and_report_match() {
    with_server(|root| async move {
        let caps = root.capabilities_request().send().promise.await.unwrap();
        let list = caps.get().unwrap().get_capabilities().unwrap();
        let ids: Vec<String> = list
            .get_ids()
            .unwrap()
            .iter()
            .map(|s| s.unwrap().to_string().unwrap())
            .collect();
        assert!(
            ids.iter().any(|i| i == "task.subscribe") && ids.iter().any(|i| i == "task.export")
        );
        assert!(!ids.iter().any(|i| i.starts_with("board.")));

        let report = root.support_report_request().send().promise.await.unwrap();
        let report = report.get().unwrap().get_report().unwrap();
        assert_eq!(report.get_provider().unwrap().to_str().unwrap(), "fs-tasks");
        let refused = report
            .get_entries()
            .unwrap()
            .iter()
            .filter(|e| !e.get_supported())
            .count();
        assert_eq!(refused, 5);
    })
    .await;
}
