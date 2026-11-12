//! iCalendar (`VTODO`) import and export.
//!
//! Pattern: Template method — `import_ics` is the fixed flow (split, skip known uid, parse,
//! normalise, store, report; the known `uid`s are read once, see `known.rs`); splitting and converting are the `ical` adapters.

use fs_objects::{Blob, ObjectId};

use super::handle::TaskRepo;
use super::known::KnownUids;
use super::unread::unread_properties;
use crate::error::{Error, Result};
use crate::ical::{
    ICS_MEDIA_TYPE, parse_ics, put_comments, split_ics, take_comments, to_ics, unfolded_lines,
};
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
        // Asked once: a lookup by `uid` for each part would look at every stored task each time.
        let mut known = if parts.is_empty() {
            KnownUids::default()
        } else {
            self.known_uids().await?
        };
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
            if known.contains(&part.uid) {
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
                        self.import_one(task, &original, &mut known, &mut report)
                            .await?;
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
        known: &mut KnownUids,
        report: &mut ImportReport,
    ) -> Result<()> {
        task.normalise();
        if task.uid.is_empty() {
            report.failed.push("component without uid".into());
            return Ok(());
        }
        if known.contains(&task.uid) {
            report.skipped.push(task.uid);
            return Ok(());
        }
        let uid = task.uid.clone();
        // `COMMENT` lines become comments (own objects); the task does not keep them
        let comments = take_comments(&mut task);
        match self.store_new(task, Some(original.clone())).await {
            Ok(e) => {
                self.queue_created(&e.task);
                known.add(uid.clone());
                report.created.push(e.id);
                self.import_comments(&e, &comments, &uid, report).await;
            }
            // One refused task must not lose the report of all the others.
            Err(
                e @ (Error::Invalid(_)
                | Error::Time(_)
                | Error::Conflict(_)
                | Error::Exists(_)
                | Error::TooLarge(_)
                | Error::Store(fs_objects::Error::Invalid(_))),
            ) => {
                report.failed.push(format!("{uid}: {e}"));
            }
            Err(e) => return Err(e),
        }
        Ok(())
    }

    /// The comments of an imported task: the author is the importing actor, because iCalendar names none.
    async fn import_comments(
        &self,
        task: &crate::interface::TaskEntry,
        texts: &[String],
        uid: &str,
        report: &mut ImportReport,
    ) {
        let Some(context) = task.envelope.iri.clone() else {
            return;
        };
        let mut made = 0;
        for text in texts.iter().filter(|t| !t.trim().is_empty()) {
            match self
                .comment_add_unlocked(&context, text, None, &self.actor.clone())
                .await
            {
                Ok(_) => made += 1,
                Err(e) => report
                    .notes
                    .push(format!("{uid}: a COMMENT was not kept: {e}")),
            }
        }
        if made > 0 {
            report.notes.push(format!(
                "{uid}: {made} COMMENT line(s) became comments; their author is the importing actor (iCalendar names none)"
            ));
        }
    }

    /// The tasks as iCalendar text; the comments of a task (not the deleted ones) go out as its `COMMENT` lines, oldest
    /// first, a reply after the comment it answers (iCalendar has no thread and no author for them).
    pub(super) async fn export_ics_inner(&self, ids: &[ObjectId]) -> Result<String> {
        let mut tasks = vec![];
        for id in ids {
            let entry = self.entry(*id).await?.ok_or(Error::NotFound(*id))?;
            let mut task = entry.task;
            if let Some(iri) = &entry.envelope.iri {
                let texts: Vec<String> = self
                    .comments_inner(iri)
                    .await?
                    .into_iter()
                    .filter(|c| !c.tombstone)
                    .map(|c| c.body)
                    .collect();
                put_comments(&mut task, &texts);
            }
            tasks.push(task);
        }
        to_ics(&tasks)
    }
}
