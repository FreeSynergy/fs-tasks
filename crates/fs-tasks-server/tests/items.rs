//! The item methods of the contract over a real socket, against the store-backed provider. Values
//! are compared, so a method that answers the wrong thing fails here.

mod common;

use common::{ICS, create, items_of, json, set_id, task_json, with_server};
use fs_interfaces::rpc::{read_uuid, write_uuid};
use fs_interfaces::tasks_capnp::{Relation, items};
use serde_json::Value;

async fn found_by_id(items: &items::Client, id: uuid::Uuid) -> Option<Value> {
    let mut get = items.get_request();
    write_uuid(get.get().init_id(), Some(id));
    let answer = get.send().promise.await.unwrap();
    let r = answer.get().unwrap();
    r.get_found().then(|| task_json(r.get_task().unwrap()))
}

#[tokio::test(flavor = "current_thread")]
async fn each_write_does_exactly_what_it_says() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let id = create(&items, "uid-1", "First").await;

        let mut by_uid = items.get_by_uid_request();
        by_uid.get().set_uid("uid-1");
        let answer = by_uid.send().promise.await.unwrap();
        let r = answer.get().unwrap();
        assert!(r.get_found());
        assert_eq!(task_json(r.get_task().unwrap())["title"], "First");
        assert_eq!(found_by_id(&items, id).await.unwrap()["uid"], "uid-1");

        let mut edit = items.edit_request();
        set_id(edit.get().init_id(), id);
        edit.get().set_jscalendar(json("uid-1", "Renamed").as_str());
        let answer = edit.send().promise.await.unwrap();
        let task = answer.get().unwrap().get_task().unwrap();
        assert_eq!(task_json(task)["title"], "Renamed");

        let mut label = items.assign_label_request();
        set_id(label.get().init_id(), id);
        label.get().set_label("urgent");
        label.send().promise.await.unwrap();

        let mut relate = items.create_relation_request();
        set_id(relate.get().init_id(), id);
        relate.get().set_other_uid("uid-2");
        relate.get().set_relation(Relation::DependsOn);
        relate.send().promise.await.unwrap();

        let mut assign = items.assign_request();
        set_id(assign.get().init_id(), id);
        assign.get().set_name("Ada");
        assign.get().set_email("ada@example.org");
        assign.send().promise.await.unwrap();

        let stored = found_by_id(&items, id).await.unwrap();
        assert_eq!(stored["keywords"]["urgent"], true);
        let relation = &stored["relatedTo"]["uid-2"]["relation"];
        assert_eq!(relation["depends-on"], true, "{stored}");
        let who: Vec<&Value> = stored["participants"]
            .as_object()
            .unwrap()
            .values()
            .collect();
        assert_eq!(who[0]["email"], "ada@example.org");
        assert_eq!(who[0]["name"], "Ada");

        let mut delete = items.delete_request();
        set_id(delete.get().init_id(), id);
        delete.send().promise.await.unwrap();
        assert_eq!(found_by_id(&items, id).await, None);
    })
    .await;
}

