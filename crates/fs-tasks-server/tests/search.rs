//! `TaskFilter.term` and `open` over the socket: what they match, and that they combine by AND.

mod common;

use common::{Read, create_text, items_of, json_with, read_pages, with_server};
use fs_interfaces::tasks_capnp::items;

async fn found(
    items: &items::Client,
    set: impl FnOnce(fs_interfaces::tasks_capnp::task_filter::Builder<'_>),
) -> Vec<String> {
    let mut call = items.query_request();
    set(call.get().init_filter());
    let answer = call.send().promise.await.unwrap();
    let page = answer.get().unwrap().get_page().unwrap();
    let mut uids: Vec<String> = page
        .get_items()
        .unwrap()
        .iter()
        .map(|t| common::task_json(t)["uid"].as_str().unwrap().to_string())
        .collect();
    uids.sort();
    uids
}

#[tokio::test(flavor = "current_thread")]
async fn a_term_matches_title_and_description_regardless_of_case() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        for (uid, title, description) in [
            ("a", "Quarterly REPORT", None),
            ("b", "Call", Some("send the Report to Ärger")),
            ("c", "Lunch", Some("with the team")),
            ("d", "100% done_", None),
        ] {
            create_text(&items, &json_with(uid, title, description, None, None)).await;
        }
        assert_eq!(
            found(&items, |mut f| f.set_term("report")).await,
            ["a", "b"]
        );
        assert_eq!(
            found(&items, |mut f| f.set_term("REPORT")).await,
            ["a", "b"]
        );
        assert_eq!(found(&items, |mut f| f.set_term("ärger")).await, ["b"]);
        assert_eq!(found(&items, |mut f| f.set_term("TEAM")).await, ["c"]);
        assert_eq!(
            found(&items, |mut f| f.set_term("nothing like it"))
                .await
                .len(),
            0
        );
        // `%` and `_` are letters of the term, not wildcards.
        assert_eq!(found(&items, |mut f| f.set_term("0% d")).await, ["d"]);
        assert_eq!(found(&items, |mut f| f.set_term("e_")).await, ["d"]);
        assert_eq!(
            found(&items, |mut f| f.set_term("")).await.len(),
            4,
            "empty = no restriction"
        );
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn open_means_needs_action_or_in_process_and_no_progress_counts_as_needs_action() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        for (uid, progress) in [
            ("none", None),
            ("needs", Some("needs-action")),
            ("busy", Some("in-process")),
            ("done", Some("completed")),
            ("failed", Some("failed")),
            ("dropped", Some("cancelled")),
        ] {
            create_text(&items, &json_with(uid, "T", None, None, progress)).await;
        }
        assert_eq!(
            found(&items, |mut f| f.set_open(true)).await,
            ["busy", "needs", "none"]
        );
        assert_eq!(found(&items, |mut f| f.set_open(false)).await.len(), 6);
        // AND with progress: open and finished is nothing; open and in-process is that one.
        let both = |progress: &'static str| {
            move |mut f: fs_interfaces::tasks_capnp::task_filter::Builder<'_>| {
                f.set_open(true);
                f.set_progress(progress);
            }
        };
        assert_eq!(found(&items, both("completed")).await.len(), 0);
        assert_eq!(found(&items, both("in-process")).await, ["busy"]);
    })
    .await;
}

const DEC_1: i64 = 1_796_083_200_000; // 2026-12-01T00:00:00Z

#[tokio::test(flavor = "current_thread")]
async fn term_open_and_the_older_filters_combine_by_and() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let make = |uid, title, due, progress| json_with(uid, title, None, due, progress);
        create_text(
            &items,
            &make("a", "Report one", Some("2026-12-01T10:00:00"), None),
        )
        .await;
        create_text(
            &items,
            &make(
                "b",
                "Report two",
                Some("2026-12-02T10:00:00"),
                Some("completed"),
            ),
        )
        .await;
        create_text(
            &items,
            &make("c", "Other", Some("2026-12-01T11:00:00"), None),
        )
        .await;
        create_text(&items, &make("d", "Report three", None, Some("in-process"))).await;
        assert_eq!(
            found(&items, |mut f| {
                f.set_term("report");
                f.set_open(true);
            })
            .await,
            ["a", "d"]
        );
        assert_eq!(
            found(&items, |mut f| {
                f.set_term("report");
                f.set_open(true);
                f.reborrow().init_due_from().set_unix_millis(DEC_1);
                f.init_due_to().set_unix_millis(DEC_1 + 86_400_000);
            })
            .await,
            ["a"]
        );
        // Paged: a term that matches few tasks still pages through exactly those.
        let paged = read_pages(&items, Read::Query(""), |_| async {}).await;
        assert_eq!(paged.len(), 4);
    })
    .await;
}
