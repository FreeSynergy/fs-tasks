//! The rename "face" to "frontend": the menu says "Frontend" and no visible text says "face"; the choice kept by an
//! older version (key `face`) is read once and migrated, and the old key stays.
//!
//! Pattern: Memento migration test over a real file; counter-checks that the old and the new reading differ.

mod common;

use std::sync::Arc;

use common::{Scratch, World};
use fs_tasks_app::args::HELP;
use fs_tasks_app::{Day, Frontend, FrontendOptions, Frontends, Preference};
use fs_tasks_core::TaskItems;

fn read(file: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap()
}

#[test]
fn a_choice_kept_under_the_old_key_is_read_and_migrated() {
    let dir = Scratch::new();
    let file = dir.file("settings.json");
    std::fs::write(&file, "{\"face\": \"day\"}").unwrap();
    let memory = Preference::new(file.clone());
    assert_eq!(memory.load(), Frontend::Day);
    let after = read(&file);
    assert_eq!(after["frontend"], "day", "written under the new key");
    assert_eq!(after["face"], "day", "the old key is not removed");
    assert_eq!(dir.names(), ["settings.json"], "no temporary file is left");
    // the second reading comes from the new key
    assert_eq!(memory.load(), Frontend::Day);
}

#[test]
fn the_new_key_wins_over_the_old_one_and_is_not_rewritten() {
    let dir = Scratch::new();
    let file = dir.file("settings.json");
    let text = "{\"frontend\": \"classic\", \"face\": \"day\"}";
    std::fs::write(&file, text).unwrap();
    assert_eq!(Preference::new(file.clone()).load(), Frontend::Classic);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
}

#[test]
fn counter_check_without_any_key_nothing_is_migrated() {
    let dir = Scratch::new();
    let file = dir.file("settings.json");
    std::fs::write(&file, "{\"other\": \"day\"}").unwrap();
    assert_eq!(Preference::new(file.clone()).load(), Frontend::DEFAULT);
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "{\"other\": \"day\"}"
    );
}

#[test]
fn the_menu_says_frontend_and_no_visible_text_says_face() {
    let w = World::new();
    let items: Arc<dyn TaskItems> = w.repo.clone();
    let options = FrontendOptions {
        today: Day::parse("2026-11-12").unwrap(),
        wait: false,
        list: None,
        open: None,
        view: None,
        remember: None,
        live: None,
    };
    let window = Frontends::new(
        items,
        w.runtime.handle().clone(),
        Frontend::Classic,
        options,
    )
    .unwrap();
    let labels: Vec<String> = window.app().with_shell(|shell| {
        shell
            .menus()
            .iter()
            .flat_map(|menu| {
                let lines = menu.entries.iter().map(|e| e.label.clone());
                std::iter::once(menu.label.clone()).chain(lines)
            })
            .collect()
    });
    assert!(labels.iter().any(|l| l == "Frontend"), "{labels:?}");
    for text in labels.iter().map(String::as_str).chain([HELP]) {
        let low = text.to_lowercase();
        assert!(!low.contains("gesicht"), "{text}");
        assert!(
            !low.split(|c: char| !c.is_alphabetic())
                .any(|w| w == "face" || w == "faces"),
            "{text}"
        );
    }
    // counter-check: the test would see an old word
    assert!(
        "the Face menu"
            .to_lowercase()
            .split(' ')
            .any(|w| w == "face")
    );
}
