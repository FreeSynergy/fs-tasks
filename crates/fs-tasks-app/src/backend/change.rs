//! `change`: the fields sent become an `Edit`, checked, merged into the task and written against its revision.
//!
//! Pattern: Command (the partial change of ADR 0066) — the rule lives here and in `write.rs::merge`: a field not sent
//! stays, an empty one clears, values replace.

use fs_admin_frontend::{BackendError, Outcome, Values};
use fs_tasks_core::{Revision, Task};
use fs_types::{BasicRules, Edit, RecordIssue, RecordProblem, TypeProblem};

use super::commands::{Ctx, done, id_of};
use super::issues::{backend_error, issue, refused};
use super::read::record;
use super::write::merge;

/// Fields the description lists but this program does not write through `change`: they are shown, and `change`
/// accepts them only unchanged (people and relations have their own commands; rules and alerts are `Unsupported`).
const READ_ONLY: [&str; 4] = ["participants", "relatedTo", "recurrenceRules", "alerts"];

impl Ctx<'_> {
    /// `change`: the fields sent become an `Edit`, checked by the rules of `fs-types`; read, merge, write if nobody
    /// wrote in between.
    pub fn change(&self, input: &Values) -> Result<Outcome, BackendError> {
        let id = id_of(input, "id")?;
        let entry = self
            .wait(self.items.get(id))?
            .ok_or(BackendError::NotFound)?;
        let revision = Revision::of(&entry);
        let sent = self.checked_edit(input, &record(&entry).values)?;
        let mut json = entry.task.to_value();
        merge(&mut json, &sent).map_err(|e| refused(&e))?;
        let task = Task::from_value(json).map_err(|e| backend_error(&e.into()))?;
        Ok(done(
            self.wait(self.items.edit_checked(id, revision, task))?.id,
        ))
    }

    /// The values of the edit in canonical form; a field that is cleared is an empty list.
    fn checked_edit(&self, input: &Values, current: &Values) -> Result<Values, BackendError> {
        let mut edit = Edit::new();
        let mut found: Vec<RecordIssue> = Vec::new();
        for (name, values) in input.iter().filter(|(n, _)| *n != "id") {
            let Some(field) = self.description.find_field(name) else {
                found.push(RecordIssue {
                    field: name.clone(),
                    index: 0,
                    problem: RecordProblem::UnknownField,
                });
                continue;
            };
            let same = current.get(name).map_or(&[][..], Vec::as_slice) == values.as_slice();
            if READ_ONLY.contains(&name.as_str()) {
                if !same {
                    found.push(issue(name, TypeProblem::Character));
                }
                continue;
            }
            let texts: Vec<&str> = values.iter().map(String::as_str).collect();
            edit = edit.set(&[name], field.basic, &texts);
        }
        match edit.check(self.description, &BasicRules::standard()) {
            Ok(canonical) if found.is_empty() => Ok(canonical
                .fields
                .into_iter()
                .map(|f| (f.path.join("/"), f.values))
                .collect()),
            Ok(_) => Err(refused(&found)),
            Err(mut issues) => {
                issues.extend(found);
                Err(refused(&issues))
            }
        }
    }
}
