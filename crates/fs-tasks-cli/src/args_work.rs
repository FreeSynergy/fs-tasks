//! The words of `add`, `list` and the worker commands.
//!
//! Pattern: Command (pure parse) — a command's words are a list of positional words and `--flag [value]` pairs;
//! `flags` splits them, each command then names what it takes and what it refuses.

use crate::args::{Command, NewTask, Query, To, Work};

/// The words of one command, split.
struct Flags<'a> {
    words: Vec<&'a str>,
    values: Vec<(&'a str, &'a str)>,
    switches: Vec<&'a str>,
}

impl<'a> Flags<'a> {
    /// `valued` flags take the next word, `switches` stand alone; any other `--word` is a mistake. A word that is
    /// not a flag is positional (one that starts with `--` has to be a known flag).
    fn split(tail: &[&'a str], valued: &[&str], switches: &[&str]) -> Result<Self, String> {
        let mut out = Flags {
            words: vec![],
            values: vec![],
            switches: vec![],
        };
        let mut it = tail.iter();
        while let Some(&word) = it.next() {
            if valued.contains(&word) {
                let value = it.next().ok_or(format!("{word} needs a value"))?;
                out.values.push((word, value));
            } else if switches.contains(&word) {
                out.switches.push(word);
            } else if word.starts_with("--") {
                return Err(format!("unknown option {word}"));
            } else {
                out.words.push(word);
            }
        }
        Ok(out)
    }

    fn get(&self, flag: &str) -> Option<String> {
        self.values
            .iter()
            .rev()
            .find(|(f, _)| *f == flag)
            .map(|(_, v)| (*v).to_string())
    }

    fn need(&self, flag: &str) -> Result<String, String> {
        self.get(flag).ok_or(format!("{flag} is needed"))
    }

    fn has(&self, flag: &str) -> bool {
        self.switches.contains(&flag)
    }

    /// Exactly `n` positional words.
    fn exactly(&self, n: usize, command: &str, what: &str) -> Result<Vec<String>, String> {
        if self.words.len() == n {
            Ok(self.words.iter().map(|w| (*w).to_string()).collect())
        } else {
            Err(format!("{command} needs {what}"))
        }
    }
}

/// The words after `add`.
pub(crate) fn parse_add(tail: &[&str]) -> Result<NewTask, String> {
    // The title comes first, so a title that looks like an option stays a title.
    let (title, rest) = match tail.split_first() {
        Some((t, rest)) if !t.starts_with("--") => (*t, rest),
        _ => return Err("add needs a title".into()),
    };
    let f = Flags::split(
        rest,
        &[
            "--due",
            "--zone",
            "--label",
            "--project",
            "--objective",
            "--as",
        ],
        &["--subjective", "--json"],
    )
    .map_err(|e| format!("{e} for add"))?;
    if !f.words.is_empty() {
        return Err(format!("unknown option {} for add", f.words[0]));
    }
    let made = NewTask {
        title: title.into(),
        due: f.get("--due"),
        zone: f.get("--zone"),
        label: f.get("--label"),
        project: f.get("--project"),
        objective: f.get("--objective"),
        subjective: f.has("--subjective"),
        creator: f.get("--as"),
        json: f.has("--json"),
    };
    if made.zone.is_some() && made.due.is_none() {
        return Err("--zone only goes with --due".into());
    }
    if made.objective.is_some() && made.subjective {
        return Err("a task is objective (--objective) or subjective, not both".into());
    }
    Ok(made)
}

/// The words after `list`.
pub(crate) fn parse_list(tail: &[&str]) -> Result<Query, String> {
    let f = Flags::split(tail, &["--project", "--status"], &["--json"])?;
    if !f.words.is_empty() {
        return Err("list takes no word, only --project, --status, --json".into());
    }
    let status = f.get("--status");
    if let Some(s) = &status
        && !["open", "in-progress", "done", "failed", "cancelled"].contains(&s.as_str())
    {
        return Err(format!(
            "unknown status {s}; open, in-progress, done, failed or cancelled"
        ));
    }
    Ok(Query {
        project: f.get("--project"),
        status,
        json: f.has("--json"),
    })
}

fn to_of(word: &str) -> Result<To, String> {
    match word {
        "in-progress" => Ok(To::InProgress),
        "done" => Ok(To::Done),
        "failed" => Ok(To::Failed),
        other => Err(format!(
            "unknown status {other}; in-progress, done or failed"
        )),
    }
}

/// The words of a worker command (`claim`, `release`, `next`, `status`, `note`, `check`).
pub(crate) fn parse_work(name: &str, tail: &[&str]) -> Result<Command, String> {
    let (valued, switches): (&[&str], &[&str]) = match name {
        "claim" => (&["--as", "--until"], &["--json"]),
        "release" => (&["--as"], &["--json"]),
        "next" => (&["--as", "--project"], &["--json"]),
        "status" => (&["--note", "--report", "--cost"], &["--json"]),
        "note" | "check" => (&[], &["--json"]),
        other => return Err(format!("unknown command {other}; try --help")),
    };
    // A note's text is a word of its own and may look like an option only if it is quoted as one word.
    let f = if name == "note" {
        Flags {
            words: tail.iter().copied().filter(|w| *w != "--json").collect(),
            values: vec![],
            switches: tail.iter().copied().filter(|w| *w == "--json").collect(),
        }
    } else {
        Flags::split(tail, valued, switches).map_err(|e| format!("{e} for {name}"))?
    };
    let work = match name {
        "claim" => Work::Claim {
            id: f.exactly(1, name, "a task id")?.remove(0),
            name: f.need("--as")?,
            until: f.get("--until"),
        },
        "release" => Work::Release {
            id: f.exactly(1, name, "a task id")?.remove(0),
            name: f.need("--as")?,
        },
        "next" => {
            f.exactly(0, name, "no word")?;
            Work::Next {
                name: f.need("--as")?,
                project: f.get("--project"),
            }
        }
        "status" => {
            let mut w = f.exactly(2, name, "a task id and a status")?.into_iter();
            let (id, to) = (w.next().unwrap_or_default(), w.next().unwrap_or_default());
            let cost = f.get("--cost");
            if let Some(c) = &cost
                && !c.parse::<f64>().is_ok_and(|c| c.is_finite() && c >= 0.0)
            {
                return Err("--cost is a number of USD, 0 or more".into());
            }
            Work::Status {
                id,
                to: to_of(&to)?,
                note: f.get("--note"),
                report: f.get("--report"),
                cost,
            }
        }
        "note" => {
            let mut w = f.exactly(2, name, "a task id and one text")?.into_iter();
            Work::Note {
                id: w.next().unwrap_or_default(),
                text: w.next().unwrap_or_default(),
            }
        }
        _ => Work::Check(f.exactly(1, name, "a task id")?.remove(0)),
    };
    Ok(Command::Work {
        work,
        json: f.has("--json"),
    })
}
