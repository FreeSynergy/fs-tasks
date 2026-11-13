//! The frontends of the window: the menu names them, a choice changes the frontend and is remembered, a broken memory means
//! the default, and `--plan` asks for a frontend without changing the memory.
//!
//! Pattern: Command tests through the shell's own way of choosing an entry, over the real core; the memory is a real file.

mod common;

use std::sync::Arc;

use common::{Scratch, World};
use fs_tasks_app::args::{Plan, parse};
use fs_tasks_app::{Day, Frontend, FrontendOptions, Frontends, Preference};
use fs_tasks_core::TaskItems;

fn options(remember: Option<Preference>) -> FrontendOptions {
    FrontendOptions {
        today: Day::parse("2026-11-12").unwrap(),
        wait: false,
        list: None,
        open: None,
        view: None,
        remember,
        live: None,
    }
}

fn window(w: &World, start: Frontend, remember: Option<Preference>) -> Frontends {
    let items: Arc<dyn TaskItems> = w.repo.clone();
    Frontends::new(items, w.runtime.handle().clone(), start, options(remember)).unwrap()
}

fn labels_of(frontends: &Frontends) -> Vec<String> {
    frontends.app().with_shell(|shell| {
        shell
            .menus()
            .iter()
            .flat_map(|menu| {
                let lines = menu.entries.iter().map(|e| e.label.clone());
                std::iter::once(menu.label.clone()).chain(lines)
            })
            .collect()
    })
}

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| (*w).to_string()).collect()
}

#[test]
fn every_frontend_is_saved_and_loaded() {
    let dir = Scratch::new();
    let memory = Preference::new(dir.file("settings.json"));
    for frontend in Frontend::ALL {
        memory.save(frontend).unwrap();
        assert_eq!(memory.load(), frontend);
    }
    assert_eq!(dir.names(), ["settings.json"], "no temporary file is left");
}

#[test]
fn no_file_means_the_board() {
    let dir = Scratch::new();
    let memory = Preference::new(dir.file("settings.json"));
    assert_eq!(memory.load(), Frontend::Board);
    assert_eq!(Frontend::DEFAULT, Frontend::Board);
}

#[test]
fn a_broken_file_means_the_default_and_the_next_choice_replaces_it() {
    let dir = Scratch::new();
    let file = dir.file("settings.json");
    let memory = Preference::new(file.clone());
    for broken in [
        "",
        "not json",
        "[1, 2]",
        "{\"frontend\": 7}",
        "{\"frontend\": \"cube\"}",
        "{\"face\": 7}",
        "{\"face\": \"cube\"}",
        "{}",
    ] {
        std::fs::write(&file, broken).unwrap();
        assert_eq!(memory.load(), Frontend::Board, "{broken:?}");
    }
    memory.save(Frontend::Day).unwrap();
    assert_eq!(memory.load(), Frontend::Day);
}

#[test]
fn saving_makes_the_folder_and_a_failure_is_an_error_not_a_crash() {
    let dir = Scratch::new();
    let deep = Preference::new(dir.file("a").join("b").join("settings.json"));
    deep.save(Frontend::Classic).unwrap();
    assert_eq!(deep.load(), Frontend::Classic);
    std::fs::write(dir.file("plain"), "x").unwrap();
    let blocked = Preference::new(dir.file("plain").join("settings.json"));
    assert!(blocked.save(Frontend::Day).is_err());
    assert_eq!(blocked.load(), Frontend::Board);
}

#[test]
fn the_menu_names_the_frontends_and_the_shown_one_cannot_be_chosen() {
    let w = World::new();
    let frontends = window(&w, Frontend::Board, None);
    assert_eq!(labels_of(&frontends), ["Frontend", "Classic", "Board", "Day list"]);
    assert!(!frontends.app().choose("frontend-board"), "it is shown already");
    assert!(!frontends.app().choose("frontend-cube"), "no such frontend");
    assert!(
        !frontends.app().choose("task-new"),
        "the classic commands belong to the classic frontend"
    );
    assert_eq!(frontends.frontend(), Frontend::Board);
}

