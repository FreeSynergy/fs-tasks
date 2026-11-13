//! The words of the command line.
//!
//! Pattern: Command (pure parse) — no I/O, so every spelling is tested without a server.

use std::path::PathBuf;

use crate::args_work::{parse_add, parse_list, parse_work};

/// A new task.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NewTask {
    /// The title.
    pub title: String,
    /// Local due time, `2026-12-01T10:00:00`.
    pub due: Option<String>,
    /// IANA zone of `due`; default `Etc/UTC`.
    pub zone: Option<String>,
    /// A label (JSCalendar keyword).
    pub label: Option<String>,
    /// The project: the label `project:<name>`.
    pub project: Option<String>,
    /// The check command: the task is objective.
    pub objective: Option<String>,
    /// Done is the actor's word (the default; the flag only says it out loud).
    pub subjective: bool,
    /// The actor that makes it (a name or an IRI); default the owner.
    pub creator: Option<String>,
    /// Print JSON.
    pub json: bool,
}

/// The tasks to list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    /// Only this project.
    pub project: Option<String>,
    /// Only this status: `open`, `in-progress`, `done`, `failed`, `cancelled`.
    pub status: Option<String>,
    /// Print JSON.
    pub json: bool,
}

/// The status a worker sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum To {
    /// Work has begun (`in-process`).
    InProgress,
    /// Finished (`completed`).
    Done,
    /// Tried and did not work (`failed`).
    Failed,
}

/// What a worker does with a task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Work {
    /// Take it, for `until` (a length like `90m`, `2h`, or a UTC time; at most 24 hours).
    Claim {
        /// The task.
        id: String,
        /// The worker's name.
        name: String,
        /// How long.
        until: Option<String>,
    },
    /// Give the claim back.
    Release {
        /// The task.
        id: String,
        /// The worker's name.
        name: String,
    },
    /// Set the status; a closing one may carry the result line, the report and the cost.
    Status {
        /// The task.
        id: String,
        /// The new status.
        to: To,
        /// A short result line.
        note: Option<String>,
        /// A path or URL of the report.
        report: Option<String>,
        /// The cost in USD.
        cost: Option<String>,
    },
    /// The short line of a task.
    Note {
        /// The task.
        id: String,
        /// The line.
        text: String,
    },
    /// The next ready task, taken.
    Next {
        /// The worker's name.
        name: String,
        /// Only this project.
        project: Option<String>,
    },
    /// Run the check command of an objective task.
    Check(String),
}

/// What to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// What the server offers (`capabilities`).
    Capabilities,
    /// Tasks, one line each.
    List(Query),
    /// One task.
    Show(String),
    /// A new task.
    Add(NewTask),
    /// Mark a task completed (an objective task needs its passed check).
    Done(String),
    /// Delete a task.
    Delete(String),
    /// iCalendar `VTODO` of all tasks.
    Export,
    /// Import a `.ics` file.
    Import(PathBuf),
    /// What a worker does.
    Work {
        /// The step.
        work: Work,
        /// Print JSON.
        json: bool,
    },
}

impl Command {
    /// Does the caller want JSON (also for the error)?
    #[must_use]
    pub fn json(&self) -> bool {
        match self {
            Self::List(q) => q.json,
            Self::Add(n) => n.json,
            Self::Work { json, .. } => *json,
            _ => false,
        }
    }
}

/// The help text.
pub const HELP: &str = "fs-tasks                    the window (needs the gui feature)
fs-tasks app [OPTIONS]     the window with options
fs-tasks serve [OPTIONS]   the server (--socket, --data, --actor, --instance)
fs-tasks [--socket PATH] [--data DIR] <command>

Commands:
  capabilities               what the server offers
  list [--project P] [--status S] [--json]
                             tasks; S is open, in-progress, done, failed or cancelled
  show <id>                  one task as JSCalendar
  add <title> [--due LOCAL] [--zone ZONE] [--label L] [--project P] [--as NAME]
              [--objective CMD | --subjective] [--json]
                             a new task; LOCAL like 2026-12-01T10:00:00, ZONE like Europe/Berlin
  done <id>                  mark a task completed
  delete <id>                delete a task
  export                     all tasks as iCalendar
  import <file.ics>          import VTODOs
Work (exit 0 ok, 1 error, 3 conflict: somebody else holds the claim; add --json for JSON):
  claim <id> --as NAME [--until 90m|2h|UTC] [--json]
  release <id> --as NAME [--json]
  next --as NAME [--project P] [--json]
                             the first ready task, claimed; nothing printed (JSON: null) if none
  status <id> in-progress|done|failed [--note LINE] [--report PATH|URL] [--cost USD] [--json]
  note <id> <text>           a short line (at most 500 bytes) shown on the board
  check <id>                 run the check command of an objective task (done needs it to pass)

The socket defaults to $XDG_RUNTIME_DIR/freesynergy/tasks.sock. A command starts the local server
(`fs-tasks serve`, data in $XDG_DATA_HOME/freesynergy/tasks) if none is there.";

/// The words, parsed.
#[derive(Debug, PartialEq, Eq)]
pub struct Args {
    /// `--socket`.
    pub socket: Option<PathBuf>,
    /// `--data`: the data folder of a server this command has to start.
    pub data: Option<PathBuf>,
    /// The command; `None` for `--help`.
    pub command: Option<Command>,
}

/// Parse the words after the program name: options (`--socket PATH`, `--data DIR`, `--help`) first, then the command.
///
/// # Errors
/// An unknown word, a missing value, a missing id. The message says what is wrong.
pub fn parse(words: &[String]) -> Result<Args, String> {
    // Options come before the command word. After it every word is the command's own, so a title or
    // a value that looks like an option (`-h`, `--socket`) stays what the user typed.
    let (mut socket, mut data) = (None, None);
    let mut at = 0;
    while let Some(word) = words.get(at) {
        let slot: &mut Option<PathBuf> = match word.as_str() {
            "--help" | "-h" => {
                return Ok(Args {
                    socket,
                    data,
                    command: None,
                });
            }
            "--socket" => &mut socket,
            "--data" => &mut data,
            _ => break,
        };
        *slot = Some(
            words
                .get(at + 1)
                .ok_or(format!("{word} needs a path"))?
                .into(),
        );
        at += 2;
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
        "export" => Command::Export,
        "show" => Command::Show(one("id")?),
        "done" => Command::Done(one("id")?),
        "delete" => Command::Delete(one("id")?),
        "import" => Command::Import(one("file")?.into()),
        "add" => Command::Add(parse_add(tail)?),
        "list" => Command::List(parse_list(tail)?),
        other => parse_work(other, tail)?,
    };
    Ok(Args {
        socket,
        data,
        command: Some(command),
    })
}
