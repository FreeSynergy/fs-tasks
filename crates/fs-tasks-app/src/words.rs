//! The words of the program.
//!
//! Pattern: Value objects — all English text of this frontend in one place; translation (ADR 0053)
//! will replace this file's content, not its users.

use fs_components_workbenches::tasks::TaskLabels;

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

/// Headings and labels of what is shown about a task.
pub(crate) mod field {
    pub const SENSITIVE: &str = "Sensitive";
    pub const DETAILS: &str = "Details";
    pub const TITLE: &str = "Title";
    pub const DESCRIPTION: &str = "Description";
    pub const DUE: &str = "Due";
    pub const PRIORITY: &str = "Priority";
    pub const STATUS: &str = "Status";
    pub const LABELS: &str = "Labels (separated by commas)";
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
    pub const COMPUTED: &str =
        "This status is worked out from the participants; the task does not state one itself.";
    pub const NEEDS_TITLE: &str = "enter a title";
    pub const BAD_DUE: &str = "enter the due date with a time, like 2026-11-30 17:00";
    pub const BAD_PRIORITY: &str = "the priority is a number from 0 to 9, or empty";
    pub const CHANGED_SINCE: &str = "Someone else changed this task in the meantime.";
    pub const NO_COLUMN: &str = "there is no such status";
}
