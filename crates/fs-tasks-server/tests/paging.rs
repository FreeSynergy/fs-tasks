//! Pages cut at a key: mixed-case titles and ties, many page sizes, data changing between pages.

mod common;

use common::{Read, create_text, items_of, json_with, read_pages, set_id, with_page_size};

/// (uid, title, due as Berlin local time). Same titles in other case, equal dues, undated tasks.
const TASKS: [(&str, &str, Option<&str>); 14] = [
    ("U-01", "alpha", Some("2026-12-01T10:00:00")),
    ("u-02", "Alpha", Some("2026-12-01T10:00:00")),
    ("U-03", "ALPHA", Some("2026-12-01T10:00:00")),
    ("u-04", "Äpfel", Some("2026-12-01T10:00:00")),
    ("U-05", "äpfel", Some("2026-12-02T10:00:00")),
    ("u-06", "Zeta", Some("2026-12-02T10:00:00")),
    ("U-07", "zeta", None),
    ("u-08", "Beta", None),
    ("U-09", "BETA", None),
    ("u-10", "beta", None),
    ("U-11", "Straße", Some("2027-01-01T00:00:00")),
    ("u-12", "STRASSE", Some("2027-01-01T00:00:00")),
    ("U-13", "Éclair", None),
    ("u-14", "éclair", Some("2026-11-30T23:59:59")),
];

const SIZES: [usize; 8] = [1, 2, 3, 5, 7, 13, 14, 100];

async fn fill(items: &fs_interfaces::tasks_capnp::items::Client) {
    for (uid, title, due) in TASKS {
        create_text(items, &json_with(uid, title, None, due, None)).await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn every_page_size_gives_every_task_once_in_the_same_order() {
    let reference = std::rc::Rc::new(std::cell::RefCell::new(None::<Vec<String>>));
    for how in [Read::List, Read::Query("")] {
        for size in SIZES {
            let reference = reference.clone();
            with_page_size(size, |root| async move {
                let items = items_of(&root).await;
                fill(&items).await;
                let uids = read_pages(&items, how, |_| async {}).await;
                let mut sorted = uids.clone();
                sorted.sort();
                sorted.dedup();
                assert_eq!(sorted.len(), TASKS.len(), "size {size}: {uids:?}");
                let mut first = reference.borrow_mut();
                match first.as_ref() {
                    None => *first = Some(uids),
                    Some(first) => assert_eq!(&uids, first, "size {size}"),
                }
            })
            .await;
        }
    }
    // The order is: due date (undated last), then the title in lower case, then the id. The
    // ties of a title in other case and of an equal due date are inside, none is lost.
    let order = reference.borrow().clone().unwrap();
    let key = |uid: &String| {
        let (_, title, due) = TASKS.iter().find(|t| t.0 == uid).unwrap();
        (due.is_none(), due.map(str::to_string), title.to_lowercase())
    };
    let keys: Vec<_> = order.iter().map(key).collect();
    assert!(keys.windows(2).all(|w| w[0] <= w[1]), "{order:?}");
}

#[tokio::test(flavor = "current_thread")]
async fn a_filter_pages_through_exactly_what_it_matches() {
    with_page_size(3, |root| async move {
        let items = items_of(&root).await;
        for (i, (uid, title, due)) in TASKS.iter().enumerate() {
            let text = json_with(uid, title, None, *due, None);
            let id = create_text(&items, &text).await;
            if i % 2 == 0 {
                let mut label = items.assign_label_request();
                set_id(label.get().init_id(), id);
                label.get().set_label("even");
                label.send().promise.await.unwrap();
            }
        }
        let mut uids = read_pages(&items, Read::Query("even"), |_| async {}).await;
        uids.sort();
        let mut expected: Vec<_> = TASKS.iter().step_by(2).map(|t| t.0.to_string()).collect();
        expected.sort();
        assert_eq!(uids, expected);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_task_deleted_or_added_between_two_pages_moves_no_boundary() {
    with_page_size(4, |root| async move {
        let items = items_of(&root).await;
        fill(&items).await;
        let all = read_pages(&items, Read::List, |_| async {}).await;

        let again = std::rc::Rc::new(std::cell::Cell::new(0));
        let seen = read_pages(&items, Read::List, |first_page_ids| {
            let items = items.clone();
            let again = again.clone();
            async move {
                // Two tasks of the page go, and one that sorts before everything arrives.
                for id in &first_page_ids[..2] {
                    let mut delete = items.delete_request();
                    set_id(delete.get().init_id(), *id);
                    delete.send().promise.await.unwrap();
                }
                let early = json_with("early", "aaa", None, Some("2020-01-01T00:00:00"), None);
                create_text(&items, &early).await;
                again.set(again.get() + 1);
            }
        })
        .await;
        assert_eq!(again.get(), 1);
        // What came after the first page is exactly what was there, and nothing twice.
        assert_eq!(seen, all, "the boundary stays at the key, not at a count");
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_cursor_that_is_not_ours_is_an_invalid_request() {
    with_page_size(4, |root| async move {
        let items = items_of(&root).await;
        fill(&items).await;
        let position = [&[1u8][..], &2u64.to_le_bytes()].concat();
        for cursor in [&b"garbage"[..], &[0xff], &[2], &position] {
            let mut call = items.list_request();
            call.get().init_cursor().set_opaque(cursor);
            let error = call.send().promise.await.err().expect("refused");
            let problem = fs_interfaces::rpc::Problem::from_error(&error).unwrap();
            assert_eq!(problem.problem_type, fs_interfaces::rpc::INVALID_REQUEST);
        }
    })
    .await;
}
