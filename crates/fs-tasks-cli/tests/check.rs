//! Objective tasks and `check`: a check command is executable code, so these are mostly refusals.

mod common;

use common::{cmd, id_of, raw_create, with_cli};
use fs_tasks_cli::{Policy, execute};
use serde_json::{Value, json};

const FOREIGN: &str = "check command not run: foreign origin";

fn argv(text: &str) -> Vec<String> {
    text.split_whitespace().map(String::from).collect()
}

/// A task as `add` would make it (local, by the owner), but with any check command, which `add` would refuse.
fn local_task(check: &str) -> Value {
    json!({
        "@type": "Task", "version": "2.0", "uid": format!("urn:uuid:{}", check.len()), "title": "T",
        "freesynergy.net:origin": "local",
        "freesynergy.net:createdBy": "https://localhost/actors/owner",
        "freesynergy.net:check": check,
    })
}

#[tokio::test(flavor = "current_thread")]
async fn a_local_trusted_task_runs_its_check_and_done_needs_it_to_pass() {
    with_cli(|root| async move {
        let ok = id_of(&cmd(&root, "add Good --objective true --json", &["true"]).await.unwrap());
        let early = cmd(&root, &format!("status {ok} done"), &["true"]).await.unwrap_err();
        assert!(early.text.contains("done needs a passed check"), "{early}");
        assert!(cmd(&root, &format!("done {ok}"), &["true"]).await.is_err(), "`done` is gated too");
        assert_eq!(cmd(&root, &format!("check {ok}"), &["true"]).await.unwrap(), "check passed");
        let done = cmd(&root, &format!("status {ok} done --json"), &["true"]).await.unwrap();
        assert!(done.contains("\"status\":\"done\""), "{done}");

        let bad = id_of(&cmd(&root, "add Bad --objective false --json", &["false"]).await.unwrap());
        let failed = cmd(&root, &format!("check {bad}"), &["false"]).await.unwrap_err();
        assert!(failed.text.starts_with("check failed: exit 1"), "{failed}");
        assert!(cmd(&root, &format!("status {bad} done"), &["false"]).await.is_err());
        // A subjective task is done by its actor at any time.
        let free = id_of(&cmd(&root, "add Free --subjective", &[]).await.unwrap());
        cmd(&root, &format!("status {free} done"), &[]).await.unwrap();
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn the_default_list_lets_cargo_run() {
    with_cli(|root| async move {
        // `add` takes the command as one word, so a command with arguments is built raw here.
        let raw = raw_create(&root, &local_task("cargo --version")).await;
        assert_eq!(cmd(&root, &format!("check {raw}"), &[]).await.unwrap(), "check passed");
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn foreign_origin_never_runs_and_changes_nothing() {
    with_cli(|root| async move {
        // No origin at all (written over the wire by somebody else) and an explicit foreign one.
        let mut none = local_task("true");
        none.as_object_mut().unwrap().remove("freesynergy.net:origin");
        let mut imported = local_task("true");
        imported["uid"] = json!("urn:uuid:imported");
        imported["freesynergy.net:origin"] = json!("imported");
        for task in [none, imported] {
            let id = raw_create(&root, &task).await;
            let before = cmd(&root, &format!("show {id}"), &[]).await.unwrap();
            let e = cmd(&root, &format!("check {id}"), &["true"]).await.unwrap_err();
            assert_eq!((e.text.as_str(), e.code()), (FOREIGN, 1));
            let e = cmd(&root, &format!("status {id} done"), &["true"]).await.unwrap_err();
            assert_eq!(e.text, FOREIGN, "done is not accepted either");
            assert_eq!(cmd(&root, &format!("show {id}"), &[]).await.unwrap(), before);
        }
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn an_imported_task_given_a_check_afterwards_is_still_foreign() {
    with_cli(|root| async move {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("in.ics");
        std::fs::write(
            &file,
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VTODO\r\nUID:t1\r\nDTSTAMP:20261101T000000Z\r\nSUMMARY:In\r\nEND:VTODO\r\nEND:VCALENDAR\r\n",
        )
        .unwrap();
        cmd(&root, &format!("import {}", file.display()), &[]).await.unwrap();
        let listed = cmd(&root, "list --json", &[]).await.unwrap();
        let listed: Value = serde_json::from_str(&listed).unwrap();
        assert_eq!(listed[0]["origin"], "imported");
        // Even with a check command written into it later, it keeps its origin.
        let id = listed[0]["id"].as_str().unwrap().to_string();
        let shown: Value = serde_json::from_str(&cmd(&root, &format!("show {id}"), &[]).await.unwrap()).unwrap();
        let mut edited = shown.clone();
        edited["freesynergy.net:check"] = json!("true");
        edit_raw(&root, &id, &edited).await;
        let e = cmd(&root, &format!("check {id}"), &["true"]).await.unwrap_err();
        assert_eq!(e.text, FOREIGN);
    })
    .await;
}

async fn edit_raw(root: &fs_interfaces::tasks_capnp::tasks::Client, id: &str, task: &Value) {
    let items = root.items_request().send().promise.await.unwrap().get().unwrap().get_items().unwrap();
    let mut req = items.edit_request();
    fs_interfaces::rpc::write_uuid(req.get().init_id(), Some(id.parse::<fs_objects::ObjectId>().unwrap().as_uuid()));
    req.get().set_jscalendar(task.to_string().as_str());
    req.send().promise.await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn a_creator_that_is_not_trusted_does_not_run_a_check_unless_it_is_listed() {
    with_cli(|root| async move {
        let id = id_of(&cmd(&root, "add A --as agent --objective true --json", &["true"]).await.unwrap());
        let e = cmd(&root, &format!("check {id}"), &["true"]).await.unwrap_err();
        assert_eq!(e.text, "check command not run: creator not trusted");
        // The trust list is a setting.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("check-policy.json"), r#"{"trusted_actors":["owner","agent"],"programs":["true"]}"#).unwrap();
        let policy = Policy::load_from(Some(dir.path())).unwrap();
        assert!(policy.trusted.contains(&"https://localhost/actors/agent".to_string()));
        let words = argv(&format!("check {id}"));
        let command = fs_tasks_cli::parse(&words).unwrap().command.unwrap();
        let out = fs_tasks_cli::run_with(&root, command, &Ok(policy)).await.unwrap();
        assert_eq!(out, "check passed");
        // A settings file that cannot be read means that nothing runs.
        std::fs::write(dir.path().join("check-policy.json"), "{").unwrap();
        assert!(Policy::load_from(Some(dir.path())).is_err());
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn programs_off_the_list_and_path_tricks_do_not_run() {
    with_cli(|root| async move {
        for (n, check) in [
            "rm -rf x",
            "bash scripts/../evil.sh",
            "bash ../evil.sh",
            "bash /etc/evil.sh",
            "bash -c id",
            "bash",
            "/usr/bin/cargo build",
            "./cargo build",
            "scripts/run.sh",
            "cargo/../sh x",
        ]
        .into_iter()
        .enumerate()
        {
            let mut t = local_task(check);
            t["uid"] = json!(format!("urn:uuid:n{n}"));
            let id = raw_create(&root, &t).await;
            let e = cmd(&root, &format!("check {id}"), &[]).await.unwrap_err();
            assert!(e.text.starts_with("check command not run: "), "{check}: {e}");
            assert!(e.text != FOREIGN, "{check}: {e}");
            let shown = cmd(&root, &format!("show {id}"), &[]).await.unwrap();
            assert!(!shown.contains("freesynergy.net:checked"), "{check}: nothing recorded");
        }
        // `add` refuses them at once as well.
        assert!(cmd(&root, "add X --objective rm", &[]).await.is_err());
        // A script under scripts/ is fine as far as the policy goes.
        let policy = Policy::default();
        assert!(policy.program_ok(&argv("bash scripts/ok.sh a b")).is_ok());
        assert!(policy.program_ok(&argv("cargo test --workspace")).is_ok());
        assert!(policy.program_ok(&argv("fs-release check")).is_ok());
    })
    .await;
}

#[test]
fn a_check_runs_without_a_shell_with_a_time_limit_and_bounded_output() {
    // Nothing expands: the word reaches the program as it is.
    let out = execute(&argv("echo $HOME;touch_x"), 10).unwrap();
    assert!(out.passed && out.tail.trim() == "$HOME;touch_x", "{out:?}");
    // The output is cut to its end.
    let big = execute(&argv("head -c 300000 /dev/zero"), 10).unwrap();
    assert!(big.passed && big.tail.len() <= 2 * 2048, "{}", big.tail.len());
    // A program that does not end is killed.
    let slow = execute(&argv("sleep 30"), 1).unwrap();
    assert!(slow.timed_out && !slow.passed, "{slow:?}");
    assert!(execute(&argv("no-such-program-here"), 1).is_err());
}
