//! The faces of the window: the menu names them, a choice changes the face and is remembered, a broken memory means
//! the default, and `--plan` asks for a face without changing the memory.
//!
//! Pattern: Command tests through the shell's own way of choosing an entry, over the real core; the memory is a real file.

mod common;

use std::sync::Arc;

use common::{Scratch, World};
use fs_tasks_app::args::{Plan, parse};
use fs_tasks_app::{Day, Face, FaceOptions, Faces, Preference};
use fs_tasks_core::TaskItems;

fn options(remember: Option<Preference>) -> FaceOptions {
    FaceOptions {
        today: Day::parse("2026-11-12").unwrap(),
        wait: false,
        list: None,
        open: None,
        view: None,
        remember,
    }
}

fn window(w: &World, start: Face, remember: Option<Preference>) -> Faces {
    let items: Arc<dyn TaskItems> = w.repo.clone();
    Faces::new(items, w.runtime.handle().clone(), start, options(remember)).unwrap()
}

fn labels_of(faces: &Faces) -> Vec<String> {
    faces.app().with_shell(|shell| {
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
fn every_face_is_saved_and_loaded() {
    let dir = Scratch::new();
    let memory = Preference::new(dir.file("settings.json"));
    for face in Face::ALL {
        memory.save(face).unwrap();
        assert_eq!(memory.load(), face);
    }
    assert_eq!(dir.names(), ["settings.json"], "no temporary file is left");
}

#[test]
fn no_file_means_the_board() {
    let dir = Scratch::new();
    let memory = Preference::new(dir.file("settings.json"));
    assert_eq!(memory.load(), Face::Board);
    assert_eq!(Face::DEFAULT, Face::Board);
}

#[test]
fn a_broken_file_means_the_default_and_the_next_choice_replaces_it() {
    let dir = Scratch::new();
    let file = dir.file("settings.json");
    let memory = Preference::new(file.clone());
    for broken in ["", "not json", "[1, 2]", "{\"face\": 7}", "{\"face\": \"cube\"}", "{}"] {
        std::fs::write(&file, broken).unwrap();
        assert_eq!(memory.load(), Face::Board, "{broken:?}");
    }
    memory.save(Face::Day).unwrap();
    assert_eq!(memory.load(), Face::Day);
}

#[test]
fn saving_makes_the_folder_and_a_failure_is_an_error_not_a_crash() {
    let dir = Scratch::new();
    let deep = Preference::new(dir.file("a").join("b").join("settings.json"));
    deep.save(Face::Classic).unwrap();
    assert_eq!(deep.load(), Face::Classic);
    std::fs::write(dir.file("plain"), "x").unwrap();
    let blocked = Preference::new(dir.file("plain").join("settings.json"));
    assert!(blocked.save(Face::Day).is_err());
    assert_eq!(blocked.load(), Face::Board);
}

#[test]
fn the_menu_names_the_faces_and_the_shown_one_cannot_be_chosen() {
    let w = World::new();
    let faces = window(&w, Face::Board, None);
    assert_eq!(labels_of(&faces), ["Face", "Classic", "Board", "Day list"]);
    assert!(!faces.app().choose("face-board"), "it is shown already");
    assert!(!faces.app().choose("face-cube"), "no such face");
    assert!(!faces.app().choose("task-new"), "the classic commands belong to the classic face");
    assert_eq!(faces.face(), Face::Board);
}

#[test]
fn the_classic_face_keeps_its_own_menus_beside_the_face_menu() {
    let w = World::new();
    let faces = window(&w, Face::Classic, None);
    assert_eq!(
        labels_of(&faces),
        ["Task", "New", "View", "List", "Status", "Face", "Classic", "Board", "Day list"]
    );
    assert!(faces.app().choose("view-status"));
    assert!(faces.app().choose("task-new"));
}

#[test]
fn choosing_a_face_changes_it_and_the_menus_and_is_remembered() {
    let w = World::new();
    let dir = Scratch::new();
    let memory = Preference::new(dir.file("settings.json"));
    let faces = window(&w, Face::Classic, Some(memory.clone()));
    assert!(faces.app().choose("face-day"));
    assert_eq!(faces.face(), Face::Day);
    assert_eq!(memory.load(), Face::Day);
    assert_eq!(labels_of(&faces), ["Face", "Classic", "Board", "Day list"]);
    assert!(faces.app().choose("face-board"));
    assert_eq!(faces.face(), Face::Board);
    assert_eq!(memory.load(), Face::Board);
    assert!(faces.app().choose("face-classic"));
    assert_eq!(faces.face(), Face::Classic);
    assert_eq!(memory.load(), Face::Classic);
    assert_eq!(labels_of(&faces).first().map(String::as_str), Some("Task"));
    assert!(faces.app().choose("view-list"), "the classic commands work again");
}

#[test]
fn a_choice_that_cannot_be_remembered_still_changes_the_face() {
    let w = World::new();
    let dir = Scratch::new();
    std::fs::write(dir.file("plain"), "x").unwrap();
    let memory = Preference::new(dir.file("plain").join("settings.json"));
    let faces = window(&w, Face::Board, Some(memory));
    assert!(faces.app().choose("face-day"));
    assert_eq!(faces.face(), Face::Day);
}

#[test]
fn plan_asks_for_a_face_and_the_old_spellings_still_work() {
    let plan = |word: &str| parse(&args(&["--plan", word])).unwrap().plan;
    assert_eq!(plan("classic").and_then(Plan::face), Some(Face::Classic));
    assert_eq!(plan("board").and_then(Plan::face), Some(Face::Board));
    assert_eq!(plan("board2").and_then(Plan::face), Some(Face::Board));
    assert_eq!(plan("next").and_then(Plan::face), Some(Face::Day));
    assert_eq!(plan("next2").and_then(Plan::face), Some(Face::Day));
    assert_eq!(plan("board1").and_then(Plan::face), None);
    assert_eq!(plan("next1").and_then(Plan::face), None);
    assert!(parse(&args(&["--plan", "cube"])).is_err());
}
