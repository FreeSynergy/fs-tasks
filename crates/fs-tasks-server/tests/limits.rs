//! What does not fit one message, as a consumer sees it: refused or cut, and the connection stays.

mod common;

use common::{create, items_of, json, set_id, task_json, with_server};
use fs_interfaces::rpc::{Problem, TOO_LARGE, read_envelope};
use fs_interfaces::tasks_capnp::items;
use fs_tasks_core::MAX_TASK_BYTES;

const INVALID: &str = "https://freesynergy.net/problems/tasks/invalid";
const MIB: usize = 1024 * 1024;

fn kind(error: &capnp::Error) -> (String, u16) {
    let p = Problem::from_error(error).expect("a typed problem");
    (p.problem_type, p.status)
}

fn is(error: &capnp::Error, problem_type: &str, status: u16) {
    assert_eq!(kind(error), (problem_type.to_string(), status));
}

fn unknown_id() -> uuid::Uuid {
    fs_objects::Envelope::new("Task", "x").id.as_uuid()
}

async fn count(items: &items::Client) -> u32 {
    let list = items
        .list_request()
        .send()
        .promise
        .await
        .expect("still connected");
    list.get()
        .unwrap()
        .get_page()
        .unwrap()
        .get_items()
        .unwrap()
        .len()
}

/// The ids exist nowhere: without the check at the door the core would answer "not found".
#[tokio::test(flavor = "current_thread")]
async fn every_text_is_refused_before_it_reaches_the_core() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let huge = "x".repeat(20 * MIB);

        let mut label = items.assign_label_request();
        set_id(label.get().init_id(), unknown_id());
        label.get().set_label(huge.as_str());
        is(&label.send().promise.await.err().unwrap(), TOO_LARGE, 413);

        let mut name = items.assign_request();
        set_id(name.get().init_id(), unknown_id());
        name.get().set_name(huge.as_str());
        name.get().set_email("a@example.org");
        is(&name.send().promise.await.err().unwrap(), TOO_LARGE, 413);

        let mut email = items.assign_request();
        set_id(email.get().init_id(), unknown_id());
        email.get().set_name("a");
        email.get().set_email(huge.as_str());
        is(&email.send().promise.await.err().unwrap(), TOO_LARGE, 413);

        let mut relation = items.create_relation_request();
        set_id(relation.get().init_id(), unknown_id());
        relation.get().set_other_uid(huge.as_str());
        is(
            &relation.send().promise.await.err().unwrap(),
            TOO_LARGE,
            413,
        );

        let mut edit = items.edit_request();
        set_id(edit.get().init_id(), unknown_id());
        edit.get().set_jscalendar(json("u", &huge).as_str());
        is(&edit.send().promise.await.err().unwrap(), TOO_LARGE, 413);

        let mut broken = items.create_request();
        broken
            .get()
            .set_jscalendar(format!("{{ not json {huge}").as_str());
        is(&broken.send().promise.await.err().unwrap(), TOO_LARGE, 413);

        let mut import = items.import_ics_request();
        import.get().set_text(huge.as_str());
        is(&import.send().promise.await.err().unwrap(), TOO_LARGE, 413);

        assert_eq!(count(&items).await, 0);
    })
    .await;
}

fn task_json_text(uid: &str, title_bytes: usize) -> String {
    json(uid, &"x".repeat(title_bytes))
}

async fn create_sized(
    items: &items::Client,
    uid: &str,
    title_bytes: usize,
) -> capnp::Result<String> {
    let mut call = items.create_request();
    call.get()
        .set_jscalendar(task_json_text(uid, title_bytes).as_str());
    let answer = call.send().promise.await?;
    Ok(answer.get()?.get_task()?.get_jscalendar()?.to_string()?)
}

/// The largest title that is accepted, found from what a small task weighs when stored.
async fn largest_title(items: &items::Client) -> usize {
    let stored = create_sized(items, "uid-0", 1).await.unwrap();
    (MAX_TASK_BYTES - (stored.len() - 1)) / 2
}

