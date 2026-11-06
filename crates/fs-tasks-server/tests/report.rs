//! The report of an import always arrives: bounded in bytes, every created id kept.

mod common;

use std::sync::Arc;

use async_trait::async_trait;
use common::{ME, items_of, with_provider};
use fs_interfaces::rpc::MAX_PAGE_BYTES;
use fs_interfaces::{Capabilities, common_capnp};
use fs_store::{ObjectId, Store, TursoStore};
use fs_tasks_core::{ImportReport, Result, TaskEntry, TaskItems, TaskQuery, TaskRepo};

const MIB: usize = 1024 * 1024;

/// A provider whose import reports more than a message.
struct Talkative(ImportReport);

#[async_trait]
impl TaskItems for Talkative {
    fn capabilities(&self) -> Capabilities {
        Capabilities::new("talkative")
    }
    async fn get(&self, _: ObjectId) -> Result<Option<TaskEntry>> {
        Ok(None)
    }
    async fn get_by_uid(&self, _: &str) -> Result<Option<TaskEntry>> {
        Ok(None)
    }
    async fn list(&self) -> Result<Vec<TaskEntry>> {
        Ok(vec![])
    }
    async fn query(&self, _: &TaskQuery) -> Result<Vec<TaskEntry>> {
        Ok(vec![])
    }
    async fn import_ics(&self, _: &str) -> Result<ImportReport> {
        Ok(self.0.clone())
    }
}

fn texts(list: capnp::text_list::Reader<'_>) -> Vec<String> {
    list.iter()
        .map(|t| t.unwrap().to_string().unwrap())
        .collect()
}

#[tokio::test(flavor = "current_thread")]
async fn a_report_too_large_is_cut_and_keeps_every_created_id() {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let feed = Arc::new(TaskRepo::new(store, ME));
    let created: Vec<ObjectId> = (0..1000)
        .map(|_| fs_store::Envelope::new("Task", "x").id)
        .collect();
    let line = "x".repeat(60 * 1024);
    let provider = Talkative(ImportReport {
        created: created.clone(),
        skipped: vec!["known".into(); 3],
        failed: vec![line.clone(); 2000],
        notes: vec![format!("€{}", "y".repeat(2 * MIB))],
    });
    with_provider(Arc::new(provider), feed, |root| async move {
        let items = items_of(&root).await;
        let mut call = items.import_ics_request();
        call.get().set_text("anything");
        let answer = call
            .send()
            .promise
            .await
            .expect("the report arrives, the connection lives");
        let report = answer.get().unwrap().get_report().unwrap();

        let ids = report.get_created().unwrap();
        assert_eq!(ids.len() as usize, created.len(), "created stays whole");
        let read = |r: common_capnp::uuid::Reader<'_>| fs_interfaces::rpc::read_uuid(r).unwrap();
        assert_eq!(read(ids.get(999)), created[999].as_uuid());

        let failed = texts(report.get_failed().unwrap());
        let shown = failed.len() - 1;
        assert!(shown > 100 && shown < 2000, "{shown}");
        assert_eq!(failed[shown], format!("... {} more", 2000 - shown));
        assert!(failed[..shown].iter().all(|f| *f == line), "whole lines");
        let bytes: usize = failed.iter().map(String::len).sum();
        assert!(bytes <= MAX_PAGE_BYTES, "{bytes}");

        // What is left after the failures goes on: too little for a whole note, enough for the rest.
        assert_eq!(
            texts(report.get_skipped().unwrap()),
            ["known", "known", "known"]
        );
        assert_eq!(texts(report.get_notes().unwrap()), ["... 1 more"]);
        let list = items.list_request().send().promise.await;
        assert!(list.is_ok(), "still connected");
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_report_that_fits_is_untouched_and_one_long_text_is_cut() {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let feed = Arc::new(TaskRepo::new(store, ME));
    let provider = Talkative(ImportReport {
        skipped: vec!["a".into(), "b".into()],
        notes: vec!["€".repeat(MIB)],
        ..ImportReport::default()
    });
    with_provider(Arc::new(provider), feed, |root| async move {
        let items = items_of(&root).await;
        let mut call = items.import_ics_request();
        call.get().set_text("anything");
        let answer = call.send().promise.await.unwrap();
        let report = answer.get().unwrap().get_report().unwrap();
        assert_eq!(texts(report.get_skipped().unwrap()), ["a", "b"]);
        let notes = texts(report.get_notes().unwrap());
        assert_eq!(notes.len(), 1);
        assert!(notes[0].len() <= 64 * 1024 && notes[0].len() > 60 * 1024);
    })
    .await;
}
