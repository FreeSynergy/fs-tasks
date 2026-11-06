//! The item methods of the contract over a real socket, against the store-backed provider.

mod common;

use common::{ICS, create, items_of, json, with_server};
use fs_interfaces::rpc::{read_uuid, write_uuid};
use fs_interfaces::tasks_capnp::Relation;

#[tokio::test(flavor = "current_thread")]
async fn create_get_edit_label_relate_assign_delete() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let id = create(&items, "uid-1", "First").await;

        let mut get = items.get_by_uid_request();
        get.get().set_uid("uid-1");
        let got = get.send().promise.await.unwrap();
        assert!(got.get().unwrap().get_found());

        let mut edit = items.edit_request();
        write_uuid(edit.get().init_id(), Some(id));
        edit.get().set_jscalendar(json("uid-1", "Renamed").as_str());
        let edited = edit.send().promise.await.unwrap();
        let task = edited.get().unwrap().get_task().unwrap();
        assert!(
            task.get_jscalendar()
                .unwrap()
                .to_str()
                .unwrap()
                .contains("Renamed")
        );

        let mut label = items.assign_label_request();
        write_uuid(label.get().init_id(), Some(id));
        label.get().set_label("urgent");
        label.send().promise.await.unwrap();

        let mut relate = items.create_relation_request();
        write_uuid(relate.get().init_id(), Some(id));
        relate.get().set_other_uid("uid-2");
        relate.get().set_relation(Relation::DependsOn);
        relate.send().promise.await.unwrap();

        let mut assign = items.assign_request();
        write_uuid(assign.get().init_id(), Some(id));
        assign.get().set_name("Ada");
        assign.get().set_email("ada@example.org");
        let assigned = assign.send().promise.await.unwrap();
        let text = assigned
            .get()
            .unwrap()
            .get_task()
            .unwrap()
            .get_jscalendar()
            .unwrap();
        let text = text.to_str().unwrap();
        assert!(
            text.contains("ada@example.org") && text.contains("urgent") && text.contains("uid-2")
        );

        let mut query = items.query_request();
        query.get().init_filter().set_label("urgent");
        let page = query.send().promise.await.unwrap();
        assert_eq!(
            page.get()
                .unwrap()
                .get_page()
                .unwrap()
                .get_items()
                .unwrap()
                .len(),
            1
        );

        let mut delete = items.delete_request();
        write_uuid(delete.get().init_id(), Some(id));
        delete.send().promise.await.unwrap();

        let mut get = items.get_request();
        write_uuid(get.get().init_id(), Some(id));
        let gone = get.send().promise.await.unwrap();
        assert!(!gone.get().unwrap().get_found());
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn lists_page_through_everything() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        for i in 0..105 {
            create(&items, &format!("uid-{i}"), "T").await;
        }
        let (mut total, mut pages, mut cursor) = (0, 0, Vec::new());
        loop {
            let mut list = items.list_request();
            list.get().init_cursor().set_opaque(&cursor);
            let answer = list.send().promise.await.unwrap();
            let page = answer.get().unwrap().get_page().unwrap();
            total += page.get_items().unwrap().len();
            pages += 1;
            cursor = page.get_next().unwrap().get_opaque().unwrap().to_vec();
            if cursor.is_empty() {
                break;
            }
        }
        assert_eq!((total, pages), (105, 2));
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn ics_goes_in_and_comes_back_out() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let mut import = items.import_ics_request();
        import.get().set_text(ICS);
        let answer = import.send().promise.await.unwrap();
        let report = answer.get().unwrap().get_report().unwrap();
        let created = report.get_created().unwrap();
        assert_eq!(created.len(), 1);
        let id = read_uuid(created.get(0)).unwrap();

        let mut export = items.export_ics_request();
        write_uuid(export.get().init_ids(1).get(0), Some(id));
        let text = export.send().promise.await.unwrap();
        let text = text.get().unwrap().get_text().unwrap().to_string().unwrap();
        assert!(text.contains("BEGIN:VTODO") && text.contains("Minutes"));

        let mut again = items.import_ics_request();
        again.get().set_text(ICS);
        let answer = again.send().promise.await.unwrap();
        assert_eq!(
            answer
                .get()
                .unwrap()
                .get_report()
                .unwrap()
                .get_skipped()
                .unwrap()
                .len(),
            1
        );
    })
    .await;
}
