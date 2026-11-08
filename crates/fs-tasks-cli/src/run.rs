//! Carrying out one command against a `tasks::Client`.
//!
//! Pattern: Command (executor) — one function per command, each a few contract calls. Errors come
//! back as the text of the problem the server sent; the contract's problem types are not reinterpreted.

use fs_interfaces::common_capnp;
use fs_interfaces::rpc::{read_envelope, write_uuid};
use fs_interfaces::tasks_capnp::{items, task, tasks};
use fs_objects::ObjectId;
use serde_json::{Value, json};

use crate::args::Command;
use crate::show::{line, pretty};

type Out = Result<String, String>;

fn failed(e: &capnp::Error) -> String {
    e.to_string()
}

fn id_of(text: &str) -> Result<ObjectId, String> {
    text.trim()
        .parse()
        .map_err(|_| format!("{text:?} is not a task id"))
}

/// Run `command`; the text to print.
///
/// # Errors
/// The server refused (its problem text), or an id or a file could not be read.
pub async fn run(root: &tasks::Client, command: Command) -> Out {
    let items = root
        .items_request()
        .send()
        .promise
        .await
        .map_err(|e| failed(&e))?
        .get()
        .and_then(fs_interfaces::tasks_capnp::tasks::items_results::Reader::get_items)
        .map_err(|e| failed(&e))?;
    match command {
        Command::Capabilities => capabilities(root).await,
        Command::List => list(&items).await,
        Command::Show(id) => show(&items, id_of(&id)?).await,
        Command::Add {
            title,
            due,
            zone,
            label,
        } => add(&items, &title, due, zone, label).await,
        Command::Done(id) => done(&items, id_of(&id)?).await,
        Command::Delete(id) => delete(&items, id_of(&id)?).await,
        Command::Export => export(&items).await,
        Command::Import(path) => import(&items, &path).await,
    }
}

