//! What a worker does with a task: claim, release, set the status, leave a note, take the next one, check.
//!
//! Pattern: Command (executor) — each step is one `edit::change` of the task, so it is decided on the stored task
//! and refused (conflict, exit 3) if somebody else's claim holds. The rules of what may run as a check are in
//! `policy`; here they are only asked.

use chrono::{DateTime, Duration, Utc};
use fs_interfaces::rpc::write_uuid;
use fs_interfaces::tasks_capnp::items;
use fs_objects::ObjectId;
use fs_tasks_core::contract::readiness_from_wire;
use fs_tasks_core::model::{ActorType, MAX_CLAIM_HOURS, MAX_NOTE_BYTES, NOTE_KEY};
use fs_tasks_core::plan::Readiness;
use fs_tasks_core::{Task, now_utc};
use serde_json::{Value, json};

use crate::args::{To, Work};
use crate::check::check;
use crate::edit::{all_tasks, change, project_of, status_word, view};
use crate::fail::Failure;
use crate::policy::{Policy, actor_iri};
use crate::secrets::refuse_secret;

/// How long a claim runs when `--until` is not given.
const DEFAULT_HOURS: i64 = 2;
/// The most bytes of a report path or URL.
const MAX_REPORT: usize = 500;

pub(crate) fn id_of(text: &str) -> Result<ObjectId, String> {
    text.trim()
        .parse()
        .map_err(|_| format!("{text:?} is not a task id"))
}

/// A worker name becomes an actor IRI; only plain names, so the IRI is one actor and nothing else.
pub(crate) fn worker(name: &str) -> Result<String, String> {
    let plain = !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if plain {
        Ok(actor_iri(name))
    } else {
        Err("a worker name is 1 to 64 letters, digits, `.`, `_` or `-`".into())
    }
}

/// `90m`, `2h`, or a UTC time `2026-11-13T18:00:00Z`; at most 24 hours from now.
fn until_of(text: Option<&str>, now: DateTime<Utc>) -> Result<DateTime<Utc>, String> {
    let Some(text) = text else {
        return Ok(now + Duration::hours(DEFAULT_HOURS));
    };
    let span = |digits: &str, unit: fn(i64) -> Duration| {
        digits.parse::<i64>().ok().filter(|n| *n > 0).map(unit)
    };
    let until = match text.split_at(text.len().saturating_sub(1)) {
        (d, "m") => span(d, Duration::minutes).map(|s| now + s),
        (d, "h") => span(d, Duration::hours).map(|s| now + s),
        _ => fs_tasks_core::utc(text).ok(),
    }
    .ok_or("--until is a length like 90m or 2h, or a UTC time like 2026-11-13T18:00:00Z")?;
    if until <= now || until > now + Duration::hours(MAX_CLAIM_HOURS) {
        return Err(format!(
            "--until is within the next {MAX_CLAIM_HOURS} hours"
        ));
    }
    Ok(until)
}

/// Take the task for the worker; a live claim of somebody else is a conflict.
fn take(task: &mut Task, name: &str, iri: &str, until: DateTime<Utc>) -> Result<(), Failure> {
    if let Some(held) = task.claimed_at(Utc::now())
        && held.by != iri
    {
        return Err(Failure::conflict(format!(
            "the task is claimed by {} until {}",
            held.by,
            held.until.format("%Y-%m-%dT%H:%M:%SZ")
        )));
    }
    if !matches!(task.effective_progress(), "needs-action" | "in-process") {
        return Err(format!("the task is {}, not open", status_word(task)).into());
    }
    task.set_claim(iri, until);
    task.assign_actor(iri, ActorType::Application, name);
    Ok(())
}

fn short(what: &str, text: &str, most: usize) -> Result<(), String> {
    refuse_secret(what, text)?;
    if text.len() > most {
        return Err(format!("{what} is at most {most} bytes"));
    }
    Ok(())
}

/// Done needs, for an objective task, a passed check of the command as it stands, by a task that may run one.
fn may_finish(task: &Task, policy: &Result<Policy, String>) -> Result<(), Failure> {
    let Some(command) = task.check() else {
        return Ok(());
    };
    // A policy that cannot be read trusts nobody.
    let refusal = match policy {
        Ok(p) => p.refusal(task),
        Err(_) => Some("check command not run: settings unreadable"),
    };
    if let Some(why) = refusal {
        return Err(why.into());
    }
    if task.checked_command() != Some(command) {
        return Err("done needs a passed check: run `fs-tasks check <id>` first".into());
    }
    Ok(())
}

/// Set `done` (the plain `done` command and `status done`).
pub(crate) async fn finish(
    items: &items::Client,
    id: ObjectId,
    policy: &Result<Policy, String>,
) -> Result<Task, Failure> {
    change(items, id, |t| {
        may_finish(t, policy)?;
        t.progress = Some("completed".into());
        t.progress_updated = Some(now_utc());
        Ok(())
    })
    .await
}

