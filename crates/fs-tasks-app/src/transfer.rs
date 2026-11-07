//! iCalendar out: the export by a path from the command line.
//!
//! Pattern: Template method over the contract — fixed flow: read, choose, export, report. Nothing is
//! hidden: what failed or was left out is in the report.

use std::path::Path;

use fs_components_app::{write_new_file, write_new_private_file};
use fs_store::ObjectId;
use fs_tasks_core::{TaskEntry, TaskItems};
use tokio::runtime::Handle;

use crate::source::{is_marked, reason_text};
use crate::words::{fill, sentences, transfer as said};

/// What an export did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExportReport {
    /// Tasks written.
    pub written: usize,
    /// Tasks marked sensitive and left out, by their `uid` alone: the title may be what is sensitive,
    /// and this goes to the terminal and its logs (ADR 0029).
    pub left_out: Vec<String>,
    /// Tasks marked sensitive and written, because their `uid` was named; by `uid` alone.
    pub released: Vec<String>,
    /// Tasks that could not be expressed as iCalendar, by `uid`, with the reason.
    pub failed: Vec<String>,
}

impl ExportReport {
    /// In words.
    pub fn message(&self, path: &Path) -> String {
        let mut text = fill(
            said::WROTE,
            &[
                ("written", &self.written.to_string()),
                ("file", &path.display().to_string()),
            ],
        );
        for (sentence, list) in [
            (said::RELEASED, self.released.join(", ")),
            (said::LEFT_OUT, self.left_out.join(", ")),
            (said::FAILED, self.failed.join("; ")),
        ] {
            if !list.is_empty() {
                text = sentences(&text, &fill(sentence, &[("list", &list)]));
            }
        }
        text
    }
}

/// Write all tasks to a new file at `path`. A file that exists is not replaced, and a failed write
/// leaves nothing behind. Tasks marked sensitive are left out unless `released` names their `uid` (ADR
/// 0029: they leave only by explicit release, one by one); a file with such a task in it is for its
/// owner alone (mode 0600 on Unix).
///
/// # Errors
/// A message for the person if the list cannot be read, the file cannot be written, or `released` names
/// a task that does not exist: then nothing is written.
pub fn export_all(
    items: &dyn TaskItems,
    runtime: &Handle,
    path: &Path,
    released: &[String],
) -> Result<ExportReport, String> {
    let all = runtime
        .block_on(items.list())
        .map_err(|e| reason_text(&e))?;
    let mut report = ExportReport::default();
    let mut unmatched: Vec<&String> = released.iter().collect();
    let mut chosen: Vec<&TaskEntry> = Vec::new();
    for entry in &all {
        let id = entry.id.to_string();
        let named = |given: &&String| given.as_str() == entry.task.uid || **given == id;
        let is_released = released.iter().any(|given| named(&given));
        unmatched.retain(|given| !named(given));
        if is_marked(entry) {
            if !is_released {
                report.left_out.push(entry.task.uid.clone());
                continue;
            }
            report.released.push(entry.task.uid.clone());
        }
        chosen.push(entry);
    }
    if let Some(nothing) = unmatched.first() {
        return Err(fill(said::NAMES_NO_TASK, &[("name", nothing)]));
    }
    let text = text_of(items, runtime, &chosen, &mut report)?;
    if report.released.is_empty() {
        write_new_file(path, text.as_bytes())?;
    } else {
        write_new_private_file(path, text.as_bytes())?;
    }
    Ok(report)
}

/// The text of all of `chosen`. If that fails, each is tried alone, so one that cannot be expressed
/// costs only itself and is named.
fn text_of(
    items: &dyn TaskItems,
    runtime: &Handle,
    chosen: &[&TaskEntry],
    report: &mut ExportReport,
) -> Result<String, String> {
    let ids: Vec<ObjectId> = chosen.iter().map(|e| e.id).collect();
    if let Ok(text) = runtime.block_on(items.export_ics(&ids)) {
        report.written = ids.len();
        return Ok(text);
    }
    let mut good = Vec::new();
    for entry in chosen {
        match runtime.block_on(items.export_ics(&[entry.id])) {
            Ok(_) => good.push(entry.id),
            Err(e) => report.failed.push(fill(
                said::FAILED_ONE,
                &[("uid", &entry.task.uid), ("error", &e.to_string())],
            )),
        }
    }
    report.written = good.len();
    runtime
        .block_on(items.export_ics(&good))
        .map_err(|e| reason_text(&e))
}
