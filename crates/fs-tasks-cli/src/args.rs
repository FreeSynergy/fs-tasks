//! The words of the command line.
//!
//! Pattern: Command (pure parse) — no I/O, so every spelling is tested without a server.

use std::path::PathBuf;

/// What to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// What the server offers (`capabilities`).
    Capabilities,
    /// All tasks, one line each.
    List,
    /// One task.
    Show(String),
    /// A new task.
    Add {
        /// The title.
        title: String,
        /// Local due time, `2026-12-01T10:00:00`.
        due: Option<String>,
        /// IANA zone of `due`; default `Etc/UTC`.
        zone: Option<String>,
        /// A label (JSCalendar keyword).
        label: Option<String>,
    },
    /// Mark a task completed.
    Done(String),
    /// Delete a task.
    Delete(String),
    /// iCalendar `VTODO` of all tasks.
    Export,
    /// Import a `.ics` file.
    Import(PathBuf),
}

/// The help text.
pub const HELP: &str = "fs-tasks                    the window (needs the gui feature)
fs-tasks app [OPTIONS]     the window with options
fs-tasks serve [OPTIONS]   the server (--socket, --data, --actor, --instance)
fs-tasks [--socket PATH] <command>

Commands:
  capabilities               what the server offers
  list                       all tasks
  show <id>                  one task as JSCalendar
  add <title> [--due LOCAL] [--zone ZONE] [--label L]
                             a new task; LOCAL like 2026-12-01T10:00:00, ZONE like Europe/Berlin
  done <id>                  mark a task completed
  delete <id>                delete a task
  export                     all tasks as iCalendar
  import <file.ics>          import VTODOs

The socket defaults to $XDG_RUNTIME_DIR/freesynergy/tasks.sock.";

/// The words, parsed.
#[derive(Debug, PartialEq, Eq)]
pub struct Args {
    /// `--socket`.
    pub socket: Option<PathBuf>,
    /// The command; `None` for `--help`.
    pub command: Option<Command>,
}

/// Parse the words after the program name: options (`--socket PATH`, `--help`) first, then the command.
///
/// # Errors
/// An unknown word, a missing value, a missing id. The message says what is wrong.
pub fn parse(words: &[String]) -> Result<Args, String> {
    // Options come before the command word. After it every word is the command's own, so a title or
    // a value that looks like an option (`-h`, `--socket`) stays what the user typed.
    let mut socket = None;
    let mut at = 0;
    while let Some(word) = words.get(at) {
        match word.as_str() {
            "--help" | "-h" => {
                return Ok(Args {
                    socket,
                    command: None,
                });
            }
            "--socket" => {
                socket = Some(words.get(at + 1).ok_or("--socket needs a path")?.into());
                at += 2;
            }
            _ => break,
        }
    }
    let rest: Vec<&str> = words[at..].iter().map(String::as_str).collect();
    let (name, tail) = rest.split_first().ok_or("no command; try --help")?;
    let one = |what: &str| -> Result<String, String> {
        match tail {
            [x] => Ok((*x).into()),
            _ => Err(format!("{name} needs exactly one {what}")),
        }
    };
    let command = match *name {
        "capabilities" => Command::Capabilities,
        "list" => Command::List,
        "export" => Command::Export,
        "show" => Command::Show(one("id")?),
        "done" => Command::Done(one("id")?),
        "delete" => Command::Delete(one("id")?),
        "import" => Command::Import(one("file")?.into()),
        "add" => add(tail)?,
        other => return Err(format!("unknown command {other}; try --help")),
    };
    Ok(Args {
        socket,
        command: Some(command),
    })
}

fn add(tail: &[&str]) -> Result<Command, String> {
    let (title, mut it) = match tail.split_first() {
        Some((t, rest)) if !t.starts_with("--") => (*t, rest.iter()),
        _ => return Err("add needs a title".into()),
    };
    let (mut due, mut zone, mut label) = (None, None, None);
    while let Some(flag) = it.next() {
        let slot = match *flag {
            "--due" => &mut due,
            "--zone" => &mut zone,
            "--label" => &mut label,
            other => return Err(format!("unknown option {other} for add")),
        };
        *slot = Some((*it.next().ok_or(format!("{flag} needs a value"))?).to_string());
    }
    if zone.is_some() && due.is_none() {
        return Err("--zone only goes with --due".into());
    }
    Ok(Command::Add {
        title: title.into(),
        due,
        zone,
        label,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn the_socket_option_comes_before_the_command() {
        let a = parse(&words("--socket /x/y list")).unwrap();
        assert_eq!(a.socket, Some("/x/y".into()));
        assert_eq!(a.command, Some(Command::List));
        assert_eq!(
            parse(&words("--socket /s show abc")).unwrap().command,
            Some(Command::Show("abc".into()))
        );
        assert_eq!(parse(&words("-h")).unwrap().command, None);
    }

    #[test]
    fn add_takes_a_title_and_options() {
        let c = parse(&words(
            "add Plan --due 2026-12-01T10:00:00 --zone Europe/Berlin --label work",
        ))
        .unwrap()
        .command;
        assert_eq!(
            c,
            Some(Command::Add {
                title: "Plan".into(),
                due: Some("2026-12-01T10:00:00".into()),
                zone: Some("Europe/Berlin".into()),
                label: Some("work".into()),
            })
        );
    }

    #[test]
    fn after_the_command_every_word_is_the_commands_own() {
        // `-h` and `--socket` are values here, not options.
        let c = parse(&words("add -h")).unwrap().command;
        assert!(
            matches!(c, Some(Command::Add { ref title, .. }) if title == "-h"),
            "{c:?}"
        );
        assert_eq!(
            parse(&words("show --socket")).unwrap().command,
            Some(Command::Show("--socket".into()))
        );
    }

    #[test]
    fn mistakes_are_named() {
        for bad in [
            "",
            "frob",
            "show",
            "show a b",
            "add",
            "add --due x",
            "add T --due",
            "add T --x 1",
            "--socket",
        ] {
            assert!(parse(&words(bad)).is_err(), "{bad:?}");
        }
    }
}
