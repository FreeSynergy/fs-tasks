//! Boards: own objects, columns from progress, places, several boards per task, limits that only report.

mod common;

use common::repo;
use fs_tasks_core::model::{Board, BoardTask, Column};
use fs_tasks_core::{Boards, Error, Task, TaskItems, TaskRepo};

async fn make(
    r: &TaskRepo,
    uid: &str,
    progress: &str,
    label: Option<&str>,
) -> fs_tasks_core::TaskEntry {
    let mut t = Task::new(uid);
    t.uid = uid.into();
    t.progress = Some(progress.into());
    if let Some(l) = label {
        t.label(l);
    }
    r.create(t).await.unwrap()
}

fn cards(layout: &fs_tasks_core::model::Layout, column: &str) -> Vec<String> {
    layout
        .columns
        .iter()
        .find(|c| c.column.id == column)
        .unwrap()
        .cards
        .clone()
}

#[test]
fn the_standard_board_puts_a_card_where_its_progress_says_and_a_place_wins() {
    let b = Board::standard("Work");
    assert_eq!(b.problem(), None);
    let tasks = |v: &[(&str, &str)]| -> Vec<BoardTask> {
        v.iter()
            .map(|(u, p)| BoardTask {
                uid: (*u).into(),
                progress: (*p).into(),
            })
            .collect()
    };
    let t = tasks(&[
        ("a", "needs-action"),
        ("b", "in-process"),
        ("c", "completed"),
        ("d", "cancelled"),
        ("e", "failed"),
        ("f", "odd"),
    ]);
    let l = b.layout(&t);
    assert_eq!(
        cards(&l, "todo"),
        ["a", "f"],
        "an unknown progress goes to the first column"
    );
    assert_eq!(cards(&l, "doing"), ["b"]);
    assert_eq!(cards(&l, "done"), ["c"]);
    assert_eq!(cards(&l, "closed"), ["d", "e"]);
    // placing a card moves it and keeps the order; the progress is not touched
    let mut b = b;
    b.move_card("c", "todo", Some(0)).unwrap();
    b.move_card("a", "todo", None).unwrap();
    let l = b.layout(&t);
    assert_eq!(cards(&l, "todo"), ["c", "a", "f"]);
    assert_eq!(cards(&l, "done").len(), 0);
    // moving it again leaves it in one place only
    b.move_card("c", "doing", Some(0)).unwrap();
    assert_eq!(cards(&b.layout(&t), "doing"), ["c", "b"]);
    assert!(b.move_card("c", "nowhere", None).is_err());
    // a column that is taken away gives its cards back to their progress
    b.remove_column("doing");
    let l = b.layout(&t);
    assert_eq!(
        cards(&l, "todo"),
        ["a", "b", "f"],
        "b falls to the first column: nothing lists in-process now"
    );
}

#[test]
fn a_board_that_is_not_valid_says_why() {
    let mut b = Board::standard("x");
    b.columns.push(Column::new("todo", "Again", &[]));
    assert!(b.problem().unwrap().contains("twice"));
    let mut b = Board::standard("x");
    b.columns[0].wip_limit = Some(0);
    assert!(b.problem().is_some());
    let mut b = Board::standard("x");
    b.order.insert("ghost".into(), vec!["a".into()]);
    assert!(b.problem().unwrap().contains("does not exist"));
    let mut b = Board::standard("x");
    b.order.insert("todo".into(), vec!["a".into()]);
    b.order.insert("done".into(), vec!["a".into()]);
    assert!(b.problem().unwrap().contains("more than once"));
    assert!(
        Board {
            columns: vec![],
            ..Board::standard("x")
        }
        .problem()
        .is_some()
    );
}

