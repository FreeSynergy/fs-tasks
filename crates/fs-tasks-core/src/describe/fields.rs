//! The fields of the type `Task` and the inputs of its commands.
//!
//! Pattern: Builder (`coding.md` §2), the chain of `FieldDescription`; one function per field, so the record, `create`,
//! `change` and `check` use the same definition. Basic kinds the JSCalendar members need and `fs-types` does not have
//! are mapped honestly to `text` (duration, whole number, colour, time zone): see `DESIGN.md`.

use fs_types::{BasicType, FieldDescription};

use crate::validate::PROGRESS;

/// The relations of `relate` (RFC 8984 and RFC 9253), as the keys of `Relation`.
pub const RELATIONS: [&str; 9] = [
    "first",
    "next",
    "child",
    "parent",
    "depends-on",
    "finishtostart",
    "finishtofinish",
    "starttostart",
    "starttofinish",
];

/// `camelCase` -> `camel-case`, for the translation keys.
pub fn kebab(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            out.push('-');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// A field of the type: label `task.<name>`.
fn field(name: &str, basic: BasicType) -> FieldDescription {
    FieldDescription::new(name, basic).label(&format!("task.{}", kebab(name)))
}

/// The identity of a task, as a reference to the type itself (the front end fills it from the selection).
pub fn id() -> FieldDescription {
    field("id", BasicType::Reference).target("Task").required()
}

/// A field that belongs to one command only: label `command.<command>.<name>`.
pub fn local(command: &str, name: &str, basic: BasicType) -> FieldDescription {
    FieldDescription::new(name, basic).label(&format!("command.{}.{}", kebab(command), kebab(name)))
}

/// The fields a person can type for a task (inputs of `create`, `change`, `check`), in the order of the form.
pub fn editable() -> Vec<FieldDescription> {
    vec![
        field("title", BasicType::Text).required(),
        field("description", BasicType::MultilineText),
        field("start", BasicType::DateTime),
        field("due", BasicType::DateTime),
        field("estimatedDuration", BasicType::Text),
        field("progress", BasicType::Choice).choices(&PROGRESS),
        field("percentComplete", BasicType::Text),
        field("priority", BasicType::Text),
        field("keywords", BasicType::Keywords),
        field("locations", BasicType::Place).multiple(),
        field("links", BasicType::Link).multiple(),
        field("color", BasicType::Text),
        field("timeZone", BasicType::Text),
    ]
}

/// The fields a stored task shows: the editable ones and those that other commands or the core fill.
pub fn record() -> Vec<FieldDescription> {
    let mut fields = editable();
    fields.insert(
        8,
        field("participants", BasicType::Reference)
            .target("Person")
            .multiple(),
    );
    fields.insert(
        9,
        field("relatedTo", BasicType::Reference)
            .target("Task")
            .multiple(),
    );
    fields.push(field("recurrenceRules", BasicType::Text).multiple());
    fields.push(field("alerts", BasicType::Text).multiple());
    fields
}
