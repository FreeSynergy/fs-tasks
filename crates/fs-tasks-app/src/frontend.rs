//! The frontend of the window: which of the three screens the person looks at, and the memory of the choice.
//!
//! Pattern: Value object (`Frontend`) + Memento holder (`Preference`, one small file in the data folder). A file that is
//! missing, unreadable or broken means the default; a write that fails is reported, never fatal.

use std::path::PathBuf;

use serde_json::{Value, json};

/// The key under which versions up to 0.0.8 kept the choice (read once, then migrated to `frontend`).
const LEGACY_KEY: &str = "face";

/// A frontend of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frontend {
    /// The first window: list and status view.
    Classic,
    /// A: the board, columns by status, cards.
    Board,
    /// B: the day list, Today, Upcoming, Anytime and the labels.
    Day,
}

impl Frontend {
    /// What a new installation shows.
    pub const DEFAULT: Frontend = Frontend::Board;

    /// All frontends, in the order of the menu.
    pub const ALL: [Frontend; 3] = [Frontend::Classic, Frontend::Board, Frontend::Day];

    /// The stable name (in the settings file and in the id of the menu entry).
    pub fn id(self) -> &'static str {
        match self {
            Frontend::Classic => "classic",
            Frontend::Board => "board",
            Frontend::Day => "day",
        }
    }

    /// The frontend with this stable name.
    pub fn from_id(id: &str) -> Option<Frontend> {
        Frontend::ALL.into_iter().find(|f| f.id() == id)
    }

    /// What the menu says.
    pub fn label(self) -> &'static str {
        match self {
            Frontend::Classic => "Classic",
            Frontend::Board => "Board",
            Frontend::Day => "Day list",
        }
    }
}

/// The data folder of the program from the two variables: `$XDG_DATA_HOME` if it is an absolute path (the XDG rule: a
/// relative one is invalid and ignored, else the settings would land wherever the program was started), else
/// `$HOME/.local/share`; below it `freesynergy/tasks`.
pub fn data_dir_in(
    xdg: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    let base = xdg
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home.map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(base.join("freesynergy").join("tasks"))
}

/// The remembered frontend, in one file.
#[derive(Clone, Debug)]
pub struct Preference {
    file: PathBuf,
}

impl Preference {
    /// The memory kept in `file`.
    pub fn new(file: PathBuf) -> Self {
        Self { file }
    }

    /// The remembered frontend; the default if there is none or the file cannot be understood (a broken file is named on
    /// stderr, and the next choice replaces it).
    pub fn load(&self) -> Frontend {
        let text = match std::fs::read_to_string(&self.file) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Frontend::DEFAULT,
            Err(e) => {
                eprintln!("fs-tasks: cannot read {}: {e}", self.file.display());
                return Frontend::DEFAULT;
            }
        };
        let value = serde_json::from_str::<Value>(&text).ok();
        let named = |key: &str| {
            value
                .as_ref()
                .and_then(|v| v.get(key)?.as_str().and_then(Frontend::from_id))
        };
        let frontend = named("frontend").or_else(|| {
            // Migration: an older version kept the choice under the key `face`. Read it, and write it
            // under `frontend` (the old key stays in the file, so an older program still finds it).
            let old = named(LEGACY_KEY)?;
            if let Err(why) = self.write(&json!({ "frontend": old.id(), LEGACY_KEY: old.id() })) {
                eprintln!("fs-tasks: cannot migrate the remembered frontend: {why}");
            }
            Some(old)
        });
        frontend.unwrap_or_else(|| {
            eprintln!(
                "fs-tasks: {} holds no frontend I know; using the default",
                self.file.display()
            );
            Frontend::DEFAULT
        })
    }

    /// Remember `frontend`: written to a new file beside the old one, then put in its place, so a crash leaves the old
    /// choice or the new one, never half of it.
    ///
    /// # Errors
    /// A message for the person if the folder or the file cannot be written.
    pub fn save(&self, frontend: Frontend) -> Result<(), String> {
        self.write(&json!({ "frontend": frontend.id() }))
    }

    /// Write `value` atomically (temporary file, synced, renamed over the old one).
    fn write(&self, value: &Value) -> Result<(), String> {
        let name = self
            .file
            .file_name()
            .ok_or_else(|| format!("{} is not a file name", self.file.display()))?;
        if let Some(dir) = self.file.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let mut temp_name = name.to_os_string();
        temp_name.push(format!(".{}.tmp", std::process::id()));
        let temp = self.file.with_file_name(temp_name);
        let body = value.to_string();
        // on disk before it takes the place of the old file (else a crash may leave an empty file in its place)
        let written = std::fs::File::create(&temp).and_then(|mut f| {
            std::io::Write::write_all(&mut f, body.as_bytes())?;
            f.sync_all()
        });
        written.map_err(|e| {
            let _ = std::fs::remove_file(&temp);
            format!("{}: {e}", temp.display())
        })?;
        std::fs::rename(&temp, &self.file).map_err(|e| {
            let _ = std::fs::remove_file(&temp);
            format!("{}: {e}", self.file.display())
        })
    }
}
