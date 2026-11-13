//! The wiring of the window to the server's change feed: what the Board shows follows another client, and the window
//! does not keep itself alive through the feed.
//!
//! Pattern: the real window (`Frontends`) over `RemoteItems` over a served repository; "another client" writes to the
//! repository. The counter test is the same window without the wiring (`live: None`).

use std::sync::Arc;
use std::time::{Duration, Instant};

use fs_objects::{Store, TursoStore};
use fs_tasks_app::{Day, Frontend, FrontendOptions, Frontends};
use fs_tasks_core::{Task, TaskItems, TaskRepo};
use fs_tasks_server::{RemoteItems, host};

fn options(live: Option<Arc<fs_tasks_server::Changes>>) -> FrontendOptions {
    FrontendOptions {
        today: Day::parse("2026-11-12").unwrap(),
        wait: true,
        list: None,
        open: None,
        view: None,
        remember: None,
        live,
    }
}

fn shows(frontends: &Frontends, title: &str) -> bool {
    frontends
        .cache()
        .and_then(|c| c.current())
        .is_some_and(|s| {
            s.entries
                .iter()
                .any(|e| e.task.title.as_deref() == Some(title))
        })
}

fn wait_for(limit: Duration, mut done: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    done()
}

struct Served {
    rt: tokio::runtime::Runtime,
    repo: Arc<TaskRepo>,
    socket: std::path::PathBuf,
    _dir: tempfile::TempDir,
    hosted: Option<fs_tasks_server::Hosted>,
}

fn served() -> Served {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let repo = rt.block_on(async {
        let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
        Arc::new(TaskRepo::new(store, "https://localhost/actors/owner"))
    });
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("run").join("tasks.sock");
    let hosted = host(repo.clone(), socket.clone()).unwrap();
    Served {
        rt,
        repo,
        socket,
        _dir: dir,
        hosted: Some(hosted),
    }
}

fn window(s: &Served, wired: bool) -> Frontends {
    let remote = RemoteItems::connect(s.socket.clone(), None).unwrap();
    let live = wired.then(|| remote.changes());
    let items: Arc<dyn TaskItems> = Arc::new(remote);
    Frontends::new(items, s.rt.handle().clone(), Frontend::Board, options(live)).unwrap()
}

#[test]
fn the_board_shows_what_another_client_creates() {
    let s = served();
    let (wired, deaf) = (window(&s, true), window(&s, false));
    assert!(!shows(&wired, "from the command line"));
    s.rt.block_on(s.repo.create(Task::new("from the command line")))
        .unwrap();
    assert!(
        wait_for(Duration::from_secs(5), || shows(
            &wired,
            "from the command line"
        )),
        "the window did not follow within 5 s"
    );
    // Counter test: without the wiring the same window stays as it was.
    std::thread::sleep(Duration::from_millis(300));
    assert!(!shows(&deaf, "from the command line"));
}

#[test]
fn when_the_server_dies_the_open_window_says_so_and_neither_crashes_nor_hangs() {
    let mut s = served();
    let window = window(&s, true);
    let cache = window.cache().unwrap();
    assert!(cache.problem().is_none());
    drop(s.hosted.take()); // the server ends while the window is open
    let start = Instant::now();
    cache.reload_in_background();
    assert!(
        wait_for(Duration::from_secs(15), || cache.problem().is_some()),
        "no message after the server ended"
    );
    assert!(start.elapsed() < Duration::from_secs(15));
    let said = cache.problem().unwrap();
    assert!(!said.is_empty(), "{said:?}");
    // The window still answers: the last picture stays.
    assert!(cache.current().is_some());
}
