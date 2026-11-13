//! The face of the window: which of the three screens the person looks at, and the memory of the choice.
//!
//! Pattern: Value object (`Face`) + Memento holder (`Preference`, one small file in the data folder). A file that is
//! missing, unreadable or broken means the default; a write that fails is reported, never fatal.

use std::path::PathBuf;

use serde_json::{Value, json};

/// A face of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    /// The first window: list and status view.
    Classic,
    /// A: the board, columns by status, cards.
    Board,
    /// B: the day list, Today, Upcoming, Anytime and the labels.
    Day,
}

impl Face {
    /// What a new installation shows.
    pub const DEFAULT: Face = Face::Board;

    /// All faces, in the order of the menu.
    pub const ALL: [Face; 3] = [Face::Classic, Face::Board, Face::Day];

    /// The stable name (in the settings file and in the id of the menu entry).
    pub fn id(self) -> &'static str {
        match self {
            Face::Classic => "classic",
            Face::Board => "board",
            Face::Day => "day",
        }
    }

    /// The face with this stable name.
    pub fn from_id(id: &str) -> Option<Face> {
        Face::ALL.into_iter().find(|f| f.id() == id)
    }

    /// What the menu says.
    pub fn label(self) -> &'static str {
        match self {
            Face::Classic => "Classic",
            Face::Board => "Board",
            Face::Day => "Day list",
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

/// The remembered face, in one file.
#[derive(Clone, Debug)]
pub struct Preference {
    file: PathBuf,
}

impl Preference {
    /// The memory kept in `file`.
    pub fn new(file: PathBuf) -> Self {
        Self { file }
    }

    /// The remembered face; the default if there is none or the file cannot be understood (a broken file is named on
    /// stderr, and the next choice replaces it).
    pub fn load(&self) -> Face {
        let text = match std::fs::read_to_string(&self.file) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Face::DEFAULT,
            Err(e) => {
                eprintln!("fs-tasks: cannot read {}: {e}", self.file.display());
                return Face::DEFAULT;
            }
        };
        let face = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| v.get("face")?.as_str().and_then(Face::from_id));
        face.unwrap_or_else(|| {
            eprintln!(
                "fs-tasks: {} holds no face I know; using the default",
                self.file.display()
            );
            Face::DEFAULT
        })
    }

    /// Remember `face`: written to a new file beside the old one, then put in its place, so a crash leaves the old
    /// choice or the new one, never half of it.
    ///
    /// # Errors
    /// A message for the person if the folder or the file cannot be written.
    pub fn save(&self, face: Face) -> Result<(), String> {
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
        let body = json!({ "face": face.id() }).to_string();
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
