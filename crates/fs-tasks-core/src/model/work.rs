//! Work-board members of a task: where it came from, how it is checked, the short note.
//!
//! Pattern: Value object — vendor members `freesynergy.net:...` of the task (RFC 8984 section 3.3), kept verbatim in
//! `extra`, so the wire and the store need no change. The core stores and bounds them and interprets nothing; the
//! rule of what may run lives in `fs-tasks-cli` (a check command is executable code).

use serde_json::{Value, json};

use super::task::Task;

/// Where the task came from: `local` (made by `fs-tasks add` on this machine) or `imported` (an `.ics` import).
pub const ORIGIN_KEY: &str = "freesynergy.net:origin";
/// The actor IRI that made the task here.
pub const CREATED_BY_KEY: &str = "freesynergy.net:createdBy";
/// The check command of an objective task: one text, split into words without a shell by whoever runs it.
pub const CHECK_KEY: &str = "freesynergy.net:check";
/// What a passed check recorded: `{ command, at }` (the command that ran, so a changed command is not covered).
pub const CHECKED_KEY: &str = "freesynergy.net:checked";
/// One short line (a result, a hint) for the people who look at the board.
pub const NOTE_KEY: &str = "freesynergy.net:note";

/// The origin of tasks made on this machine.
pub const ORIGIN_LOCAL: &str = "local";
/// The origin of tasks that came in from outside (import, and what sync will bring).
pub const ORIGIN_IMPORTED: &str = "imported";

/// The most bytes of a note and of a check command.
pub const MAX_NOTE_BYTES: usize = 500;

impl Task {
    /// The origin, if one was written. A task without one is foreign.
    pub fn origin(&self) -> Option<&str> {
        self.vendor(ORIGIN_KEY).and_then(Value::as_str)
    }
    /// Set the origin.
    pub fn set_origin(&mut self, origin: &str) {
        self.set_vendor(ORIGIN_KEY, json!(origin));
    }
    /// The actor IRI that made the task here.
    pub fn created_by(&self) -> Option<&str> {
        self.vendor(CREATED_BY_KEY).and_then(Value::as_str)
    }
    /// The check command; a task that has one is objective.
    pub fn check(&self) -> Option<&str> {
        self.vendor(CHECK_KEY).and_then(Value::as_str)
    }
    /// The command a passed check recorded.
    pub fn checked_command(&self) -> Option<&str> {
        self.vendor(CHECKED_KEY)?.get("command")?.as_str()
    }
    /// Record that `command` ran and passed, at `at` (UTC text).
    pub fn set_checked(&mut self, command: &str, at: &str) {
        self.set_vendor(CHECKED_KEY, json!({"command": command, "at": at}));
    }
    /// Forget a recorded check.
    pub fn clear_checked(&mut self) {
        self.extra.remove(CHECKED_KEY);
    }
    /// The note.
    pub fn note(&self) -> Option<&str> {
        self.vendor(NOTE_KEY).and_then(Value::as_str)
    }
    /// Mark the task as imported and drop what a stranger must not bring along: a recorded check.
    pub(crate) fn mark_imported(&mut self) {
        self.set_origin(ORIGIN_IMPORTED);
        self.clear_checked();
    }
}

/// The problems with the form of the work-board members: `(path, what is wrong)`.
pub fn work_issues(t: &Task) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for key in [NOTE_KEY, CHECK_KEY, ORIGIN_KEY, CREATED_BY_KEY] {
        let Some(v) = t.vendor(key) else { continue };
        match v.as_str() {
            None => out.push((format!("/{key}"), "a text".into())),
            Some(s) if s.len() > MAX_NOTE_BYTES => {
                out.push((format!("/{key}"), "too long (at most 500 bytes)".into()));
            }
            Some(_) => {}
        }
    }
    out
}