async fn capabilities(root: &tasks::Client) -> Out {
    let answer = root
        .capabilities_request()
        .send()
        .promise
        .await
        .map_err(|e| failed(&e))?;
    let list = answer
        .get()
        .and_then(fs_interfaces::tasks_capnp::tasks::capabilities_results::Reader::get_capabilities)
        .map_err(|e| failed(&e))?;
    let ids = list
        .get_ids()
        .map_err(|e| failed(&e))?
        .iter()
        .map(|t| {
            t.map_err(|e| failed(&e))?
                .to_string()
                .map_err(|e| e.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ids.join("\n"))
}

fn one(t: task::Reader<'_>) -> Result<(ObjectId, String, u64), String> {
    let id = read_envelope(t.get_envelope().map_err(|e| failed(&e))?)
        .map_err(|e| failed(&e))?
        .id;
    let text = t
        .get_jscalendar()
        .map_err(|e| failed(&e))?
        .to_string()
        .map_err(|e| e.to_string())?;
    Ok((id, text, t.get_revision()))
}

/// Every task, page by page: id, JSCalendar text, revision.
async fn all(items: &items::Client) -> Result<Vec<(ObjectId, String, u64)>, String> {
    let mut found = vec![];
    let mut cursor: Vec<u8> = vec![];
    loop {
        let mut req = items.list_request();
        req.get().init_cursor().set_opaque(&cursor);
        let answer = req.send().promise.await.map_err(|e| failed(&e))?;
        let page = answer
            .get()
            .and_then(items::list_results::Reader::get_page)
            .map_err(|e| failed(&e))?;
        for t in page.get_items().map_err(|e| failed(&e))? {
            found.push(one(t)?);
        }
        let next = page
            .get_next()
            .and_then(common_capnp::cursor::Reader::get_opaque)
            .map_err(|e| failed(&e))?;
        if next.is_empty() {
            return Ok(found);
        }
        cursor = next.to_vec();
    }
}

async fn list(items: &items::Client) -> Out {
    let lines: Vec<String> = all(items)
        .await?
        .iter()
        .map(|(id, text, _)| line(&id.to_string(), text))
        .collect();
    Ok(lines.join("\n"))
}

async fn fetch(items: &items::Client, id: ObjectId) -> Result<(String, u64), String> {
    let mut req = items.get_request();
    write_uuid(req.get().init_id(), Some(id.as_uuid()));
    let answer = req.send().promise.await.map_err(|e| failed(&e))?;
    let r = answer.get().map_err(|e| failed(&e))?;
    if !r.get_found() {
        return Err(format!("no task {id}"));
    }
    let (_, text, revision) = one(r.get_task().map_err(|e| failed(&e))?)?;
    Ok((text, revision))
}

async fn show(items: &items::Client, id: ObjectId) -> Out {
    Ok(pretty(&fetch(items, id).await?.0))
}

async fn add(
    items: &items::Client,
    title: &str,
    due: Option<String>,
    zone: Option<String>,
    label: Option<String>,
) -> Out {
    let mut task = json!({
        "@type": "Task", "version": "2.0",
        "uid": ObjectId::new().to_string(), "title": title,
    });
    if let Some(due) = due {
        task["due"] = Value::String(due);
        task["timeZone"] = Value::String(zone.unwrap_or_else(|| "Etc/UTC".into()));
    }
    if let Some(label) = label {
        task["keywords"] = json!({ label: true });
    }
    let mut req = items.create_request();
    req.get().set_jscalendar(task.to_string().as_str());
    let answer = req.send().promise.await.map_err(|e| failed(&e))?;
    let (id, text, _) = one(answer
        .get()
        .and_then(fs_interfaces::tasks_capnp::items::create_results::Reader::get_task)
        .map_err(|e| failed(&e))?)?;
    Ok(line(&id.to_string(), &text))
}

async fn done(items: &items::Client, id: ObjectId) -> Out {
    let (text, revision) = fetch(items, id).await?;
    let mut v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    v["progress"] = Value::String("completed".into());
    let mut req = items.edit_if_request();
    write_uuid(req.get().init_id(), Some(id.as_uuid()));
    req.get().set_revision(revision);
    req.get().set_jscalendar(v.to_string().as_str());
    let answer = req.send().promise.await.map_err(|e| failed(&e))?;
    let (id, text, _) = one(answer
        .get()
        .and_then(fs_interfaces::tasks_capnp::items::edit_if_results::Reader::get_task)
        .map_err(|e| failed(&e))?)?;
    Ok(line(&id.to_string(), &text))
}

async fn delete(items: &items::Client, id: ObjectId) -> Out {
    let mut req = items.delete_request();
    write_uuid(req.get().init_id(), Some(id.as_uuid()));
    req.send().promise.await.map_err(|e| failed(&e))?;
    Ok(format!("deleted {id}"))
}

async fn export(items: &items::Client) -> Out {
    let ids: Vec<ObjectId> = all(items).await?.into_iter().map(|t| t.0).collect();
    let mut req = items.export_ics_request();
    let mut list = req
        .get()
        .init_ids(u32::try_from(ids.len()).map_err(|_| "too many tasks".to_string())?);
    for (i, id) in (0u32..).zip(&ids) {
        write_uuid(list.reborrow().get(i), Some(id.as_uuid()));
    }
    let answer = req.send().promise.await.map_err(|e| failed(&e))?;
    answer
        .get()
        .and_then(items::export_ics_results::Reader::get_text)
        .map_err(|e| failed(&e))?
        .to_string()
        .map_err(|e| e.to_string())
}

async fn import(items: &items::Client, path: &std::path::Path) -> Out {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut req = items.import_ics_request();
    req.get().set_text(text.as_str());
    let answer = req.send().promise.await.map_err(|e| failed(&e))?;
    let report = answer
        .get()
        .and_then(fs_interfaces::tasks_capnp::items::import_ics_results::Reader::get_report)
        .map_err(|e| failed(&e))?;
    let created = report.get_created().map_err(|e| failed(&e))?.len();
    let skipped = report.get_skipped().map_err(|e| failed(&e))?.len();
    let failed_n = report.get_failed().map_err(|e| failed(&e))?.len();
    Ok(format!(
        "created {created}, skipped {skipped}, failed {failed_n}"
    ))
}
