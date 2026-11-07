//! iCalendar in: the import by a path from the command line.
//!
//! Pattern: Facade — one function hides the file, the runtime and the contract's `import_ics` behind a
//! single call that answers a summary. Nothing is lost silently: what failed or was skipped is in the summary.

use std::path::Path;

use fs_tasks_core::TaskItems;
use tokio::runtime::Handle;

use crate::source::reason_text;
use crate::words::{fill, sentences, transfer as said};

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
        let (created, skipped) = (self.created.to_string(), self.skipped.to_string());
        let mut text = if self.failed.is_empty() {
            fill(
                said::IMPORTED,
                &[("created", &created), ("skipped", &skipped)],
            )
        } else {
            fill(
                said::IMPORTED_FAILED,
                &[
                    ("created", &created),
                    ("skipped", &skipped),
                    ("count", &self.failed.len().to_string()),
                    ("list", &self.failed.join("; ")),
                ],
            )
        };
        if !self.notes.is_empty() {
            text = sentences(
                &text,
                &fill(said::NOTES, &[("notes", &self.notes.join("; "))]),
            );
        }
        if let Some(why) = &self.stopped {
            text = sentences(&text, &fill(said::STOPPED, &[("why", why)]));
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
    let text = std::fs::read_to_string(path).map_err(|e| {
        fill(
            said::CANNOT_READ,
            &[
                ("file", &path.display().to_string()),
                ("error", &e.to_string()),
            ],
        )
    })?;
    if !text.to_ascii_uppercase().contains("BEGIN:VCALENDAR") {
        return Err(fill(
            said::NO_CALENDAR,
            &[("file", &path.display().to_string())],
        ));
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
