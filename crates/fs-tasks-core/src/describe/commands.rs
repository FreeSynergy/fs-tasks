//! The commands the core does: one per method of `TaskItems` (plus `change`, `find`, `check`, `suggest`).
//!
//! Pattern: Builder, as `fields.rs`; the name of a command is the key of the support table (`support.rs`).
//! `change` is partial (ADR 0066): a field that is not sent stays, an empty list clears it, values replace.

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
        "change" => {
            let mut inputs = vec![id()];
            inputs.extend(editable());
            with_inputs(command(name, CommandKind::Change), inputs)
        }
        "delete" => command(name, CommandKind::Delete).input(id()),
        "find" => with_inputs(command(name, CommandKind::Find), find_inputs()),
        "assign" => command(name, CommandKind::Change)
            .input(id())
            .input(local(name, "name", BasicType::Text).required())
            .input(local(name, "email", BasicType::Email).required()),
        "relate" => command(name, CommandKind::Change)
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
        "label" => command(name, CommandKind::Change)
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
        local(n, "text", BasicType::Text),
        local(n, "dueFrom", BasicType::DateTime),
        local(n, "dueTo", BasicType::DateTime),
        local(n, "progress", BasicType::Choice).choices(&PROGRESS),
        local(n, "label", BasicType::Text),
        local(n, "open", BasicType::Choice).choices(&["open"]),
    ]
}
