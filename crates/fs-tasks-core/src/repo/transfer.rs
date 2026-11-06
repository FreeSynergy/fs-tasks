//! iCalendar (`VTODO`) import and export.
//!
//! Pattern: Template method — `import_ics` is the fixed flow (split, skip known uid, parse,
//! normalise, store, report); splitting and converting are the `ical` adapters.

use fs_store::{Blob, ObjectId};

use super::handle::TaskRepo;
use super::unread::unread_properties;
use crate::error::{Error, Result};
use crate::ical::{ICS_MEDIA_TYPE, parse_ics, split_ics, to_ics, unfolded_lines};
use crate::interface::ImportReport;
use crate::model::Task;

impl TaskRepo {
    /// Import the `VTODO`s of an `.ics` text. Each UID gets its own original (`text/calendar`,
    /// ADR 0030); known `uid`s are skipped; the tz database version is recorded (`tzdb_version`).
    pub(super) async fn import_ics_inner(&self, text: &str) -> Result<ImportReport> {
        let mut report = ImportReport::default();
        if !text.to_ascii_uppercase().contains("BEGIN:VCALENDAR") {
            report.failed.push("not an iCalendar text".into());
            return Ok(report);
        }
        let parts = split_ics(text);
        // Whole unfolded lines only: a description may quote the words, also on a continuation line.
        let found = |t: &str| {
            unfolded_lines(t)
                .iter()
                .filter(|l| l.eq_ignore_ascii_case("BEGIN:VTODO"))
                .count()
        };
        let kept: usize = parts.iter().map(|p| found(&p.text)).sum();
        for _ in kept..found(text) {
            report.failed.push("VTODO without a UID".into());
        }
        for part in parts {
            if self.stored_by_uid(&part.uid).await?.is_some() {
                report.skipped.push(part.uid);
                continue;
            }
            match parse_ics(&part.text) {
                Ok((tasks, notes)) => {
                    report.notes.extend(notes);
                    let part_text = part.text.clone();
                    let original = Blob {
                        media_type: ICS_MEDIA_TYPE.into(),
                        bytes: part.text.into_bytes(),
                    };
                    report.notes.extend(unread_properties(&part_text, &tasks));
                    for task in tasks {
                        self.import_one(task, &original, &mut report).await?;
                    }
                }
                Err(e) => report.failed.push(format!("{}: {e}", part.uid)),
            }
        }
        Ok(report)
    }

    async fn import_one(
        &self,
        mut task: Task,
        original: &Blob,
        report: &mut ImportReport,
    ) -> Result<()> {
        task.normalise();
        if task.uid.is_empty() {
            report.failed.push("component without uid".into());
            return Ok(());
        }
        if self.stored_by_uid(&task.uid).await?.is_some() {
            report.skipped.push(task.uid);
            return Ok(());
        }
        let uid = task.uid.clone();
        match self.create_inner(task, Some(original.clone())).await {
            Ok(e) => report.created.push(e.id),
            // One refused task must not lose the report of all the others.
            Err(
                e @ (Error::Invalid(_)
                | Error::Time(_)
                | Error::Conflict(_)
                | Error::TooLarge(_)
                | Error::Store(fs_store::Error::Invalid(_))),
            ) => {
                report.failed.push(format!("{uid}: {e}"));
            }
            Err(e) => return Err(e),
        }
        Ok(())
    }

    /// The tasks as iCalendar text.
    pub(super) async fn export_ics_inner(&self, ids: &[ObjectId]) -> Result<String> {
        let mut tasks = vec![];
        for id in ids {
            tasks.push(self.entry(*id).await?.ok_or(Error::NotFound(*id))?.task);
        }
        to_ics(&tasks)
    }
}
