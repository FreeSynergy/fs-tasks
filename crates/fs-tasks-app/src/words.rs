//! The words of the program.
//!
//! Pattern: none — constants (whole sentences with named placeholders, `{name}`) and one pure
//! function that fills them. All English text of this frontend is in this file; translation
//! (ADR 0053) will replace its content, not its users.

use fs_components_workbenches::tasks::TaskLabels;

/// A sentence with its named placeholders (`{name}`) filled from `values`; the sentence stays whole,
/// so a language may put the parts in another order.
pub(crate) fn fill(sentence: &str, values: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(sentence.len());
    let mut rest = sentence;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let named = rest[open + 1..].split_once('}').and_then(|(name, _)| {
            values
                .iter()
                .find(|(given, _)| *given == name)
                .map(|(_, value)| (name.len(), *value))
        });
        if let Some((len, value)) = named {
            out.push_str(value);
            rest = &rest[open + len + 2..];
        } else {
            out.push('{');
            rest = &rest[open + 1..];
        }
    }
    out.push_str(rest);
    out
}

/// Two sentences after one another.
pub(crate) fn sentences(first: &str, next: &str) -> String {
    format!("{first}. {next}")
}

/// What the command line says to a person.
pub(crate) mod cli {
    pub const NO_SOCKET: &str = "no --socket given: the app talks to a running tasks server; start one with `fs-tasks-server --socket <path>` and pass the same path here";
    pub const STORE_GONE: &str = "--store is gone: the store belongs to the server, start it with `fs-tasks-server --data <dir>` and give the app its --socket";
    pub const NEEDS_VALUE: &str = "{option} needs a value";
    pub const UNKNOWN_OPTION: &str = "unknown option {option}";
    pub const SENSITIVE_NEEDS_EXPORT: &str = "--export-sensitive needs --export";
    pub const BAD_VIEW: &str = "--view wants list or status, not {given}";
    pub const CANNOT_REACH: &str = "cannot reach the tasks server at {socket}: {error}";
    pub const NOT_EXPORTED: &str = "{count} could not be exported";
    pub const WROTE: &str = "wrote {file}";
}

/// What an import or an export says in the status line and on the terminal.
pub(crate) mod transfer {
    pub const CANNOT_READ: &str = "cannot read {file}: {error}";
    pub const NO_CALENDAR: &str = "{file} holds no calendar";
    pub const IMPORTED: &str = "Imported {created}, {skipped} already known";
    pub const IMPORTED_FAILED: &str =
        "Imported {created}, {skipped} already known, {count} failed: {list}";
    pub const NOTES: &str = "Notes: {notes}";
    pub const STOPPED: &str = "The import stopped: {why}";
    pub const WROTE: &str = "Wrote {written} to {file}";
    pub const RELEASED: &str = "Released by you: {list}";
    pub const LEFT_OUT: &str =
        "Left out as sensitive (name one with --export-sensitive <uid>): {list}";
    pub const FAILED: &str = "Failed: {list}";
    pub const FAILED_ONE: &str = "{uid}: {error}";
    pub const NAMES_NO_TASK: &str = "--export-sensitive {name} names no task; nothing was written";
}

/// The title of the window.
pub const TITLE: &str = "FreeSynergy Tasks";

/// The words of the workbench.
pub fn labels() -> TaskLabels {
    let mut labels = TaskLabels::default();
    labels.columns = vec![
        field::TITLE.into(),
        field::DUE.into(),
        field::PRIORITY.into(),
        field::STATUS.into(),
    ];
    labels.create = menu::NEW.into();
    labels.view_list = menu::LIST.into();
    labels.view_status = menu::STATUS.into();
    labels
}

/// The name of a `progress` value; a value the standard does not list shows as it is.
pub(crate) fn progress_title(progress: &str) -> &str {
    match progress {
        "needs-action" => "To do",
        "in-process" => "In progress",
        "completed" => "Done",
        "failed" => "Failed",
        "cancelled" => "Cancelled",
        other => other,
    }
}

/// The menus of the window.
pub(crate) mod menu {
    pub const TASK: &str = "Task";
    pub const NEW: &str = "New";
    pub const VIEW: &str = "View";
    pub const LIST: &str = "List";
    pub const STATUS: &str = "Status";
}

