//! The support table: which command of `requirements.md` the core does, and why not where it does not.
//!
//! Pattern: Registry (`coding.md` §2) — one table, `capability id -> command name, supported, reason`, is the single
//! source for the type description, for the refusal of a front end and for the self-check against `requirements.md`.
//! The description carries the flag (`CommandDescription::supported`, fs-interfaces v0.0.11); `task_type.rs` sets it from
//! this table in one line (`SWITCH`). A front end and the backend read the flag of the description, not this table.

/// One capability id of `requirements.md` (or of the contract) and the command that stands for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    /// The capability id as written in `requirements.md` or `tasks.capnp` (`card.move`).
    pub capability: &'static str,
    /// The command of the description (lower camel case: `cardMove`). Several ids may share one command.
    pub command: &'static str,
    /// The core does it.
    pub supported: bool,
    /// English, for the log and `DESIGN.md`: why it is not supported (empty where it is).
    pub reason: &'static str,
}

const fn yes(capability: &'static str, command: &'static str) -> Row {
    Row {
        capability,
        command,
        supported: true,
        reason: "",
    }
}

const fn no(capability: &'static str, command: &'static str, reason: &'static str) -> Row {
    Row {
        capability,
        command,
        supported: false,
        reason,
    }
}

const BOARD: &str = "boards are not built; the Boards interface is a placeholder";
const PEOPLE: &str = "membership and rights belong to fs-groups; not connected";
const HISTORY: &str = "history belongs to fs-history; not connected";
const OWN: &str = "no model or no interface method in the core yet";

/// Every capability id, in the order of `requirements.md` (Required, Optional), then the ids of the contract.
pub const TABLE: &[Row] = &[
    no("board.create", "boardCreate", BOARD),
    no("board.edit", "boardEdit", BOARD),
    no("board.open", "boardOpen", BOARD),
    no("column.create", "columnCreate", BOARD),
    no("column.rename", "columnRename", BOARD),
    no("column.move", "columnMove", BOARD),
    yes("card.create", "create"),
    yes("card.edit", "change"),
    no(
        "card.move",
        "cardMove",
        "a card is in a column through task_link, which is not started",
    ),
    no(
        "card.setStatus",
        "cardSetStatus",
        "RFC 10051 status has no interface method; progress is changed with change",
    ),
    yes("card.assign", "assign"),
    yes("card.setDue", "change"),
    yes("label.assign", "label"),
    no(
        "subtask.add",
        "subtaskAdd",
        "progress from the children is not started; relate with child works",
    ),
    yes("relation.create", "relate"),
    no(
        "attachment.add",
        "attachmentAdd",
        "the file is kept by fs-files; not connected",
    ),
    no(
        "comment.add",
        "commentAdd",
        "comments are AS2 notes; no model yet",
    ),
    no("card.watch", "cardWatch", "no subscription list yet"),
    no(
        "card.notify",
        "cardNotify",
        "delivery is a platform matter; not defined",
    ),
    no("card.history", "cardHistory", HISTORY),
    no("field.define", "fieldDefine", BOARD),
    no("card.setField", "cardSetField", OWN),
    no("template.apply", "templateApply", BOARD),
    no("member.add", "memberAdd", PEOPLE),
    no("member.role", "memberRole", PEOPLE),
    no("webhook.create", "webhookCreate", OWN),
    no("board.export", "boardExport", BOARD),
    no(
        "board.sync",
        "boardSync",
        "subscribe exists in TaskFeed; live sync of a board is not built",
    ),
    no("swimlane.create", "swimlaneCreate", BOARD),
    no("column.limit", "columnLimit", BOARD),
    no("epic.create", "epicCreate", OWN),
    no("card.type", "cardType", OWN),
    no("workflow.define", "workflowDefine", OWN),
    yes("card.priority", "change"),
    no(
        "card.remind",
        "cardRemind",
        "alerts are kept but cannot be edited yet",
    ),
    no(
        "card.repeat",
        "cardRepeat",
        "no basic type for a recurrence rule; recurrence rules are kept but cannot be edited",
    ),
    no("comment.mention", "commentMention", OWN),
    yes("card.locate", "change"),
    no("card.copy", "cardCopy", OWN),
    no("card.share", "cardShare", OWN),
    no("card.archive", "cardArchive", OWN),
    no("board.archive", "boardArchive", BOARD),
    no("board.style", "boardStyle", BOARD),
    yes("card.search", "find"),
    no("query.save", "querySave", OWN),
    yes("card.list", "find"),
    no("rule.create", "ruleCreate", OWN),
    no("form.submit", "formSubmit", OWN),
    no("card.linkCommit", "cardLinkCommit", OWN),
    no("board.import", "boardImport", BOARD),
    yes("task.schedule", "change"),
    no(
        "task.filter",
        "taskFilter",
        "no query language; find takes the fields of TaskQuery",
    ),
    no("task.triage", "taskTriage", OWN),
    no("cycle.create", "cycleCreate", OWN),
    no("task.urgency", "taskUrgency", OWN),
    no("project.review", "projectReview", OWN),
    yes("task.create", "create"),
    yes("task.edit", "change"),
    yes("task.assign", "assign"),
    yes("task.delete", "delete"),
    yes("task.import", "importIcs"),
    yes("task.export", "exportIcs"),
    yes("task.check", "check"),
    yes("task.suggest", "suggest"),
];

/// The row of a capability id.
pub fn row(capability: &str) -> Option<&'static Row> {
    TABLE.iter().find(|r| r.capability == capability)
}

/// The command names of the description, each once, in table order.
pub fn command_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = Vec::new();
    for r in TABLE {
        if !names.contains(&r.command) {
            names.push(r.command);
        }
    }
    names
}

/// Whether the core does the command. Unknown names are not supported.
pub fn is_supported(command: &str) -> bool {
    TABLE.iter().any(|r| r.command == command && r.supported)
}

/// Why the command is not supported (`None` where it is, or where the name is unknown).
pub fn reason(command: &str) -> Option<&'static str> {
    TABLE
        .iter()
        .find(|r| r.command == command && !r.supported)
        .map(|r| r.reason)
}
