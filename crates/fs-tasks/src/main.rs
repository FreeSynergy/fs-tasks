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
/// Kept in step with `fs-tasks-app/src/args.rs` (all its options except `-h`/`--help`, which the CLI answers too).
const WINDOW_OPTIONS: [&str; 13] = [
    "--size",
    "--snapshot",
    "--store",
    "--connect",
    "--view",
    "--plan",
    "--open",
    "--list",
    "--sample",
    "--today",
    "--import",
    "--export",
    "--export-sensitive",
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
        Some(_) if routes_to_window(words) => window(words),
        Some(_) => fs_tasks_cli::cli_command(words),
    }
}

/// Whether the first word is an option of the window (then the word `app` may be left out).
fn routes_to_window(words: &[String]) -> bool {
    words
        .first()
        .is_some_and(|first| WINDOW_OPTIONS.contains(&first.as_str()))
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

#[cfg(test)]
mod tests {
    use super::routes_to_window;

    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|w| (*w).to_string()).collect()
    }

    #[test]
    fn every_window_option_as_first_word_goes_to_the_window() {
        for option in super::WINDOW_OPTIONS {
            assert!(routes_to_window(&words(&[option, "x"])), "{option}");
        }
    }

    #[test]
    fn the_plan_option_of_the_typing_test_goes_to_the_window() {
        let args = words(&["--plan", "classic", "--size", "1000x640", "--store", "x.db"]);
        assert!(routes_to_window(&args));
    }

    #[test]
    fn commands_and_the_command_lines_options_stay_with_the_cli() {
        for args in [
            &["list"][..],
            &["add", "x"],
            &["--socket", "s", "list"],
            &[],
        ] {
            assert!(!routes_to_window(&words(args)), "{args:?}");
        }
    }
}
