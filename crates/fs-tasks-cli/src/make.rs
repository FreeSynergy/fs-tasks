//! Making, listing and finishing tasks.
//!
//! Pattern: Command (executor) — `add` writes the members the work board needs (origin, creator, check command,
//! project label) into the new task; `list` filters what the server lists; `done` goes through the same gate as
//! `status done`.

use fs_interfaces::tasks_capnp::items;
use fs_objects::ObjectId;
use fs_tasks_core::model::{
    CHECK_KEY, CREATED_BY_KEY, MAX_NOTE_BYTES, ORIGIN_KEY, ORIGIN_LOCAL,
};
use serde_json::{Value, json};

use crate::args::{NewTask, Query};
use crate::edit::{all_tasks, project_of, status_word, task_of, view};
use crate::fail::Failure;
use crate::policy::{OWNER, Policy};
use crate::run::{failed, one};
use crate::secrets::refuse_secret;
use crate::show::line;
use crate::work::{finish, worker};

type Out = Result<String, Failure>;

/// A project or label is one plain word.
fn plain(what: &str, text: &str) -> Result<(), String> {
    refuse_secret(what, text)?;
    let ok = !text.is_empty()
        && text.len() <= 64
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ':'));
    if ok {
        Ok(())
    } else {
        Err(format!("{what} is 1 to 64 letters, digits, `.`, `_`, `-` or `:`"))
    }
}

pub(crate) async fn add(
    items: &items::Client,
    new: NewTask,
    policy: &Result<Policy, String>,
) -> Out {
    refuse_secret("the title", &new.title)?;
    let mut keywords = serde_json::Map::new();
    if let Some(l) = &new.label {
        plain("the label", l)?;
        keywords.insert(l.clone(), Value::Bool(true));
    }
    if let Some(p) = &new.project {
        plain("the project", p)?;
        keywords.insert(format!("project:{p}"), Value::Bool(true));
    }
    let creator = match &new.creator {
        Some(name) => worker(name)?,
        None => OWNER.to_string(),
    };
    let mut task = json!({
        "@type": "Task", "version": "2.0",
        "uid": ObjectId::new().to_string(), "title": new.title,
        ORIGIN_KEY: ORIGIN_LOCAL, CREATED_BY_KEY: creator,
    });
    if let Some(due) = new.due {
        task["due"] = Value::String(due);
        task["timeZone"] = Value::String(new.zone.unwrap_or_else(|| "Etc/UTC".into()));
    }
    if !keywords.is_empty() {
        task["keywords"] = Value::Object(keywords);
    }
    if let Some(command) = new.objective {
        refuse_secret("the check command", &command)?;
        if command.len() > MAX_NOTE_BYTES {
            return Err(format!("the check command is at most {MAX_NOTE_BYTES} bytes").into());
        }
        // Refused at once if it could never run here; the policy is asked again when it runs.
        if let Ok(policy) = policy {
            let argv: Vec<String> = command.split_whitespace().map(String::from).collect();
            policy.program_ok(&argv)?;
        }
        task[CHECK_KEY] = Value::String(command);
    }
    let mut req = items.create_request();
    req.get().set_jscalendar(task.to_string().as_str());
    let answer = req.send().promise.await.map_err(|e| failed(&e))?;
    let (id, text, _) = one(answer
        .get()
        .and_then(items::create_results::Reader::get_task)
        .map_err(|e| failed(&e))?)?;
    Ok(if new.json {
        view(id, &task_of(&text)?).to_string()
    } else {
        line(&id.to_string(), &text)
    })
}

pub(crate) async fn list(items: &items::Client, query: &Query) -> Out {
    let shown: Vec<_> = all_tasks(items)
        .await?
        .into_iter()
        .filter(|(_, t, _)| {
            query.project.as_deref().is_none_or(|p| project_of(t) == Some(p))
                && query.status.as_deref().is_none_or(|s| status_word(t) == s)
        })
        .collect();
    Ok(if query.json {
        Value::Array(shown.iter().map(|(id, t, _)| view(*id, t)).collect()).to_string()
    } else {
        let lines: Vec<String> = shown
            .iter()
            .map(|(id, t, _)| line(&id.to_string(), &t.to_value().to_string()))
            .collect();
        lines.join("\n")
    })
}

pub(crate) async fn done(
    items: &items::Client,
    id: ObjectId,
    policy: &Result<Policy, String>,
) -> Out {
    let task = finish(items, id, policy).await?;
    Ok(line(&id.to_string(), &task.to_value().to_string()))
}
