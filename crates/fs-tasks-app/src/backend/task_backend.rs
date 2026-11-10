//! `TaskBackend`: the generated Admin frontend's `Backend` over the tasks program's own interface.
//!
//! Pattern: Adapter + Command — the frontend's trait (`fs_admin_frontend::Backend`, Repository) over `TaskItems`
//! (ADR 0043: the core's trait, never `TaskRepo`). `run` picks the function of `commands.rs` by command name; a command
//! the support table of the core marks as not supported is refused before anything else (a last guard: the front end of
//! this revision cannot show the lock itself, see `DESIGN.md`).

use std::sync::Arc;

use fs_admin_frontend::{Backend, BackendError, Outcome, Record, Values};
use fs_tasks_core::{DescribesType, Task, TaskItems};
use fs_types::{BasicRules, CommandKind, TypeDescription};
use tokio::runtime::Handle;

use super::commands::Ctx;
use super::issues;

/// The tasks of one provider as records of the type `Task`. Do not call it from inside the runtime: it blocks the
/// calling thread until the core has answered.
pub struct TaskBackend {
    items: Arc<dyn TaskItems>,
    runtime: Handle,
    description: TypeDescription,
}

impl TaskBackend {
    /// A backend over `items`, driven with `runtime`.
    pub fn new(items: Arc<dyn TaskItems>, runtime: Handle) -> Self {
        Self {
            items,
            runtime,
            description: Task::describe(),
        }
    }

    fn ctx(&self) -> Ctx<'_> {
        Ctx {
            items: &self.items,
            runtime: &self.runtime,
            description: &self.description,
        }
    }

    /// The input checked against the description, as a program must (the front end checked it too).
    fn check(&self, name: &str, input: &Values) -> Result<(), BackendError> {
        let command = self
            .description
            .find_command(name)
            .ok_or(BackendError::NotFound)?;
        if command.kind == CommandKind::Change {
            // the input of `change` is an Edit, checked field by field in `commands.rs`
            return Ok(());
        }
        let mut shape = TypeDescription::new("Input", "fs-tasks", "none", "input");
        shape.fields.clone_from(&command.input);
        let issues = shape.check_record(&BasicRules::standard(), input);
        if issues.is_empty() {
            Ok(())
        } else {
            Err(issues::refused(&issues))
        }
    }
}

impl Backend for TaskBackend {
    fn list(&self, query: &Values) -> Result<Vec<Record>, BackendError> {
        self.check("find", query)?;
        self.ctx().find(query)
    }

    fn run(&self, name: &str, input: &Values) -> Result<Outcome, BackendError> {
        let command = self
            .description
            .find_command(name)
            .ok_or(BackendError::NotFound)?;
        // the guard: the shipped frontend cannot show the lock, so a command the description reports as not supported
        // is refused here
        if !command.supported {
            return Err(BackendError::Unavailable(format!("unsupported: {name}")));
        }
        self.check(name, input)?;
        let ctx = self.ctx();
        match name {
            "create" => ctx.create(input),
            "change" => ctx.change(input),
            "delete" => ctx.delete(input),
            "assign" => ctx.assign(input),
            "label" => ctx.label(input),
            "relate" => ctx.relate(input),
            "importIcs" => ctx.import_ics(input),
            "exportIcs" => ctx.export_ics(input),
            "check" => ctx.check(input),
            "suggest" => ctx.suggest(input),
            "find" => Err(BackendError::Unavailable("find is read with list".into())),
            _ => Err(BackendError::Unavailable(format!("unsupported: {name}"))),
        }
    }
}
