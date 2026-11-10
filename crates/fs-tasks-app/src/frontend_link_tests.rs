//! Tests of the temporary converter: the shipped frontend accepts what it gets and offers what it should.
//!
//! Pattern: none (tests).
#![allow(clippy::pedantic)]

use fs_admin_frontend::{Placement, plan_commands, unoffered_commands};
use fs_tasks_core::{DescribesType, Task};
use fs_types_frontend::{CommandKind, TypeRegistry};

use super::description;

#[test]
fn the_frontend_registry_accepts_the_converted_description() {
    let d = description(
        &Task::describe(),
        &["participants", "relatedTo", "recurrenceRules", "alerts"],
    );
    let mut registry = TypeRegistry::new();
    registry
        .add(d.clone())
        .expect("sound for the shipped frontend");
    assert_eq!(d.commands.len(), Task::describe().commands.len());
}

#[test]
fn change_gets_the_record_id_and_the_editable_fields_and_is_offered_on_the_record() {
    let d = description(
        &Task::describe(),
        &["participants", "relatedTo", "recurrenceRules", "alerts"],
    );
    let change = d.find_command("change").unwrap();
    assert_eq!(change.kind, CommandKind::Change);
    assert_eq!(change.input[0].name, "id");
    assert!(change.input.iter().any(|f| f.name == "due"));
    assert!(!change.input.iter().any(|f| f.name == "participants"));
    let plans = plan_commands(&d);
    let plan = plans.iter().find(|p| p.name == "change").unwrap();
    assert_eq!(plan.placement, Placement::OnRecord);
    assert_eq!(plan.bound.as_deref(), Some("id"));
    // every command of the description is offered: the locked ones too (the frontend cannot show the lock)
    assert!(unoffered_commands(&d).is_empty());
    assert!(plans.iter().any(|p| p.name == "cardMove"));
}
