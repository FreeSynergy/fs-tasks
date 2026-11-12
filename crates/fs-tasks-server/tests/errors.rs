//! Failures and the provider's report, as a consumer sees them: the exact problem type, not a status.

mod common;

use common::{create, items_of, json, set_id, with_server};
use fs_interfaces::UNSUPPORTED_PROBLEM_TYPE;
use fs_interfaces::rpc::{INVALID_REQUEST, Problem, write_uuid};
use fs_objects::Envelope;

const INVALID: &str = "https://freesynergy.net/problems/tasks/invalid";
const CONFLICT: &str = "https://freesynergy.net/problems/tasks/conflict";
const NOT_FOUND: &str = "https://freesynergy.net/problems/tasks/not-found";

fn of(error: &capnp::Error) -> (String, u16) {
    let p = Problem::from_error(error).expect("a typed problem");
    (p.problem_type, p.status)
}

fn is(error: &capnp::Error, problem_type: &str, status: u16) {
    assert_eq!(of(error), (problem_type.to_string(), status));
}

#[tokio::test(flavor = "current_thread")]
async fn errors_arrive_typed() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let id = create(&items, "uid-1", "First").await;

        let mut twice = items.create_request();
        twice.get().set_jscalendar(json("uid-1", "Again").as_str());
        is(&twice.send().promise.await.err().unwrap(), CONFLICT, 409);

        let mut broken = items.create_request();
        broken.get().set_jscalendar("{ not json");
        is(&broken.send().promise.await.err().unwrap(), INVALID, 422);

        let mut unknown = items.delete_request();
        write_uuid(
            unknown.get().init_id(),
            Some(Envelope::new("Task", "x").id.as_uuid()),
        );
        is(&unknown.send().promise.await.err().unwrap(), NOT_FOUND, 404);

        let mut no_id = items.get_request();
        no_id.get().init_id();
        is(
            &no_id.send().promise.await.err().unwrap(),
            INVALID_REQUEST,
            422,
        );

        let mut cursor = items.list_request();
        cursor.get().init_cursor().set_opaque(b"bad");
        is(
            &cursor.send().promise.await.err().unwrap(),
            INVALID_REQUEST,
            422,
        );

        let mut label = items.assign_label_request();
        set_id(label.get().init_id(), id);
        label.get().set_label("");
        is(&label.send().promise.await.err().unwrap(), INVALID, 422);

        let mut relation = items.create_relation_request();
        set_id(relation.get().init_id(), id);
        relation.get().set_other_uid("");
        is(&relation.send().promise.await.err().unwrap(), INVALID, 422);

        let mut assign = items.assign_request();
        set_id(assign.get().init_id(), id);
        assign.get().set_name("n");
        assign.get().set_email("");
        is(&assign.send().promise.await.err().unwrap(), INVALID, 422);

        let mut query = items.query_request();
        query
            .get()
            .init_filter()
            .init_due_to()
            .set_unix_millis(i64::MAX);
        is(
            &query.send().promise.await.err().unwrap(),
            INVALID_REQUEST,
            422,
        );

        let e = root.boards_request().send().promise.await.err().unwrap();
        is(&e, UNSUPPORTED_PROBLEM_TYPE, 501);
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

        let report = root.support_report_request().send().promise.await.unwrap();
        let report = report.get().unwrap().get_report().unwrap();
        assert_eq!(report.get_provider().unwrap().to_str().unwrap(), "fs-tasks");
        let entries = report.get_entries().unwrap();
        let supported: Vec<String> = entries
            .iter()
            .filter(|e| e.get_supported())
            .map(|e| e.get_capability().unwrap().to_string().unwrap())
            .collect();
        assert_eq!(ids, supported, "the list and the report say the same");
        assert!(ids.iter().any(|i| i == "task.subscribe"));
        assert!(ids.iter().any(|i| i == "task.export"));
        let refused: Vec<String> = entries
            .iter()
            .filter(|e| !e.get_supported())
            .map(|e| e.get_capability().unwrap().to_string().unwrap())
            .collect();
        // what the core offers and the contract has no method for yet: boards, claims
        assert_eq!(refused.len(), 6);
        assert!(refused.iter().all(|i| i.starts_with("board.")
            || i.starts_with("column.")
            || i.starts_with("card.")
            || i == "task.claim"));
    })
    .await;
}