#[tokio::test]
async fn boards_are_objects_of_their_own_and_a_task_can_be_on_several() {
    let r = repo().await;
    let a = make(&r, "a", "needs-action", Some("web")).await;
    make(&r, "b", "in-process", Some("web")).await;
    make(&r, "c", "needs-action", Some("ops")).await;
    let web = r.board_create("Web", Some("web")).await.unwrap();
    let all = r.board_create("All", None).await.unwrap();
    // tasks are not boards and boards are not tasks
    assert_eq!(r.list().await.unwrap().len(), 3);
    assert_eq!(
        r.boards()
            .await
            .unwrap()
            .iter()
            .map(|b| b.board.name.clone())
            .collect::<Vec<_>>(),
        ["All", "Web"]
    );
    let lw = r.board_layout(web.id).await.unwrap();
    assert_eq!(
        (cards(&lw, "todo"), cards(&lw, "doing")),
        (vec!["a".to_string()], vec!["b".to_string()])
    );
    let la = r.board_layout(all.id).await.unwrap();
    assert_eq!(
        cards(&la, "todo").len(),
        2,
        "every task is on the board without a label"
    );
    // the same task, two boards, two places
    r.board_move_card(web.id, "a", "done", None).await.unwrap();
    assert_eq!(cards(&r.board_layout(web.id).await.unwrap(), "done"), ["a"]);
    assert_eq!(
        cards(&r.board_layout(all.id).await.unwrap(), "todo").len(),
        2
    );
    // a placed card is on the board whatever its labels say
    r.board_move_card(web.id, "c", "doing", Some(0))
        .await
        .unwrap();
    assert_eq!(
        cards(&r.board_layout(web.id).await.unwrap(), "doing"),
        ["c", "b"]
    );
    // the task itself did not change
    assert_eq!(r.get(a.id).await.unwrap().unwrap().task, a.task);
    r.board_delete(web.id).await.unwrap();
    assert!(r.board_get(web.id).await.unwrap().is_none());
    assert_eq!(r.list().await.unwrap().len(), 3, "the tasks stay");
}

#[tokio::test]
async fn a_full_column_is_reported_and_never_refuses_a_card() {
    let r = repo().await;
    for i in 0..4 {
        make(&r, &format!("t{i}"), "in-process", None).await;
    }
    let b = r.board_create("Limits", None).await.unwrap();
    let mut board = b.board.clone();
    board.columns[1].wip_limit = Some(2);
    let b = r.board_edit(b.id, b.revision(), board).await.unwrap();
    let layout = r.board_layout(b.id).await.unwrap();
    let doing = layout
        .columns
        .iter()
        .find(|c| c.column.id == "doing")
        .unwrap();
    assert_eq!((doing.cards.len(), doing.over_limit), (4, true));
    // moving one more in is not refused
    make(&r, "extra", "needs-action", None).await;
    r.board_move_card(b.id, "extra", "doing", None)
        .await
        .unwrap();
    let layout = r.board_layout(b.id).await.unwrap();
    let doing = layout
        .columns
        .iter()
        .find(|c| c.column.id == "doing")
        .unwrap();
    assert_eq!(doing.cards.len(), 5);
    assert_eq!(layout.columns.iter().filter(|c| c.over_limit).count(), 1);
}

#[tokio::test]
async fn editing_a_board_is_compare_and_set_and_refuses_an_invalid_board() {
    let r = repo().await;
    let b = r.board_create("One", None).await.unwrap();
    let mut changed = b.board.clone();
    changed.name = "Two".into();
    let edited = r
        .board_edit(b.id, b.revision(), changed.clone())
        .await
        .unwrap();
    assert_eq!(edited.board.name, "Two");
    assert_eq!(edited.envelope.name, "Two");
    assert!(matches!(
        r.board_edit(b.id, b.revision(), changed).await,
        Err(Error::Changed(_))
    ));
    let mut bad = edited.board.clone();
    bad.columns.clear();
    assert!(matches!(
        r.board_edit(b.id, edited.revision(), bad).await,
        Err(Error::Rejected(_))
    ));
    assert!(matches!(
        r.board_create("  ", None).await,
        Err(Error::Rejected(_))
    ));
    assert!(r.board_layout(fs_objects::ObjectId::new()).await.is_err());
}