#[tokio::test(flavor = "current_thread")]
async fn tasks_at_the_limit_come_back_through_list_get_and_query() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let largest = largest_title(&items).await;
        let over = create_sized(&items, "uid-9", largest + 1)
            .await
            .err()
            .unwrap();
        is(&over, TOO_LARGE, 413);
        create_sized(&items, "uid-1", largest).await.unwrap();
        create_sized(&items, "uid-2", largest).await.unwrap();

        let read = |page: fs_interfaces::tasks_capnp::task_page::Reader<'_>| {
            let mut titles = vec![];
            for task in page.get_items().unwrap() {
                let content = task_json(task);
                let name = read_envelope(task.get_envelope().unwrap()).unwrap().name;
                assert_eq!(content["title"].as_str().unwrap(), name);
                titles.push(name.len());
            }
            (
                titles,
                page.get_next().unwrap().get_opaque().unwrap().to_vec(),
            )
        };

        let mut through_list = vec![];
        let mut cursor = vec![];
        loop {
            let mut call = items.list_request();
            call.get().init_cursor().set_opaque(&cursor);
            let answer = call.send().promise.await.expect("the list answers");
            let (titles, next) = read(answer.get().unwrap().get_page().unwrap());
            through_list.extend(titles);
            if next.is_empty() {
                break;
            }
            cursor = next;
        }
        through_list.sort_unstable();
        assert_eq!(through_list, [1, largest, largest]);

        let mut through_query = vec![];
        let mut cursor = vec![];
        loop {
            let mut call = items.query_request();
            call.get().init_filter().init_due_from().set_unix_millis(0);
            call.get().init_cursor().set_opaque(&cursor);
            let answer = call.send().promise.await.expect("the query answers");
            let (titles, next) = read(answer.get().unwrap().get_page().unwrap());
            through_query.extend(titles);
            if next.is_empty() {
                break;
            }
            cursor = next;
        }
        through_query.sort_unstable();
        assert_eq!(through_query, [1, largest, largest]);

        let mut by_uid = items.get_by_uid_request();
        by_uid.get().set_uid("uid-1");
        let found = by_uid.send().promise.await.unwrap();
        let task = found.get().unwrap().get_task().unwrap();
        let id = read_envelope(task.get_envelope().unwrap())
            .unwrap()
            .id
            .as_uuid();
        let mut get = items.get_request();
        set_id(get.get().init_id(), id);
        let answer = get.send().promise.await.expect("get answers");
        let content = task_json(answer.get().unwrap().get_task().unwrap());
        assert_eq!(content["title"].as_str().unwrap().len(), largest);
    })
    .await;
}

/// Control characters escape to six bytes each: 90 MiB in one error, without the short quote and the cap.
#[tokio::test(flavor = "current_thread")]
async fn a_value_that_escapes_to_more_than_a_message_is_answered() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let id = create(&items, "uid-1", "First").await;
        let bad = "\u{7f}".repeat(15 * MIB);
        for member in ["due", "start", "estimatedDuration", "updated", "progress"] {
            let task = serde_json::json!({
                "@type": "Task", "version": "2.0", "uid": "u", "title": "t", member: bad,
            });
            let mut call = items.create_request();
            call.get().set_jscalendar(task.to_string().as_str());
            let error = call.send().promise.await.err().unwrap();
            is(&error, INVALID, 422);
            let detail = Problem::from_error(&error).unwrap().detail;
            assert!(detail.len() < 4096, "{member}: {} bytes", detail.len());
            assert!(detail.contains(member), "{detail}");
        }
        assert_eq!(count(&items).await, 1, "still connected");
        let mut get = items.get_request();
        set_id(get.get().init_id(), id);
        get.send().promise.await.expect("still connected");
    })
    .await;
}
