//! The type description of `Task`: sound, complete against `requirements.md`, and one table for support.

use fs_tasks_core::describe::support::{self, TABLE};
use fs_tasks_core::{DescribesType, Task};
use fs_types::{BasicType, CommandKind, TypeRegistry};

/// The capability ids of the Required and Optional tables of `docs/en/programs/apps/tasks/requirements.md`
/// (fs-documentation, origin/main, 2026-11-10), taken out of the file once so the test needs no network.
const REQUIREMENT_IDS: &str = include_str!("requirements_ids.txt");

#[test]
fn registry_accepts_the_description() {
    let mut registry = TypeRegistry::new();
    registry.register::<Task>().expect("sound description");
    let task = registry.get("Task").expect("registered");
    assert_eq!(task.program, "fs-tasks");
    assert_eq!(task.standard, "JSCalendar Task (RFC 8984 / 9253)");
}

#[test]
fn every_requirement_has_a_command() {
    let d = Task::describe();
    let ids: Vec<&str> = REQUIREMENT_IDS.lines().filter(|l| !l.is_empty()).collect();
    assert!(ids.len() > 50, "fixture is read: {}", ids.len());
    for id in ids {
        let row = support::row(id).unwrap_or_else(|| panic!("{id} is not in the support table"));
        assert!(d.find_command(row.command).is_some(), "{id} has no command");
    }
}

#[test]
fn every_row_of_the_table_has_a_command_and_a_reason_when_locked() {
    let d = Task::describe();
    for r in TABLE {
        assert!(d.find_command(r.command).is_some(), "{}", r.capability);
        assert_eq!(
            r.supported,
            support::is_supported(r.command),
            "{}",
            r.command
        );
        assert_eq!(r.reason.is_empty(), r.supported, "{}", r.capability);
    }
    assert_eq!(d.commands.len(), support::command_names().len());
}

#[test]
fn a_command_is_supported_or_locked_never_both() {
    // one command shared by several ids must agree with itself
    for r in TABLE {
        let same: Vec<_> = TABLE.iter().filter(|o| o.command == r.command).collect();
        assert!(
            same.iter().all(|o| o.supported == r.supported),
            "{}",
            r.command
        );
    }
}

#[test]
fn the_methods_of_task_items_are_supported_commands() {
    let d = Task::describe();
    for name in [
        "create",
        "change",
        "assign",
        "relate",
        "label",
        "delete",
        "find",
        "importIcs",
        "exportIcs",
        "check",
        "suggest",
    ] {
        assert!(support::is_supported(name), "{name}");
        assert!(d.find_command(name).is_some(), "{name}");
    }
    for name in [
        "boardCreate",
        "cardMove",
        "commentAdd",
        "cardRepeat",
        "webhookCreate",
    ] {
        assert!(!support::is_supported(name), "{name}");
        assert!(
            support::reason(name).is_some_and(|r| !r.is_empty()),
            "{name}"
        );
        assert!(d.find_command(name).is_some(), "{name}");
    }
}

#[test]
fn fields_and_labels() {
    let d = Task::describe();
    for f in [
        "title",
        "description",
        "start",
        "due",
        "estimatedDuration",
        "progress",
        "percentComplete",
        "priority",
        "keywords",
        "participants",
        "relatedTo",
        "locations",
        "links",
        "recurrenceRules",
        "alerts",
        "color",
        "timeZone",
    ] {
        assert!(d.find_field(f).is_some(), "{f}");
    }
    let progress = d.find_field("progress").unwrap();
    assert_eq!(progress.basic, BasicType::Choice);
    assert_eq!(progress.choices.len(), 5);
    assert_eq!(d.find_field("participants").unwrap().target, "Person");
    let change = d.find_command("change").unwrap();
    assert_eq!(change.kind, CommandKind::Change);
    assert!(
        change
            .input
            .iter()
            .any(|f| f.name == "id" && f.target == "Task")
    );
    assert_eq!(d.find_command("find").unwrap().kind, CommandKind::Find);
}
