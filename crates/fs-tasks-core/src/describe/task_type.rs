//! `Task` reports itself: name, owner, standard, fields and every command (ADR 0066).
//!
//! Pattern: Registry entry (`DescribesType`, ADR 0043 trait first) — the trait is `fs_types::DescribesType`; the
//! description is built from the field definitions (`fields.rs`) and from the support table (`support.rs`): every
//! command of the table is in it, the supported ones in full, the others locked (`locked.rs`).

use fs_types::{DescribesType, TypeDescription};

use super::{commands, fields, locked, support};
use crate::model::Task;

/// The standard the type follows.
pub const STANDARD: &str = "JSCalendar Task (RFC 8984 / 9253)";

impl DescribesType for Task {
    fn describe() -> TypeDescription {
        let mut t = TypeDescription::new("Task", "fs-tasks", STANDARD, "task");
        for f in fields::record() {
            t = t.field(f);
        }
        for name in support::command_names() {
            let command = commands::supported(name).unwrap_or_else(|| locked::locked(name));
            // SWITCH: the one place where the support table becomes the `supported` flag of the description.
            t = t.command(if support::is_supported(name) {
                command
            } else {
                command.unsupported()
            });
        }
        t
    }
}
