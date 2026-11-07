//! Tasks written by 0.6.0 have no `order_key`, `id_key` or `description` in their index. The
//! fixture `fixtures/v060.db` (with its `-wal`) was written by the code of tag v0.6.0, six tasks
//! through `TaskRepo::create`, not derived from the current code. Opening it must lose nothing, and what
//! cannot be repaired is said, never skipped.

use std::path::Path;
use std::sync::Arc;

use fs_store::{Envelope, Store, StoredObject, TursoStore};
use fs_tasks_core::{Error, TaskItems, TaskQuery, TaskRepo};

const ME: &str = "https://local.invalid/users/owner";

fn copy_fixture(to: &Path) -> std::path::PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for name in ["v060.db", "v060.db-wal"] {
        std::fs::copy(from.join(name), to.join(name)).unwrap();
    }
    to.join("v060.db")
}

async fn open(path: &Path) -> (TaskRepo, Arc<dyn Store>) {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open(path).await.unwrap());
    (TaskRepo::new(store.clone(), ME), store)
}

async fn paged(repo: &TaskRepo, query: &TaskQuery, size: usize) -> Vec<String> {
    let (mut uids, mut after) = (Vec::new(), None);
    loop {
        let page = repo.page(query, after.as_ref(), size).await.unwrap();
        uids.extend(page.items.iter().map(|i| i.entry.task.uid.clone()));
        after = page.items.last().map(|i| i.place.clone());
        if !page.more {
            return uids;
        }
    }
}

#[tokio::test]
async fn a_store_of_0_6_0_is_not_paged_silently_short_and_is_repaired_on_upgrade() {
    let dir = tempfile::tempdir().unwrap();
    let (repo, _store) = open(&copy_fixture(dir.path())).await;
    assert_eq!(repo.list().await.unwrap().len(), 6);

    // Before the upgrade a page says what is wrong; it does not give five of six.
    let err = repo.page(&TaskQuery::new(), None, 2).await.unwrap_err();
    assert!(matches!(err, Error::Stored(_)), "{err}");
    assert!(err.to_string().contains("upgrade_index"), "{err}");

    let report = repo.upgrade_index().await.unwrap();
    assert_eq!(
        (report.rewritten, report.failed.len()),
        (6, 0),
        "{report:?}"
    );
    assert_eq!(
        repo.upgrade_index().await.unwrap().rewritten,
        0,
        "idempotent"
    );

    // `list` now orders by due date as it should (before, the keyless tasks came in name order).
    let listed: Vec<String> = repo
        .list()
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.task.uid)
        .collect();
    assert_eq!(
        listed,
        ["old-6", "old-1", "old-2", "old-4", "old-5", "old-3"].map(String::from)
    );
    for size in [1, 2, 4, 6, 10] {
        assert_eq!(
            paged(&repo, &TaskQuery::new(), size).await,
            listed,
            "size {size}"
        );
    }
    let by_term = TaskQuery::new().matching("REPORT");
    assert_eq!(
        paged(&repo, &by_term, 2).await,
        ["old-1"],
        "the description is indexed"
    );
    let open = TaskQuery::new().only_open();
    let queried: Vec<String> = repo
        .query(&open)
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.task.uid)
        .collect();
    assert_eq!(paged(&repo, &open, 2).await, queried);
}

#[tokio::test]
async fn a_task_that_cannot_be_repaired_is_named_and_the_others_are_repaired() {
    let dir = tempfile::tempdir().unwrap();
    let (repo, store) = open(&copy_fixture(dir.path())).await;
    let mut env = Envelope::new("Task", "Broken");
    env.context = Some("tasks".into());
    let broken = StoredObject::new(
        env,
        "application/jscalendar+json",
        serde_json::json!({"@type": "Task", "percentComplete": 300}),
    );
    let broken_id = broken.envelope.id;
    store.put(broken, ME).await.unwrap();

    let report = repo.upgrade_index().await.unwrap();
    assert_eq!(report.rewritten, 6);
    assert_eq!(report.failed.len(), 1, "{report:?}");
    assert!(
        report.failed[0].contains(&broken_id.to_string()),
        "{report:?}"
    );
    // The page says that one task has no place, instead of leaving it out unnoticed.
    let err = repo.page(&TaskQuery::new(), None, 10).await.unwrap_err();
    assert!(
        matches!(err, Error::Stored(_)) && err.to_string().contains('1'),
        "{err}"
    );
}
