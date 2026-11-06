//! The command line.

use std::path::PathBuf;

use fs_components_workbenches::tasks::TaskView;
use fs_tasks_app::args::{Args, parse};

fn run(args: &[&str]) -> Result<Args, String> {
    let args: Vec<String> = args.iter().map(ToString::to_string).collect();
    parse(&args)
}

#[test]
fn nothing_given_means_the_defaults() {
    assert_eq!(run(&[]).unwrap(), Args::default());
}

#[test]
fn every_option_is_read() {
    let got = run(&[
        "--store",
        "a",
        "--import",
        "b",
        "--export",
        "c",
        "--export-sensitive",
        "d",
        "--export-sensitive",
        "e",
        "--snapshot",
        "f",
        "--size",
        "640x480",
        "--view",
        "status",
    ])
    .unwrap();
    assert_eq!(got.store, Some(PathBuf::from("a")));
    assert_eq!(got.import, Some(PathBuf::from("b")));
    assert_eq!(got.export, Some(PathBuf::from("c")));
    assert_eq!(got.export_sensitive, ["d", "e"]);
    assert_eq!(got.snapshot, Some(PathBuf::from("f")));
    assert_eq!(got.size, (640, 480));
    assert_eq!(got.view, Some(TaskView::Status));
    assert_eq!(run(&["--view", "list"]).unwrap().view, Some(TaskView::List));
}

#[test]
fn what_cannot_be_read_is_an_error_in_words() {
    for bad in [
        &["--nope"][..],
        &["--store"],
        &["--size", "wide"],
        &["--view", "board"],
        &["--export-sensitive", "x"],
    ] {
        assert!(run(bad).is_err(), "{bad:?}");
    }
}
