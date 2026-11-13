//! Carrying out one command against a `tasks::Client`.
//!
//! Pattern: Command (executor) — one function per command, each a few contract calls. Errors come
//! back as the text of the problem the server sent; the contract's problem types are not reinterpreted.

use fs_interfaces::common_capnp;
use fs_interfaces::rpc::{read_envelope, write_uuid};
use fs_interfaces::tasks_capnp::{items, task, tasks};
use fs_objects::ObjectId;

use crate::args::Command;
use crate::fail::Failure;
use crate::make::{add, done, list};
use crate::policy::{Policy, data_dir};
use crate::show::pretty;
use crate::work::{id_of, run_work};

type Out = Result<String, Failure>;

pub(crate) fn failed(e: &capnp::Error) -> String {
    e.to_string()
}

/// Run `command` with the check settings of this user; the text to print.
///
/// # Errors
/// The server refused (its problem text), or an id or a file could not be read; a conflict says so.
pub async fn run(root: &tasks::Client, command: Command) -> Out {
    run_with(root, command, &Policy::load_from(data_dir().as_deref())).await
}

/// Like [`run`], with the check settings given (or the reason they cannot be read: then no check runs).
///
/// # Errors
/// As [`run`].
pub async fn run_with(
    root: &tasks::Client,
    command: Command,
    policy: &Result<Policy, String>,
) -> Out {
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
        Command::List(query) => list(&items, &query).await,
        Command::Show(id) => show(&items, id_of(&id)?).await,
        Command::Add(new) => add(&items, new, policy).await,
        Command::Done(id) => done(&items, id_of(&id)?, policy).await,
        Command::Delete(id) => delete(&items, id_of(&id)?).await,
        Command::Export => export(&items).await,
        Command::Import(path) => import(&items, &path).await,
        Command::Work { work, json } => run_work(&items, work, json, policy).await,
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

pub(crate) fn one(t: task::Reader<'_>) -> Result<(ObjectId, String, u64), String> {
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
pub(crate) async fn all(items: &items::Client) -> Result<Vec<(ObjectId, String, u64)>, String> {
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
        if next == cursor.as_slice() {
            return Err("the server answered the same cursor twice; stopped".into());
        }
        cursor = next.to_vec();
    }
}

pub(crate) async fn fetch(items: &items::Client, id: ObjectId) -> Result<(String, u64), String> {
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
    Ok(answer
        .get()
        .and_then(items::export_ics_results::Reader::get_text)
        .map_err(|e| failed(&e))?
        .to_string()
        .map_err(|e| e.to_string())?)
}

async fn import(items: &items::Client, path: &std::path::Path) -> Out {
    let text = read_limited(path)?;
    let mut req = items.import_ics_request();
    req.get().set_text(text.as_str());
    let answer = req.send().promise.await.map_err(|e| failed(&e))?;
    let report = answer
        .get()
        .and_then(fs_interfaces::tasks_capnp::items::import_ics_results::Reader::get_report)
        .map_err(|e| failed(&e))?;
    let created = report.get_created().map_err(|e| failed(&e))?.len();
    let skipped = report.get_skipped().map_err(|e| failed(&e))?.len();
    let failures: Vec<String> = report
        .get_failed()
        .map_err(|e| failed(&e))?
        .iter()
        .map(|t| {
            t.map_err(|e| failed(&e))?
                .to_string()
                .map_err(|e| e.to_string())
        })
        .collect::<Result<_, _>>()?;
    let summary = format!(
        "created {created}, skipped {skipped}, failed {}",
        failures.len()
    );
    if failures.is_empty() {
        Ok(summary)
    } else {
        // Something was refused: the reasons go to stderr with a non-zero exit, so a script sees it.
        Err(format!("{summary}\n{}", failures.join("\n")).into())
    }
}

/// The most an import file may hold. Larger files are refused before they are read.
const MAX_IMPORT_BYTES: u64 = 16 * 1024 * 1024;

fn read_limited(path: &std::path::Path) -> Result<String, String> {
    use std::io::Read as _;
    let named = |e: std::io::Error| format!("{}: {e}", path.display());
    let file = std::fs::File::open(path).map_err(named)?;
    let mut bytes = vec![];
    file.take(MAX_IMPORT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(named)?;
    if bytes.len() as u64 > MAX_IMPORT_BYTES {
        return Err(format!("{}: larger than 16 MiB", path.display()));
    }
    String::from_utf8(bytes).map_err(|_| format!("{}: not UTF-8 text", path.display()))
}
