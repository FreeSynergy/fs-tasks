//! The words of the window (English until the catalog of ADR 0057 serves them).
//!
//! Pattern: Strategy (`Say`, a closure of `fs-components-fields`) — a key becomes a text. The fixed words of the
//! frontend and its problem codes are a table; the keys of the type description (`task.due-from`,
//! `command.board-create.name`) are made readable from the key itself, so a new field needs no entry here.

use std::sync::Arc;

use fs_components_fields::Say;

const FIXED: &[(&str, &str)] = &[
    ("menu.program", "Program"),
    ("menu.quit", "Quit"),
    ("menu.view", "View"),
    ("menu.help", "Help"),
    ("menu.about", "About"),
    ("about.version", "Version"),
    ("about.licence", "Licence"),
    ("about.origin", "Origin"),
    ("about.repository", "Repository"),
    ("about.unoffered", "Not offered here"),
    ("action.add", "Add"),
    ("action.back", "Back"),
    ("action.cancel", "Cancel"),
    ("action.close", "Close"),
    ("action.remove", "Remove"),
    ("action.save", "Save"),
    ("confirm.sure", "Are you sure?"),
    ("detail.not-loaded", "This task is not loaded."),
    ("list.empty", "No tasks yet."),
    ("list.separator", ", "),
    (
        "list.stale",
        "The program cannot be reached; this list may be out of date.",
    ),
    ("value.none", "none"),
    ("sort.ascending", "ascending"),
    ("sort.descending", "descending"),
    ("type.task", "Tasks"),
    ("view.board", "Status board"),
    ("view.timeline", "Timeline"),
    ("board.column.needs-action", "To do"),
    ("board.column.in-process", "In progress"),
    ("board.column.completed", "Done"),
    ("board.column.failed", "Failed"),
    ("board.column.cancelled", "Cancelled"),
    ("kanban.soon", "Comes later"),
    ("kanban.empty", "No tasks"),
    ("kanban.grab", "Pick up"),
    ("kanban.drop", "Drop here"),
    ("kanban.move_left", "Move left"),
    ("kanban.move_right", "Move right"),
    ("kanban.add_column", "Add column"),
    ("kanban.rename", "Rename column"),
    ("kanban.limit", "Set limit"),
    ("kanban.refused.unsupported", "Not possible yet."),
    (
        "kanban.refused.changed",
        "The task was changed by someone else; look again.",
    ),
    ("kanban.refused.rejected", "The task does not accept this."),
    ("kanban.refused.gone", "The task is gone."),
    (
        "kanban.refused.unavailable",
        "The program cannot be reached.",
    ),
    ("timeline.zoom.day", "Day"),
    ("timeline.zoom.week", "Week"),
    ("timeline.zoom.month", "Month"),
    ("timeline.month.1", "Jan"),
    ("timeline.month.2", "Feb"),
    ("timeline.month.3", "Mar"),
    ("timeline.month.4", "Apr"),
    ("timeline.month.5", "May"),
    ("timeline.month.6", "Jun"),
    ("timeline.month.7", "Jul"),
    ("timeline.month.8", "Aug"),
    ("timeline.month.9", "Sep"),
    ("timeline.month.10", "Oct"),
    ("timeline.month.11", "Nov"),
    ("timeline.month.12", "Dec"),
    ("problem.empty", "Nothing typed."),
    ("problem.missing", "This is needed."),
    ("problem.incomplete", "Incomplete."),
    (
        "problem.unknown-field",
        "The program does not know this field.",
    ),
    ("problem.not-multiple", "Only one value is possible here."),
    ("problem.not-a-number", "Not a number."),
    ("problem.too-long", "Too long."),
    ("problem.character", "This value is not accepted."),
    ("problem.choice.not-in-list", "Not one of the choices."),
    (
        "problem.date-time.bad",
        "Not a date and time with an offset, like 2026-11-10T09:30:00+01:00.",
    ),
    ("problem.uri.bad", "Not an address."),
    (
        "problem.uri.not-absolute",
        "An address needs a scheme, like https:.",
    ),
    (
        "problem.uri.unsafe-scheme",
        "This kind of address is not allowed.",
    ),
    (
        "problem.place.bad",
        "Not a place, like geo:48.2010,16.3695.",
    ),
    (
        "problem.place.range",
        "Latitude or longitude is out of range.",
    ),
    (
        "problem.keywords.bad",
        "A keyword is too long or has a character it may not.",
    ),
    ("task.estimated-duration", "Estimated duration (like PT2H)"),
    ("task.percent-complete", "Percent complete"),
    ("task.related-to", "Related to"),
    ("task.time-zone", "Time zone"),
];

/// The text for `key`.
pub fn say(key: &str) -> String {
    if let Some((_, text)) = FIXED.iter().find(|(k, _)| *k == key) {
        return (*text).to_owned();
    }
    let words = key.rsplit('.').next().unwrap_or(key).replace('-', " ");
    let mut chars = words.chars();
    let first = chars.next().map(|c| c.to_uppercase().collect::<String>());
    first.map_or_else(|| key.to_owned(), |f| f + chars.as_str())
}

/// The words as a `Say`.
pub fn words() -> Arc<dyn Say> {
    Arc::new(|key: &str| say(key))
}
