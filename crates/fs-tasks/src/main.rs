//! `fs-tasks`: the one program of FreeSynergy.Tasks (ADR 0063).
//!
//! Pattern: Command — the first word picks the subcommand and each one is a library of its own:
//! `serve` (`fs-tasks-server`), `app` or no word at all (`fs-tasks-app`, feature `gui`), anything else
//! is a command of the command line (`fs-tasks-cli`). Without the feature `gui` the bare command
//! prints the help, so a server does not hang on graphics libraries.

use std::ffi::OsString;

fn main() {
    let words: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(dispatch(&words));
}

/// Options only the window has; the command line does not know them.
const WINDOW_OPTIONS: [&str; 6] = [
    "--size",
    "--snapshot",
    "--store",
    "--view",
    "--import",
    "--export",
];

fn dispatch(words: &[String]) -> i32 {
    match words.split_first() {
        Some((first, rest)) if first == "serve" => {
            fs_tasks_server::serve_command(rest.iter().map(OsString::from))
        }
        Some((first, rest)) if first == "app" => window(rest),
        // The version, for people and for the package smoke test (`fs-tasks --version`): name, version, exit 0.
        Some((first, _)) if first == "--version" || first == "-V" => {
            println!("fs-tasks {}", env!("CARGO_PKG_VERSION"));
            0
        }
        None => window(&[]),
        // The window's own options may stand without the word `app` (`fs-tasks --size 1000x640`).
        Some((first, _)) if WINDOW_OPTIONS.contains(&first.as_str()) => window(words),
        Some(_) => fs_tasks_cli::cli_command(words),
    }
}

#[cfg(feature = "gui")]
fn window(args: &[String]) -> i32 {
    fs_tasks_app::app_command(args)
}

#[cfg(not(feature = "gui"))]
fn window(args: &[String]) -> i32 {
    if args.is_empty() {
        println!("{}", fs_tasks_cli::HELP);
        0
    } else {
        eprintln!("fs-tasks: this build has no window (feature gui)");
        2
    }
}
