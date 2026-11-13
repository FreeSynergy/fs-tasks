//! What may run as a check command, and whose tasks may start one.
//!
//! Pattern: Specification + Allowlist — a check command is executable code. It runs only for a task made on this
//! machine (`origin` local) by an actor on the trust list, and only if its program is on the program list. Both
//! lists are a setting (`check-policy.json` in the data folder); a missing file means the defaults, a file that
//! cannot be read means no check runs.

use std::path::{Path, PathBuf};

use fs_tasks_core::Task;
use fs_tasks_core::model::ORIGIN_LOCAL;
use serde_json::Value;

/// The actor that `add` writes as creator without `--as`: the owner of this machine.
pub const OWNER: &str = "https://localhost/actors/owner";
/// Programs a check may start besides `bash scripts/...`.
const PROGRAMS: [&str; 2] = ["cargo", "fs-release"];
/// The longest a check may run, in seconds, unless the setting says less.
const MAX_SECONDS: u64 = 3600;

/// The settings of the checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Policy {
    /// Actor IRIs whose local tasks may run a check.
    pub trusted: Vec<String>,
    /// More program names (no path) a check may start.
    pub programs: Vec<String>,
    /// The time limit of one check.
    pub seconds: u64,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            trusted: vec![OWNER.into()],
            programs: vec![],
            seconds: 600,
        }
    }
}

/// The IRI of an actor given as a bare name (`claude-1`) or as an IRI.
#[must_use]
pub fn actor_iri(name: &str) -> String {
    if name.contains("://") {
        name.into()
    } else {
        format!("https://localhost/actors/{name}")
    }
}

/// The folder of the program's own files: `$XDG_DATA_HOME/freesynergy/tasks`, else under `$HOME/.local/share`.
#[must_use]
pub fn data_dir() -> Option<PathBuf> {
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty());
    let base = match var("XDG_DATA_HOME") {
        Some(x) if Path::new(&x).is_absolute() => PathBuf::from(x),
        _ => PathBuf::from(var("HOME")?).join(".local/share"),
    };
    Some(base.join("freesynergy").join("tasks"))
}

impl Policy {
    /// Read the setting from `dir`; the defaults if there is no file.
    ///
    /// # Errors
    /// The file exists and is not what it should be (then no check may run).
    pub fn load_from(dir: Option<&Path>) -> Result<Self, String> {
        let Some(file) = dir.map(|d| d.join("check-policy.json")) else {
            return Ok(Self::default());
        };
        let text = match std::fs::read_to_string(&file) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(format!("{}: {e}", file.display())),
        };
        let bad = |what: &str| format!("{}: {what}", file.display());
        let v: Value = serde_json::from_str(&text).map_err(|e| bad(&e.to_string()))?;
        let list = |key: &str| -> Result<Option<Vec<String>>, String> {
            let Some(v) = v.get(key) else { return Ok(None) };
            v.as_array()
                .and_then(|a| a.iter().map(|x| x.as_str().map(String::from)).collect())
                .map(Some)
                .ok_or_else(|| bad(&format!("`{key}` is a list of texts")))
        };
        let mut policy = Self::default();
        if let Some(names) = list("trusted_actors")? {
            policy.trusted = names.iter().map(|n| actor_iri(n)).collect();
        }
        if let Some(programs) = list("programs")? {
            policy.programs = programs;
        }
        if let Some(s) = v.get("timeout_secs") {
            policy.seconds = s
                .as_u64()
                .filter(|s| (1..=MAX_SECONDS).contains(s))
                .ok_or_else(|| bad("`timeout_secs` is 1 to 3600"))?;
        }
        Ok(policy)
    }

    /// Why the check command of this task must not run: it was not made here (`foreign origin`; also a task without
    /// an origin) or not by a trusted actor. `None` if it was made here by a trusted actor.
    #[must_use]
    pub fn refusal(&self, task: &Task) -> Option<&'static str> {
        if task.origin() != Some(ORIGIN_LOCAL) {
            return Some("check command not run: foreign origin");
        }
        let trusted = task
            .created_by()
            .is_some_and(|a| self.trusted.iter().any(|t| t == a));
        (!trusted).then_some("check command not run: creator not trusted")
    }

    /// May this word list run? `bash` only with a script under `scripts/` (relative, no `..`, no symlink out of the
    /// folder); every other program by bare name from the lists, never by a path.
    ///
    /// # Errors
    /// The reason, in words.
    pub fn program_ok(&self, argv: &[String]) -> Result<(), String> {
        let name = argv.first().ok_or("the check command is empty")?;
        if name == "bash" {
            let script = argv.get(1).map_or("", String::as_str);
            return if script_ok(script) {
                Ok(())
            } else {
                Err("bash may only run a script under scripts/".into())
            };
        }
        let listed = PROGRAMS.contains(&name.as_str()) || self.programs.contains(name);
        if name.contains(['/', '\\']) || name.contains("..") || !listed {
            return Err(format!("program not allowed: {name}"));
        }
        Ok(())
    }
}

/// `scripts/...`, relative, without `.`, `..` or empty parts; if it exists, its real place is under `./scripts`.
fn script_ok(path: &str) -> bool {
    let plain = path.strip_prefix("scripts/").is_some_and(|rest| {
        !rest.is_empty()
            && !path.contains(['\\', '\0'])
            && path.split('/').all(|p| !matches!(p, "" | "." | ".."))
    });
    if !plain {
        return false;
    }
    match (
        Path::new(path).canonicalize(),
        Path::new("scripts").canonicalize(),
    ) {
        (Ok(real), Ok(root)) => real.starts_with(root),
        (Err(_), _) => true,
        (Ok(_), Err(_)) => false,
    }
}
