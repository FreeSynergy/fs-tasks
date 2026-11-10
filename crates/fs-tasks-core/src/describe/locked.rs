//! The commands of `requirements.md` the core does not do yet, described so a front end can show them locked.
//!
//! Pattern: Null Object (as `TaskItems`) — they are in the description with a name, a kind and the inputs the
//! requirement implies, and `support::is_supported` says no. The inputs are a first guess (named `Unsupported` in
//! `DESIGN.md`); they may change when the core builds the command, before a front end offers a working control.

use fs_types::{BasicType, CommandDescription, CommandKind, FieldDescription};

use super::fields::{id, local};

/// Commands that act on one task: the front end fills `id` from the selection.
fn on_task(name: &str) -> bool {
    ["card", "comment", "attachment", "subtask", "epic", "task"]
        .iter()
        .any(|p| name.starts_with(p))
}

fn kind_of(name: &str) -> CommandKind {
    let creates = ["Create", "Add", "Define", "Submit"];
    if creates.iter().any(|s| name.ends_with(s)) {
        CommandKind::Create
    } else if on_task(name) {
        CommandKind::Change
    } else {
        CommandKind::Other
    }
}

/// The extra inputs a requirement implies, by command.
fn extra(name: &str) -> Vec<FieldDescription> {
    let t = |f: &str| local(name, f, BasicType::Text);
    match name {
        "boardCreate" | "boardEdit" | "columnCreate" | "columnRename" | "swimlaneCreate" => {
            vec![t("name").required()]
        }
        "columnMove" | "cardMove" => vec![t("column").required(), t("position")],
        "columnLimit" => vec![t("column").required(), t("limit").required()],
        "cardSetStatus" => vec![t("status").required()],
        "attachmentAdd" | "cardLinkCommit" => {
            vec![local(name, "link", BasicType::Link).required()]
        }
        "commentAdd" | "commentMention" => {
            vec![local(name, "text", BasicType::MultilineText).required()]
        }
        "memberAdd" | "memberRole" => vec![
            local(name, "person", BasicType::Reference)
                .target("Person")
                .required(),
            t("role"),
        ],
        "webhookCreate" => vec![local(name, "target", BasicType::Link).required()],
        "cardSetField" | "fieldDefine" => vec![t("field").required(), t("value")],
        "cardRemind" => vec![t("before").required()],
        "cardRepeat" => vec![t("rule").required()],
        "querySave" | "taskFilter" => vec![t("query").required()],
        "boardImport" => vec![local(name, "data", BasicType::MultilineText).required()],
        _ => Vec::new(),
    }
}

/// The description of a command that is in the table and not supported.
pub fn locked(name: &str) -> CommandDescription {
    let key = super::fields::kebab(name);
    let mut c = CommandDescription::new(name, kind_of(name), &format!("command.{key}"));
    if on_task(name) {
        c = c.input(id());
    }
    for f in extra(name) {
        c = c.input(f);
    }
    c
}