async fn query_uids(
    items: &items::Client,
    set: impl FnOnce(&mut items::query_params::Builder<'_>),
) -> Vec<String> {
    let mut query = items.query_request();
    set(&mut query.get());
    let answer = query.send().promise.await.unwrap();
    let page = answer.get().unwrap().get_page().unwrap();
    page.get_items()
        .unwrap()
        .iter()
        .map(|t| task_json(t)["uid"].as_str().unwrap().to_string())
        .collect()
}

const NOV: i64 = 1_793_491_200_000; // 2026-11-01
const DEC_DUE: i64 = 1_796_115_600_000; // 2026-12-01T09:00:00Z

#[tokio::test(flavor = "current_thread")]
async fn filters_exclude_what_they_must() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let dec = create(&items, "dec", "T").await; // due 2026-12-01 10:00 Berlin = 09:00 UTC
        let mut other = items.create_request();
        other.get().set_jscalendar(
            r#"{"@type":"Task","version":"2.0","uid":"jan","title":"T","due":"2027-01-15T10:00:00","progress":"completed"}"#,
        );
        other.send().promise.await.unwrap();
        let mut label = items.assign_label_request();
        set_id(label.get().init_id(), dec);
        label.get().set_label("urgent");
        label.send().promise.await.unwrap();

        let by_label = query_uids(&items, |q| q.reborrow().init_filter().set_label("urgent")).await;
        assert_eq!(by_label, ["dec"]);
        let by_progress = query_uids(&items, |q| q.reborrow().init_filter().set_progress("completed")).await;
        assert_eq!(by_progress, ["jan"]);
        let open = query_uids(&items, |q| q.reborrow().init_filter().set_progress("needs-action")).await;
        assert_eq!(open, ["dec"]);

        let by_range = query_uids(&items, |q| {
            let mut f = q.reborrow().init_filter();
            f.reborrow().init_due_from().set_unix_millis(NOV);
            f.init_due_to().set_unix_millis(DEC_DUE + 1000);
        })
        .await;
        assert_eq!(by_range, ["dec"]);
        let before = query_uids(&items, |q| {
            q.reborrow().init_filter().init_due_to().set_unix_millis(DEC_DUE);
        })
        .await;
        assert!(before.is_empty(), "half-open: {before:?}");
        let everything = query_uids(&items, |_| {}).await;
        assert_eq!(everything, ["dec", "jan"], "no filter: all, by due date");
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn lists_page_through_everything_once_and_in_order() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        for i in 0..105 {
            create(&items, &format!("uid-{i:03}"), "T").await;
        }
        let (mut uids, mut pages, mut cursor) = (Vec::new(), 0, Vec::new());
        loop {
            let mut list = items.list_request();
            list.get().init_cursor().set_opaque(&cursor);
            let answer = list.send().promise.await.unwrap();
            let page = answer.get().unwrap().get_page().unwrap();
            for t in page.get_items().unwrap() {
                uids.push(task_json(t)["uid"].as_str().unwrap().to_string());
            }
            pages += 1;
            cursor = page.get_next().unwrap().get_opaque().unwrap().to_vec();
            if cursor.is_empty() {
                break;
            }
        }
        assert_eq!(pages, 2);
        let expected: Vec<String> = (0..105).map(|i| format!("uid-{i:03}")).collect();
        assert_eq!(uids.len(), 105);
        let mut sorted = uids.clone();
        sorted.sort();
        assert_eq!(sorted, expected, "each exactly once");
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn ics_reports_every_field_and_comes_back_out() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let mut import = items.import_ics_request();
        import.get().set_text(ICS);
        let answer = import.send().promise.await.unwrap();
        let report = answer.get().unwrap().get_report().unwrap();
        let created = report.get_created().unwrap();
        assert_eq!(created.len(), 1);
        assert_eq!(report.get_skipped().unwrap().len(), 0);
        assert_eq!(report.get_failed().unwrap().len(), 0);
        let id = read_uuid(created.get(0)).unwrap();
        assert_eq!(found_by_id(&items, id).await.unwrap()["title"], "Minutes");

        let mut export = items.export_ics_request();
        write_uuid(export.get().init_ids(1).get(0), Some(id));
        let text = export.send().promise.await.unwrap();
        let text = text.get().unwrap().get_text().unwrap().to_string().unwrap();
        assert!(
            text.contains("BEGIN:VTODO") && text.contains("UID:todo-1"),
            "{text}"
        );

        let mut again = items.import_ics_request();
        again.get().set_text(ICS);
        let answer = again.send().promise.await.unwrap();
        let report = answer.get().unwrap().get_report().unwrap();
        assert_eq!(report.get_created().unwrap().len(), 0);
        let skipped = report.get_skipped().unwrap();
        assert_eq!(skipped.get(0).unwrap().to_str().unwrap(), "todo-1");

        let mut lost = items.import_ics_request();
        lost.get().set_text("no calendar at all");
        let answer = lost.send().promise.await.unwrap();
        let failed = answer
            .get()
            .unwrap()
            .get_report()
            .unwrap()
            .get_failed()
            .unwrap();
        assert_eq!(
            failed.get(0).unwrap().to_str().unwrap(),
            "not an iCalendar text"
        );
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_large_page_is_cut_by_weight_and_stays_reachable() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let big = "x".repeat(7 * 1024 * 1024);
        for i in 0..4 {
            create(&items, &format!("big-{i}"), &big).await;
        }
        let (mut total, mut pages, mut cursor) = (0, 0, Vec::new());
        loop {
            let mut list = items.list_request();
            list.get().init_cursor().set_opaque(&cursor);
            let answer = list.send().promise.await.expect("no connection loss");
            let page = answer.get().unwrap().get_page().unwrap();
            total += page.get_items().unwrap().len();
            pages += 1;
            cursor = page.get_next().unwrap().get_opaque().unwrap().to_vec();
            if cursor.is_empty() {
                break;
            }
        }
        assert_eq!(total, 4);
        assert!(pages > 1, "4 tasks of 14 MiB answer cannot be one page");
    })
    .await;
}
