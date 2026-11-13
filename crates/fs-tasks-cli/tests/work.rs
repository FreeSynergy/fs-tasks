//! The worker commands: claim, release, next, status, note, list; conflicts, secrets, the board's words.

mod common;

use common::{cmd, id_of, with_cli};
use serde_json::Value;

fn json(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("{e}: {text}"))
}

#[tokio::test(flavor = "current_thread")]
async fn a_claim_held_by_somebody_else_is_a_conflict_that_names_the_holder() {
    with_cli(|root| async move {
        let id = id_of(
            &cmd(&root, "add Work --project p1 --json", &[])
                .await
                .unwrap(),
        );
        cmd(&root, &format!("claim {id} --as alpha --until 90m"), &[])
            .await
            .unwrap();

        let lost = cmd(&root, &format!("claim {id} --as beta"), &[])
            .await
            .unwrap_err();
        assert!(lost.conflict && lost.code() == 3, "{lost:?}");
        assert!(
            lost.text.contains("actors/alpha"),
            "the holder is named: {lost}"
        );
        let release = cmd(&root, &format!("release {id} --as beta"), &[])
            .await
            .unwrap_err();
        assert!(release.conflict, "{release:?}");

        // The holder renews; after its release another one gets it.
        cmd(&root, &format!("claim {id} --as alpha"), &[])
            .await
            .unwrap();
        cmd(&root, &format!("release {id} --as alpha"), &[])
            .await
            .unwrap();
        let won = json(
            &cmd(&root, &format!("claim {id} --as beta --json"), &[])
                .await
                .unwrap(),
        );
        assert_eq!(won["claimed_by"], "https://localhost/actors/beta");
        // The board shows who holds it: the worker is a participant of type Application.
        let shown = cmd(&root, &format!("show {id}"), &[]).await.unwrap();
        assert!(
            shown.contains("\"Application\"") && shown.contains("beta"),
            "{shown}"
        );
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn of_many_claims_at_once_exactly_one_wins() {
    with_cli(|root| async move {
        let id = id_of(&cmd(&root, "add Contested", &[]).await.unwrap());
        let id = &id;
        let (a, b, c) = tokio::join!(
            Box::pin(cmd(&root, &format!("claim {id} --as a"), &[])),
            Box::pin(cmd(&root, &format!("claim {id} --as b"), &[])),
            Box::pin(cmd(&root, &format!("claim {id} --as c"), &[])),
        );
        let results = [a, b, c];
        let won = results.iter().filter(|r| r.is_ok()).count();
        let conflicts = results
            .iter()
            .filter(|r| r.as_ref().is_err_and(|f| f.conflict))
            .count();
        assert_eq!((won, conflicts), (1, 2), "{results:?}");
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn status_note_report_cost_and_failed_are_on_the_task() {
    with_cli(|root| async move {
        let a = id_of(&cmd(&root, "add Ok --project p1", &[]).await.unwrap());
        let b = id_of(&cmd(&root, "add Bad --project p2", &[]).await.unwrap());
        cmd(&root, &format!("status {a} in-progress"), &[])
            .await
            .unwrap();
        let open = json(&cmd(&root, "list --project p1 --json", &[]).await.unwrap());
        assert_eq!(open[0]["status"], "in-progress");

        let done = cmd(
            &root,
            &format!("status {a} done --note built --report /tmp/r.md --cost 0.42 --json"),
            &[],
        )
        .await
        .unwrap();
        let done = json(&done);
        assert_eq!(done["status"], "done");
        assert_eq!(done["note"], "built");
        assert_eq!(done["result"]["report"], "/tmp/r.md");
        assert!((done["result"]["cost_usd"].as_f64().unwrap() - 0.42).abs() < 1e-9);

        cmd(&root, &format!("status {b} failed --note nope"), &[])
            .await
            .unwrap();
        let failed = json(
            &cmd(&root, "list --status failed --json", &[])
                .await
                .unwrap(),
        );
        assert_eq!(failed.as_array().unwrap().len(), 1);
        assert_eq!(failed[0]["project"], "p2");
        let only_p1 = json(&cmd(&root, "list --project p1 --json", &[]).await.unwrap());
        assert_eq!(only_p1.as_array().unwrap().len(), 1);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_note_is_short_and_secrets_are_refused_everywhere() {
    with_cli(|root| async move {
        let id = id_of(&cmd(&root, "add Plain", &[]).await.unwrap());
        for words in [
            "add key=sk-ant-x".to_string(),
            "add T --objective ghp_x".to_string(),
            format!("note {id} ghp_y"),
            format!("status {id} failed --note -----BEGIN"),
            format!("status {id} done --report ghp_z"),
        ] {
            let e = cmd(&root, &words, &[]).await.unwrap_err();
            assert!(e.text.contains("secret") && !e.conflict, "{words}: {e}");
        }
        let long = "x".repeat(501);
        assert!(cmd(&root, &format!("note {id} {long}"), &[]).await.is_err());
        let ok = "y".repeat(500);
        cmd(&root, &format!("note {id} {ok}"), &[]).await.unwrap();
        let shown = cmd(&root, &format!("show {id}"), &[]).await.unwrap();
        assert!(!shown.contains("ghp_") && shown.contains(&ok));
        assert_eq!(
            cmd(&root, "list --json", &[])
                .await
                .map(|t| json(&t).as_array().unwrap().len()),
            Ok(1),
            "only the plain task exists"
        );
    })
    .await;
}

const TWO: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\n\
BEGIN:VTODO\r\nUID:a\r\nDTSTAMP:20261101T000000Z\r\nSUMMARY:First\r\nCATEGORIES:project:p1\r\nEND:VTODO\r\n\
BEGIN:VTODO\r\nUID:b\r\nDTSTAMP:20261101T000000Z\r\nSUMMARY:Second\r\nCATEGORIES:project:p1\r\n\
RELATED-TO;RELTYPE=DEPENDS-ON:a\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";

#[tokio::test(flavor = "current_thread")]
async fn next_takes_the_first_ready_task_and_never_one_that_waits() {
    with_cli(|root| async move {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("two.ics");
        std::fs::write(&file, TWO).unwrap();
        cmd(&root, &format!("import {}", file.display()), &[])
            .await
            .unwrap();

        let first = json(
            &cmd(&root, "next --as w1 --project p1 --json", &[])
                .await
                .unwrap(),
        );
        assert_eq!(first["title"], "First");
        // `Second` waits for `First`; `First` is claimed: nothing is ready for the next worker.
        assert_eq!(
            cmd(&root, "next --as w2 --json", &[]).await.unwrap(),
            "null"
        );
        assert_eq!(cmd(&root, "next --as w2", &[]).await.unwrap(), "");
        // The same worker asking again gets its own task back.
        assert_eq!(
            json(&cmd(&root, "next --as w1 --json", &[]).await.unwrap())["title"],
            "First"
        );
        let id = first["id"].as_str().unwrap();
        cmd(&root, &format!("status {id} done"), &[]).await.unwrap();
        let second = json(&cmd(&root, "next --as w2 --json", &[]).await.unwrap());
        assert_eq!(second["title"], "Second");
        assert_eq!(
            cmd(&root, "next --as w3 --project nope --json", &[])
                .await
                .unwrap(),
            "null"
        );
    })
    .await;
}
