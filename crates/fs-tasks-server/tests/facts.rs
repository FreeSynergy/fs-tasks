//! The facts of a problem arrive as typed extensions, so a client reads them instead of a sentence.
//! The detail text stays as it was.

mod common;

use common::{create, edit_if, items_of, json, json_with, read_now, set_id, with_server};
use fs_interfaces::rpc::{Problem, TOO_LARGE, write_uuid};
use fs_objects::Envelope;
use fs_tasks_core::MAX_TASK_BYTES;

const INVALID: &str = "https://freesynergy.net/problems/tasks/invalid";
const CONFLICT: &str = "https://freesynergy.net/problems/tasks/conflict";
const NOT_FOUND: &str = "https://freesynergy.net/problems/tasks/not-found";

fn problem_of<T>(answer: Result<T, capnp::Error>) -> Problem {
    Problem::from_error(&answer.err().expect("refused")).expect("a typed problem")
}

#[tokio::test(flavor = "current_thread")]
async fn not_found_names_the_task_and_its_kind() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let gone = Envelope::new("Task", "x").id.as_uuid();
        let mut delete = items.delete_request();
        write_uuid(delete.get().init_id(), Some(gone));
        let p = problem_of(delete.send().promise.await);
        assert_eq!(p.problem_type, NOT_FOUND);
        assert_eq!(p.ext("id"), Some(gone.to_string().as_str()));
        assert_eq!(p.ext("kind"), Some("task"));
        assert_eq!(p.detail, format!("task {gone} not found"));
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_changed_task_is_a_conflict_about_that_task_and_a_taken_uid_has_no_object_yet() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let id = create(&items, "uid-1", "First").await;
        let (old, _) = read_now(&items, id).await;
        let mut edit = items.edit_request();
        set_id(edit.get().init_id(), id);
        edit.get().set_jscalendar(json("uid-1", "Second").as_str());
        edit.send().promise.await.unwrap();
        let late = edit_if(&items, id, old, &json("uid-1", "Late")).await;
        let p = problem_of(late);
        assert_eq!(p.problem_type, CONFLICT);
        assert_eq!(p.ext("id"), Some(id.to_string().as_str()));
        assert_eq!(p.ext("kind"), Some("task"));
        assert!(p.detail.starts_with("conflict:"), "{}", p.detail);

        // Open point (DESIGN.md): a taken uid is a conflict whose text names the uid, no object.
        let mut again = items.create_request();
        again.get().set_jscalendar(json("uid-1", "Again").as_str());
        let p = problem_of(again.send().promise.await);
        assert_eq!(p.problem_type, CONFLICT);
        assert_eq!(p.ext("id"), None);
        assert_eq!(p.detail, "a task with uid uid-1 exists");
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn invalid_lists_every_issue_once_and_the_detail_is_unchanged() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let text = r#"{"@type":"Task","version":"1.0","uid":"u","title":"T","estimatedDuration":"soon","progress":"nearly"}"#;
        let mut create = items.create_request();
        create.get().set_jscalendar(text);
        let p = problem_of(create.send().promise.await);
        assert_eq!(p.problem_type, INVALID);
        let issues: Vec<&str> = p.ext_all("issue").collect();
        assert!(issues.len() >= 3, "{issues:?}");
        let joined = format!("JSCalendar 2.0 violation: {}", issues.join("; "));
        assert_eq!(p.detail, joined, "the detail is the same issues, joined");
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_text_that_is_no_json_and_an_empty_required_text_give_an_issue() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let mut broken = items.create_request();
        broken.get().set_jscalendar("{ not json");
        let p = problem_of(broken.send().promise.await);
        assert_eq!(p.problem_type, INVALID);
        assert_eq!(p.ext_all("issue").collect::<Vec<_>>(), [p.detail.as_str()]);

        let id = create(&items, "uid-1", "First").await;
        let mut label = items.assign_label_request();
        set_id(label.get().init_id(), id);
        let p = problem_of(label.send().promise.await);
        assert_eq!(p.problem_type, INVALID);
        assert_eq!(p.ext("issue"), Some("label"));
        assert_eq!(p.detail, "label must not be empty");
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn too_large_carries_size_and_limit_for_the_intake_and_for_the_task() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let huge = "x".repeat(fs_interfaces::rpc::MAX_INTAKE_BYTES + 1);
        let mut call = items.create_request();
        call.get().set_jscalendar(huge.as_str());
        let p = problem_of(call.send().promise.await);
        assert_eq!(p.problem_type, TOO_LARGE);
        assert_eq!(p.ext_u64("size"), Some(huge.len() as u64));
        assert_eq!(
            p.ext_u64("limit"),
            Some(fs_interfaces::rpc::MAX_INTAKE_BYTES as u64)
        );

        // Under the intake, over what a task may weigh with its name: the core refuses it.
        let title = "y".repeat(MAX_TASK_BYTES / 2 + 1);
        let mut call = items.create_request();
        call.get()
            .set_jscalendar(json_with("big", &title, None, None, None).as_str());
        let p = problem_of(call.send().promise.await);
        assert_eq!(p.problem_type, TOO_LARGE);
        assert_eq!(p.ext_u64("limit"), Some(MAX_TASK_BYTES as u64));
        let size = p.ext_u64("size").expect("the size");
        assert!(size > MAX_TASK_BYTES as u64, "{size}");
        assert!(p.detail.contains(&size.to_string()), "{}", p.detail);
    })
    .await;
}
