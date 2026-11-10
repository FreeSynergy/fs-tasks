//! The commands the core does: one per method of `TaskItems` (plus `change`, `find`, `check`, `suggest`).
//!
//! Pattern: Builder, as `fields.rs`; the name of a command is the key of the support table (`support.rs`).
//! `change` is an `Edit` (ADR 0066): a field that is not in it stays, an empty one clears it, values replace. Commands that
//! take a record (`assign`, `relate`, `label`) are kind `other` with a reference to the type: kind `change` has no input.

use fs_types::{BasicType, CommandDescription, CommandKind, FieldDescription};

use super::fields::{RELATIONS, editable, id, local};
use crate::validate::PROGRESS;

fn command(name: &str, kind: CommandKind) -> CommandDescription {
    CommandDescription::new(
        name,
        kind,
        &format!("command.{}", super::fields::kebab(name)),
    )
}

fn with_inputs(mut c: CommandDescription, fields: Vec<FieldDescription>) -> CommandDescription {
    for f in fields {
        c = c.input(f);
    }
    c
}

/// The description of a command the core does; `None` for a name that is not (or not yet) one of them.
pub fn supported(name: &str) -> Option<CommandDescription> {
    Some(match name {
        "create" => with_inputs(command(name, CommandKind::Create), editable()),
        // The input of `change` is an `Edit` over the fields of the type (rule of `types.capnp`): no input list.
        "change" => command(name, CommandKind::Change),
        "delete" => command(name, CommandKind::Delete).input(id()),
        "find" => with_inputs(
            CommandDescription::standard_find("command.find", "Task"),
            find_inputs(),
        ),
        "assign" => command(name, CommandKind::Other)
            .input(id())
            .input(local(name, "name", BasicType::Text).required())
            .input(local(name, "email", BasicType::Email).required()),
        "relate" => command(name, CommandKind::Other)
            .input(id())
            .input(
                local(name, "other", BasicType::Reference)
                    .target("Task")
                    .required(),
            )
            .input(
                local(name, "relation", BasicType::Choice)
                    .choices(&RELATIONS)
                    .required(),
            ),
        "label" => command(name, CommandKind::Other)
            .input(id())
            .input(local(name, "label", BasicType::Text).required()),
        "importIcs" => command(name, CommandKind::Other)
            .input(local(name, "ics", BasicType::MultilineText).required())
            .result(
                local(name, "created", BasicType::Reference)
                    .target("Task")
                    .multiple(),
            )
            .result(local(name, "skipped", BasicType::Text).multiple())
            .result(local(name, "failed", BasicType::Text).multiple())
            .result(local(name, "notes", BasicType::Text).multiple()),
        "exportIcs" => command(name, CommandKind::Other).input(id()).result(local(
            name,
            "ics",
            BasicType::MultilineText,
        )),
        "check" => with_inputs(command(name, CommandKind::Check), editable())
            .result(local(name, "issues", BasicType::Text).multiple()),
        "suggest" => command(name, CommandKind::Suggest)
            .input(local(name, "typed", BasicType::Text))
            .result(local(name, "keywords", BasicType::Text).multiple()),
        _ => return None,
    })
}

/// The filters of `find`: the fields of `TaskQuery`.
fn find_inputs() -> Vec<FieldDescription> {
    let n = "find";
    vec![
        local(n, "dueFrom", BasicType::DateTime),
        local(n, "dueTo", BasicType::DateTime),
        local(n, "progress", BasicType::Choice).choices(&PROGRESS),
        local(n, "label", BasicType::Text),
        local(n, "open", BasicType::Choice).choices(&["open"]),
    ]
}
