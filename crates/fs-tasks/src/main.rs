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

fn dispatch(words: &[String]) -> i32 {
    match words.split_first() {
        Some((first, _)) if first == "--version" || first == "-V" => {
            println!("fs-tasks {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Some((first, rest)) if first == "serve" => {
            fs_tasks_server::serve_command(rest.iter().map(OsString::from))
        }
        Some((first, rest)) if first == "app" => window(rest),
        None => window(&[]),
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