/// Headings and labels of what is shown about a task.
pub(crate) mod field {
    pub const SENSITIVE: &str = "Sensitive";
    pub const DETAILS: &str = "Details";
    pub const TITLE: &str = "Title";
    pub const DESCRIPTION: &str = "Description";
    pub const DUE: &str = "Due";
    pub const PRIORITY: &str = "Priority";
    pub const STATUS: &str = "Status";
    pub const LABELS: &str = "Labels (separated by commas; \"a, b\" is one label)";
    pub const LABELS_SHORT: &str = "Labels";
    pub const PERCENT: &str = "Percent done";
    pub const EXTENDED: &str = "Extended status";
    pub const ZONE: &str = "Time zone";
    pub const REPEATS: &str = "Repeats";
    pub const YES: &str = "Yes";
    pub const PEOPLE: &str = "Participants";
    pub const RELATED: &str = "Related tasks";
}

/// Sentences about what an edit or a move can do.
pub(crate) mod say {
    pub const SERIES_SHEET: &str = "This task repeats. Edit changes the title, description, priority and labels of all its occurrences. Its due date and status cannot be changed here.";
    pub const SERIES_FORM: &str = "You are editing all occurrences of this repeating task. Its due date and status cannot be changed here, because that could damage the series.";
    pub const SERIES_REFUSED: &str = "this task repeats, and changing its due date or status here could damage the series. Nothing was changed";
    pub const SERIES_MOVE: &str =
        "This task repeats, and its status belongs to each occurrence. It cannot be moved here.";
    pub const COMPUTED: &str = "This status is worked out from the participants; the task does not state one itself. Moving it makes the status fixed, and it cannot go back to worked out here.";
    pub const NEEDS_TITLE: &str = "enter a title";
    pub const BAD_DUE: &str = "enter the due date with a time, like 2026-11-30 17:00";
    pub const BAD_PRIORITY: &str = "the priority is a number from 0 to 9, or empty";
    pub const CHANGED_SINCE: &str = "Someone else changed this task in the meantime.";
    pub const NO_COLUMN: &str = "there is no such status";
    pub const GONE: &str = "This task no longer exists.";
    pub const UNKNOWN_FIELD: &str = "the form has an unknown field \"{field}\"";
    pub const NEEDS_ENCRYPTION: &str = "this task is marked sensitive and this store is not encrypted, so it was refused ({error})";
    pub const LINE: &str = "{label}: {value}";
}

/// `--socket` was not given.
pub fn no_socket() -> String {
    cli::NO_SOCKET.to_string()
}

/// The app could not connect to the server.
pub fn cannot_reach(socket: &str, error: &str) -> String {
    fill(cli::CANNOT_REACH, &[("socket", socket), ("error", error)])
}

/// Some tasks could not be exported.
pub fn not_exported(count: usize) -> String {
    fill(cli::NOT_EXPORTED, &[("count", &count.to_string())])
}

/// A picture was written.
pub fn wrote(file: &str) -> String {
    fill(cli::WROTE, &[("file", file)])
}

/// The help text.
pub const HELP: &str = "fs-tasks-app: the tasks program

  --socket <path>    Unix socket of a running `fs-tasks-server --socket <path>` (required)
  --import <file>    import an iCalendar file first; tasks already known are skipped, never replaced
  --export <file>    write all tasks to a new iCalendar file and stop; an existing file is not
                     replaced, tasks marked sensitive are left out and named
  --export-sensitive <uid>
                     release this task marked sensitive for --export; repeat for each task
  --snapshot <png>   draw a picture instead of opening a window
  --size <WxH>       window or picture size (default 1300x800)
  --view <list|status>
                     the view to start with (default list)";

#[cfg(test)]
mod tests {
    use super::fill;

    #[test]
    fn a_value_that_looks_like_a_placeholder_is_shown_as_it_is() {
        let said = fill("{uid}: {error}", &[("uid", "{error}"), ("error", "boom")]);
        assert_eq!(said, "{error}: boom");
        let same = fill("{a} {b}", &[("a", "{b}"), ("b", "x")]);
        assert_eq!(same, "{b} x");
    }
}