async fn status(
    items: &items::Client,
    id: ObjectId,
    (to, note, report, cost): (To, Option<String>, Option<String>, Option<String>),
    policy: &Result<Policy, String>,
) -> Result<Task, Failure> {
    for (what, text) in [("the note", &note), ("the report", &report)] {
        if let Some(text) = text {
            short(
                what,
                text,
                if what == "the note" {
                    MAX_NOTE_BYTES
                } else {
                    MAX_REPORT
                },
            )?;
        }
    }
    let cost = cost.map(|c| c.parse::<f64>().unwrap_or(0.0));
    change(items, id, |t| {
        match to {
            To::InProgress => t.progress = Some("in-process".into()),
            To::Done | To::Failed => {
                if to == To::Done {
                    may_finish(t, policy)?;
                }
                t.progress = Some(
                    if to == To::Done {
                        "completed"
                    } else {
                        "failed"
                    }
                    .into(),
                );
                t.clear_claim();
                if report.is_some() || cost.is_some() {
                    t.set_result(json!({"report": report, "cost_usd": cost}));
                }
            }
        }
        if let Some(n) = &note {
            t.set_vendor(NOTE_KEY, json!(n));
        }
        t.progress_updated = Some(now_utc());
        Ok(())
    })
    .await
}

async fn ready(items: &items::Client, id: ObjectId) -> Result<bool, Failure> {
    let mut req = items.readiness_request();
    write_uuid(req.get().init_id(), Some(id.as_uuid()));
    let answer = req.send().promise.await.map_err(|e| e.to_string())?;
    let report = answer
        .get()
        .and_then(items::readiness_results::Reader::get_report)
        .map_err(|e| e.to_string())?;
    let by = report
        .get_by()
        .map_err(|e| e.to_string())?
        .iter()
        .map(|t| {
            Ok(t.map_err(|e| e.to_string())?
                .to_string()
                .unwrap_or_default())
        })
        .collect::<Result<Vec<_>, String>>()?;
    let state = report
        .get_readiness()
        .map_err(|e| capnp::Error::from(e).to_string())?;
    Ok(matches!(readiness_from_wire(state, by), Readiness::Ready))
}

/// The first open, ready task without somebody else's claim (the worker's own claim counts), claimed.
async fn next(
    items: &items::Client,
    name: &str,
    project: Option<&str>,
) -> Result<Option<(ObjectId, Task)>, Failure> {
    let iri = worker(name)?;
    let until = until_of(None, Utc::now())?;
    for (id, task, _) in all_tasks(items).await? {
        let free = task.claimed_at(Utc::now()).is_none_or(|c| c.by == iri);
        if task.effective_progress() != "needs-action"
            || !free
            || project.is_some_and(|p| project_of(&task) != Some(p))
            || !ready(items, id).await?
        {
            continue;
        }
        match change(items, id, |t| take(t, name, &iri, until)).await {
            Ok(task) => return Ok(Some((id, task))),
            // Somebody was quicker: the next one.
            Err(f) if f.conflict => {}
            Err(f) => return Err(f),
        }
    }
    Ok(None)
}

fn answer(id: ObjectId, task: &Task, json: bool, plain: &str) -> String {
    if json {
        view(id, task).to_string()
    } else {
        format!("{id}  {plain}")
    }
}

/// Carry out a worker step; the text to print.
pub(crate) async fn run_work(
    items: &items::Client,
    work: Work,
    json: bool,
    policy: &Result<Policy, String>,
) -> Result<String, Failure> {
    match work {
        Work::Claim { id, name, until } => {
            let (id, iri) = (id_of(&id)?, worker(&name)?);
            let until = until_of(until.as_deref(), Utc::now())?;
            let t = change(items, id, |t| take(t, &name, &iri, until)).await?;
            Ok(answer(id, &t, json, &format!("claimed by {name}")))
        }
        Work::Release { id, name } => {
            let (id, iri) = (id_of(&id)?, worker(&name)?);
            let t = change(items, id, |t| {
                if let Some(held) = t.claimed_at(Utc::now())
                    && held.by != iri
                {
                    return Err(Failure::conflict(format!(
                        "the claim belongs to {}, not to {iri}",
                        held.by
                    )));
                }
                t.clear_claim();
                Ok(())
            })
            .await?;
            Ok(answer(id, &t, json, "released"))
        }
        Work::Status {
            id,
            to,
            note,
            report,
            cost,
        } => {
            let id = id_of(&id)?;
            let t = status(items, id, (to, note, report, cost), policy).await?;
            Ok(answer(id, &t, json, status_word(&t)))
        }
        Work::Note { id, text } => {
            let id = id_of(&id)?;
            short("the note", &text, MAX_NOTE_BYTES)?;
            let t = change(items, id, |t| {
                t.set_vendor(NOTE_KEY, Value::String(text.clone()));
                Ok(())
            })
            .await?;
            Ok(answer(id, &t, json, "noted"))
        }
        Work::Next { name, project } => match next(items, &name, project.as_deref()).await? {
            Some((id, t)) => Ok(answer(id, &t, json, &t.display_title())),
            None if json => Ok("null".into()),
            None => Ok(String::new()),
        },
        Work::Check(id) => {
            let policy = policy.as_ref().map_err(|e| Failure::from(e.clone()))?;
            check(items, id_of(&id)?, policy).await
        }
    }
}
