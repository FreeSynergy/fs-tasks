//! The commands against a real server on a real socket (in this process).

mod common;

use common::with_cli;
use fs_tasks_cli::{Command, NewTask, Query, run};

fn add(title: &str) -> Command {
    Command::Add(NewTask {
        title: title.into(),
        due: Some("2026-12-01T10:00:00".into()),
        zone: Some("Europe/Berlin".into()),
        label: Some("work".into()),
        ..NewTask::default()
    })
}

#[tokio::test(flavor = "current_thread")]
async fn add_list_show_done_delete() {
    with_cli(|root| async move {
        let added = run(&root, add("Plan")).await.unwrap();
        let id = added.split("  ").next().unwrap().to_string();
        assert!(added.ends_with("  Plan"), "{added}");

        let listed = run(&root, Command::List(Query::default())).await.unwrap();
        assert_eq!(listed, added, "one task, the same line");

        let shown = run(&root, Command::Show(id.clone())).await.unwrap();
        assert!(shown.contains("\"title\": \"Plan\""), "{shown}");
        assert!(shown.contains("\"work\""), "{shown}");

        let finished = run(&root, Command::Done(id.clone())).await.unwrap();
        assert!(finished.contains("  completed  "), "{finished}");

        let gone = run(&root, Command::Delete(id.clone())).await.unwrap();
        assert_eq!(gone, format!("deleted {id}"));
        assert_eq!(
            run(&root, Command::List(Query::default())).await.unwrap(),
            ""
        );
        let missing = run(&root, Command::Show(id)).await;
        assert!(missing.is_err());
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn export_import_and_capabilities() {
    with_cli(|root| async move {
        run(&root, add("One")).await.unwrap();
        let ics = run(&root, Command::Export).await.unwrap();
        assert!(ics.contains("BEGIN:VTODO") && ics.contains("SUMMARY:One"), "{ics}");

        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("in.ics");
        std::fs::write(
            &file,
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VTODO\r\nUID:todo-1\r\nDTSTAMP:20261101T000000Z\r\nSUMMARY:Minutes\r\nEND:VTODO\r\nEND:VCALENDAR\r\n",
        )
        .unwrap();
        let report = run(&root, Command::Import(file.clone())).await.unwrap();
        assert_eq!(report, "created 1, skipped 0, failed 0");
        let again = run(&root, Command::Import(file)).await.unwrap();
        assert_eq!(again, "created 0, skipped 1, failed 0");

        let caps = run(&root, Command::Capabilities).await.unwrap();
        assert!(caps.lines().any(|l| l == "task.create"), "{caps}");
        assert!(run(&root, Command::Show("not-an-id".into())).await.is_err());
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn an_import_file_that_cannot_be_read_is_an_error_naming_it() {
    with_cli(|root| async move {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope.txt");
        let got = run(&root, Command::Import(missing.clone())).await;
        assert!(got.unwrap_err().text.contains("nope.txt"));
        let binary = dir.path().join("bin.dat");
        std::fs::write(&binary, [0xff, 0xfe, 0x00]).unwrap();
        let got = run(&root, Command::Import(binary)).await;
        assert!(got.unwrap_err().text.contains("not UTF-8"));
    })
    .await;
}
