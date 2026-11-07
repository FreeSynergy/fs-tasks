//! iCalendar files in and out, by a path from the command line.
//!
//! Pattern: Template method over the contract — fixed flow: read, import or export, report. Nothing is
//! hidden: what failed or was left out is in the report.

use std::fmt::Write as _;
use std::path::Path;

use fs_components_app::{write_new_file, write_new_private_file};
use fs_objects::ObjectId;
use fs_tasks_core::{TaskEntry, TaskItems};
use tokio::runtime::Handle;

use crate::source::{is_marked, reason_text};

/// What an import did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportSummary {
    /// Tasks that were added.
    pub created: usize,
    /// Tasks that were known already and were left as they are.
    pub skipped: usize,
    /// What could not be imported, with the reason.
    pub failed: Vec<String>,
    /// What the core has to say about the import.
    pub notes: Vec<String>,
    /// The import stopped here (what came before is in).
    pub stopped: Option<String>,
}

impl ImportSummary {
    /// Did all of it work?
    pub fn is_clean(&self) -> bool {
        self.failed.is_empty() && self.stopped.is_none()
    }

    /// In words, for the status line.
    pub fn message(&self) -> String {
        let mut text = format!("Imported {}, {} already known", self.created, self.skipped);
        if !self.failed.is_empty() {
            let _ = write!(
                text,
                ", {} failed: {}",
                self.failed.len(),
                self.failed.join("; ")
            );
        }
        if !self.notes.is_empty() {
            let _ = write!(text, ". Notes: {}", self.notes.join("; "));
        }
        if let Some(why) = &self.stopped {
            let _ = write!(text, ". The import stopped: {why}");
        }
        text
    }
}

/// Import the iCalendar file at `path`.
///
/// # Errors
/// A message for the person if the file cannot be read or holds no calendar.
pub fn import_file(
    items: &dyn TaskItems,
    runtime: &Handle,
    path: &Path,
) -> Result<ImportSummary, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    if !text.to_ascii_uppercase().contains("BEGIN:VCALENDAR") {
        return Err(format!("{} holds no calendar", path.display()));
    }
    let mut summary = ImportSummary::default();
    match runtime.block_on(items.import_ics(&text)) {
        Ok(report) => {
            summary.created = report.created.len();
            summary.skipped = report.skipped.len();
            summary.failed = report.failed;
            summary.notes = report.notes;
        }
        Err(e) => summary.stopped = Some(reason_text(&e)),
    }
    Ok(summary)
}

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
        let mut text = format!("Wrote {} to {}", self.written, path.display());
        if !self.released.is_empty() {
            let _ = write!(text, ". Released by you: {}", self.released.join(", "));
        }
        if !self.left_out.is_empty() {
            let _ = write!(
                text,
                ". Left out as sensitive (name one with --export-sensitive <uid>): {}",
                self.left_out.join(", ")
            );
        }
        if !self.failed.is_empty() {
            let _ = write!(text, ". Failed: {}", self.failed.join("; "));
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
        return Err(format!(
            "--export-sensitive {nothing} names no task; nothing was written"
        ));
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
            Err(e) => report.failed.push(format!("{}: {e}", entry.task.uid)),
        }
    }
    report.written = good.len();
    runtime
        .block_on(items.export_ics(&good))
        .map_err(|e| reason_text(&e))
}
