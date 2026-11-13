//! `check`: run the check command of an objective task and record that it passed.
//!
//! Pattern: Guard chain — origin and creator (`Policy::refusal`), then the program (`Policy::program_ok`), and only
//! then the command runs; every refusal happens before anything is started or written.

use fs_interfaces::tasks_capnp::items;
use fs_objects::ObjectId;
use fs_tasks_core::now_utc;

use crate::edit::{change, task_of};
use crate::exec::execute;
use crate::fail::Failure;
use crate::policy::Policy;
use crate::run::fetch;

pub(crate) async fn check(
    items: &items::Client,
    id: ObjectId,
    policy: &Policy,
) -> Result<String, Failure> {
    let task = task_of(&fetch(items, id).await?.0)?;
    // Nothing runs, and nothing is changed, unless the task was made here by a trusted actor.
    if let Some(why) = policy.refusal(&task) {
        return Err(why.into());
    }
    let command = task.check().ok_or("the task has no check command")?.to_string();
    let argv: Vec<String> = command.split_whitespace().map(String::from).collect();
    policy
        .program_ok(&argv)
        .map_err(|e| format!("check command not run: {e}"))?;
    let outcome = execute(&argv, policy.seconds)?;
    let now = now_utc();
    change(items, id, |t| {
        if t.check() != Some(command.as_str()) {
            return Err("the check command changed while it ran; run it again".into());
        }
        if outcome.passed {
            t.set_checked(&command, &now);
        } else {
            t.clear_checked();
        }
        Ok(())
    })
    .await?;
    if outcome.passed {
        return Ok("check passed".into());
    }
    let how = match (outcome.timed_out, outcome.code) {
        (true, _) => format!("timed out after {} s", policy.seconds),
        (_, Some(c)) => format!("exit {c}"),
        _ => "ended by a signal".into(),
    };
    Err(format!("check failed: {how}\n{}", outcome.tail).trim_end().to_string().into())
}
