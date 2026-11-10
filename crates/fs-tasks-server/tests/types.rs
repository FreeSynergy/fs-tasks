//! `Tasks.types` over a real socket: the description of `Task`, the unsupported commands flagged.
#![allow(clippy::pedantic)]

mod common;

use common::with_server;
use fs_interfaces::types_capnp::{BasicType, CommandKind};

fn text(r: capnp::Result<capnp::text::Reader<'_>>) -> String {
    r.unwrap().to_str().unwrap().to_owned()
}

#[tokio::test(flavor = "current_thread")]
async fn the_server_reports_the_description_of_task() {
    with_server(|root| async move {
        let answer = root.types_request().send().promise.await.unwrap();
        let catalog = answer.get().unwrap().get_catalog().unwrap();
        let answer = catalog.describe_request().send().promise.await.unwrap();
        let types = answer.get().unwrap().get_types().unwrap();
        assert_eq!(types.len(), 1);
        let task = types.get(0);
        assert_eq!(text(task.get_name()), "Task");
        assert_eq!(text(task.get_program()), "fs-tasks");
        assert_eq!(
            text(task.get_standard()),
            "JSCalendar Task (RFC 8984 / 9253)"
        );
        let fields = task.get_fields().unwrap();
        let due = fields.iter().find(|f| text(f.get_name()) == "due").unwrap();
        assert_eq!(due.get_basic().unwrap(), BasicType::DateTime);
        let commands = task.get_commands().unwrap();
        let by_name = |n: &str| commands.iter().find(|c| text(c.get_name()) == n).unwrap();
        // done by the core
        for name in [
            "create",
            "change",
            "find",
            "delete",
            "assign",
            "relate",
            "label",
            "importIcs",
            "exportIcs",
            "check",
            "suggest",
        ] {
            assert!(by_name(name).get_supported(), "{name}");
        }
        assert_eq!(by_name("change").get_kind().unwrap(), CommandKind::Change);
        assert_eq!(by_name("change").get_input().unwrap().len(), 0);
        // listed, flagged, not there yet
        for name in [
            "boardCreate",
            "cardMove",
            "commentAdd",
            "webhookCreate",
            "cardRepeat",
        ] {
            assert!(!by_name(name).get_supported(), "{name}");
        }
        assert!(commands.len() > 40);
    })
    .await;
}
