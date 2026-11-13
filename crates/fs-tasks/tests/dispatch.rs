//! The first word picks the subcommand (ADR 0063).

use std::process::Command;

fn run(args: &[&str]) -> (Option<i32>, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_fs-tasks"))
        .args(args)
        .output()
        .unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn help_names_all_three_ways_in() {
    let (code, out, _) = run(&["--help"]);
    assert_eq!(code, Some(0));
    for word in ["fs-tasks serve", "fs-tasks app", "list", "import"] {
        assert!(out.contains(word), "{word} missing in {out}");
    }
}

#[test]
fn an_unknown_command_is_refused_with_2() {
    let (code, _, err) = run(&["frob"]);
    assert_eq!(code, Some(2));
    assert!(err.contains("unknown command frob"), "{err}");
}

#[test]
fn serve_help_is_the_servers_own() {
    let (code, out, _) = run(&["serve", "--help"]);
    assert_eq!(code, Some(0));
    assert!(
        out.contains("--socket") && out.contains("--instance"),
        "{out}"
    );
}

#[test]
fn a_command_reaches_a_missing_server_with_exit_1() {
    // Nobody answers and the server cannot be started there (`/dev/null` is no folder): exit 1 at once, and no
    // server of the test is left running on the real data folder.
    let (code, _, err) = run(&["--socket", "/dev/null/no-such.sock", "list"]);
    assert_eq!(code, Some(1));
    assert!(err.contains("fs-tasks:"), "{err}");
}

#[cfg(not(feature = "gui"))]
#[test]
fn without_the_gui_the_bare_command_prints_help() {
    let (code, out, _) = run(&[]);
    assert_eq!(code, Some(0));
    assert!(out.contains("fs-tasks serve"), "{out}");
}

#[cfg(not(feature = "gui"))]
#[test]
fn the_plan_option_reaches_the_window_not_the_command_line() {
    let (code, _, err) = run(&["--plan", "classic", "--size", "1000x640", "--store", "x.db"]);
    assert_eq!(code, Some(2));
    assert!(err.contains("no window"), "{err}");
}

/// Every option the window knows, read from the window's own parser (`args.rs`), so a new one cannot be forgotten.
fn window_options() -> Vec<String> {
    let source = include_str!("../../fs-tasks-app/src/args.rs");
    let mut found = Vec::new();
    for line in source.lines() {
        let Some((names, _)) = line.trim().split_once("=>") else {
            continue;
        };
        if !names.trim_start().starts_with("\"--") && !names.trim_start().starts_with("\"-h\"") {
            continue;
        }
        for word in names.split('"').skip(1).step_by(2) {
            if word.starts_with("--") && word != "--help" {
                found.push(word.to_string());
            }
        }
    }
    found
}

#[test]
fn every_option_of_the_window_goes_to_the_window_in_the_real_program() {
    let options = window_options();
    assert!(options.len() >= 13, "found only {options:?}");
    for option in &options {
        // A bad value and an unknown flag after it: the window refuses it (1) before it does anything; the command
        // line would answer 2 (a mistake in the words).
        let (code, _, err) = run(&[option, "x", "--no-such-flag"]);
        if cfg!(feature = "gui") {
            assert_eq!(code, Some(1), "{option}: {err}");
        } else {
            assert!(err.contains("no window"), "{option}: {err}");
        }
    }
}

#[test]
fn the_version_is_the_name_and_the_package_version_with_exit_0() {
    for flag in ["--version", "-V"] {
        let (code, out, err) = run(&[flag]);
        assert_eq!(code, Some(0), "{flag}: {err}");
        assert_eq!(
            out.trim(),
            format!("fs-tasks {}", env!("CARGO_PKG_VERSION"))
        );
    }
}
