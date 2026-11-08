//! `Items.editIf` over a real socket: compare-and-set on the revision every answer carries.

mod common;

use common::{create, edit_if, items_of, json, read_now, revision_of, set_id, with_server};
use fs_interfaces::rpc::Problem;

const CONFLICT: &str = "https://freesynergy.net/problems/tasks/conflict";
const NOT_FOUND: &str = "https://freesynergy.net/problems/tasks/not-found";

#[track_caller]
fn problem<T>(answer: Result<T, capnp::Error>, kind: &str, status: u16) -> String {
    let Err(error) = answer else {
        panic!("the call succeeded");
    };
    let p = Problem::from_error(&error).expect("a typed problem");
    assert_eq!(
        (p.problem_type.as_str(), p.status),
        (kind, status),
        "{}",
        p.detail
    );
    p.detail
}

#[tokio::test(flavor = "current_thread")]
async fn every_answer_carries_a_revision_and_each_write_changes_it() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let mut make = items.create_request();
        make.get().set_jscalendar(json("u1", "Plan").as_str());
        let created = make.send().promise.await.unwrap();
        let first = revision_of(created.get().unwrap().get_task().unwrap());
        let id = {
            let env = created
                .get()
                .unwrap()
                .get_task()
                .unwrap()
                .get_envelope()
                .unwrap();
            fs_interfaces::rpc::read_envelope(env).unwrap().id.as_uuid()
        };
        assert_eq!(read_now(&items, id).await.0, first);
        let page = items.list_request().send().promise.await.unwrap();
        let listed = page.get().unwrap().get_page().unwrap().get_items().unwrap();
        assert_eq!(revision_of(listed.get(0)), first);

        let mut edit = items.edit_request();
        set_id(edit.get().init_id(), id);
        edit.get().set_jscalendar(json("u1", "Plan B").as_str());
        let done = edit.send().promise.await.unwrap();
        let second = revision_of(done.get().unwrap().get_task().unwrap());
        assert_ne!(second, first);
        assert_eq!(read_now(&items, id).await.0, second);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn edit_if_writes_on_the_revision_read_and_refuses_a_stale_one() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let id = create(&items, "u1", "Plan").await;
        let (read, _) = read_now(&items, id).await;

        let done = edit_if(&items, id, read, &json("u1", "Plan B"))
            .await
            .unwrap();
        let now = revision_of(done.get().unwrap().get_task().unwrap());
        assert_ne!(now, read);

        let detail = problem(
            edit_if(&items, id, read, &json("u1", "Plan C")).await,
            CONFLICT,
            409,
        );
        assert!(detail.starts_with("conflict:"), "{detail}");
        assert!(!detail.contains(&format!("{now:016x}")), "{detail}");
        assert_eq!(
            read_now(&items, id).await,
            (now, "Plan B".into()),
            "nothing was written"
        );

        let ghost = fs_objects::Envelope::new("Task", "x").id.as_uuid();
        problem(
            edit_if(&items, ghost, 1, &json("u9", "X")).await,
            NOT_FOUND,
            404,
        );
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn of_two_writers_on_one_revision_exactly_one_wins() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let id = create(&items, "u1", "Plan").await;
        for round in 0..10 {
            let (read, _) = read_now(&items, id).await;
            let (task_a, task_b) = (
                json("u1", &format!("A{round}")),
                json("u1", &format!("B{round}")),
            );
            let (a, b) = tokio::join!(
                edit_if(&items, id, read, &task_a),
                edit_if(&items, id, read, &task_b)
            );
            let (won, lost) = match (a.is_ok(), b.is_ok()) {
                (true, false) => (format!("A{round}"), b),
                (false, true) => (format!("B{round}"), a),
                other => panic!("round {round}: {other:?}"),
            };
            problem(lost, CONFLICT, 409);
            assert_eq!(read_now(&items, id).await.1, won);
        }
    })
    .await;
}