#[test]
fn the_classic_frontend_keeps_its_own_menus_beside_the_frontend_menu() {
    let w = World::new();
    let frontends = window(&w, Frontend::Classic, None);
    assert_eq!(
        labels_of(&frontends),
        [
            "Task", "New", "View", "List", "Status", "Frontend", "Classic", "Board", "Day list"
        ]
    );
    assert!(frontends.app().choose("view-status"));
    assert!(frontends.app().choose("task-new"));
}

#[test]
fn choosing_a_frontend_changes_it_and_the_menus_and_is_remembered() {
    let w = World::new();
    let dir = Scratch::new();
    let memory = Preference::new(dir.file("settings.json"));
    let frontends = window(&w, Frontend::Classic, Some(memory.clone()));
    assert!(frontends.app().choose("frontend-day"));
    assert_eq!(frontends.frontend(), Frontend::Day);
    assert_eq!(memory.load(), Frontend::Day);
    assert_eq!(labels_of(&frontends), ["Frontend", "Classic", "Board", "Day list"]);
    assert!(frontends.app().choose("frontend-board"));
    assert_eq!(frontends.frontend(), Frontend::Board);
    assert_eq!(memory.load(), Frontend::Board);
    assert!(frontends.app().choose("frontend-classic"));
    assert_eq!(frontends.frontend(), Frontend::Classic);
    assert_eq!(memory.load(), Frontend::Classic);
    assert_eq!(labels_of(&frontends).first().map(String::as_str), Some("Task"));
    assert!(
        frontends.app().choose("view-list"),
        "the classic commands work again"
    );
}

#[test]
fn a_choice_that_cannot_be_remembered_still_changes_the_frontend() {
    let w = World::new();
    let dir = Scratch::new();
    std::fs::write(dir.file("plain"), "x").unwrap();
    let memory = Preference::new(dir.file("plain").join("settings.json"));
    let frontends = window(&w, Frontend::Board, Some(memory));
    assert!(frontends.app().choose("frontend-day"));
    assert_eq!(frontends.frontend(), Frontend::Day);
}

#[test]
fn plan_asks_for_a_frontend_and_the_old_spellings_still_work() {
    let plan = |word: &str| parse(&args(&["--plan", word])).unwrap().plan;
    assert_eq!(plan("classic").and_then(Plan::frontend), Some(Frontend::Classic));
    assert_eq!(plan("board").and_then(Plan::frontend), Some(Frontend::Board));
    assert_eq!(plan("board2").and_then(Plan::frontend), Some(Frontend::Board));
    assert_eq!(plan("next").and_then(Plan::frontend), Some(Frontend::Day));
    assert_eq!(plan("next2").and_then(Plan::frontend), Some(Frontend::Day));
    assert_eq!(plan("board1").and_then(Plan::frontend), None);
    assert_eq!(plan("next1").and_then(Plan::frontend), None);
    assert!(parse(&args(&["--plan", "cube"])).is_err());
}

#[test]
fn a_relative_data_home_is_ignored_and_the_choice_is_saved_to_its_place() {
    use fs_tasks_app::data_dir_in;
    use std::ffi::OsString;
    use std::path::PathBuf;
    let os = |s: &str| Some(OsString::from(s));
    let home = PathBuf::from("/home/u/.local/share/freesynergy/tasks");
    assert_eq!(
        data_dir_in(os("/data"), os("/home/u")),
        Some(PathBuf::from("/data/freesynergy/tasks"))
    );
    // relative (and empty) is invalid by the XDG rule: the home is used
    assert_eq!(
        data_dir_in(os("rel/dir"), os("/home/u")),
        Some(home.clone())
    );
    assert_eq!(data_dir_in(os(""), os("/home/u")), Some(home.clone()));
    assert_eq!(data_dir_in(None, os("/home/u")), Some(home));
    assert_eq!(data_dir_in(os("rel"), None), None);
    // saving leaves no temporary file behind and the file holds the frontend
    let scratch = Scratch::new();
    let file = scratch.file("sub/settings.json");
    let memory = Preference::new(file.clone());
    memory.save(Frontend::Day).unwrap();
    assert_eq!(memory.load(), Frontend::Day);
    let left: Vec<_> = std::fs::read_dir(file.parent().unwrap()).unwrap().collect();
    assert_eq!(left.len(), 1);
}
