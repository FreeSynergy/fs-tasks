//! The command line of the window.
//!
//! Pattern: Builder-like parser — plain `std::env::args`, no extra crate for a few options. Import and export of
//! iCalendar files are commands of the command line program (`fs-tasks import|export`), not of the window.

use std::path::PathBuf;

use fs_components_app::parse_size;

/// What was asked for on the command line.
#[derive(Debug, PartialEq, Eq)]
pub struct Args {
    /// Store file; `None` = the data folder.
    pub store: Option<PathBuf>,
    /// Size of the window.
    pub size: (u32, u32),
    /// `--help` was given.
    pub help: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            store: None,
            size: (1300, 800),
            help: false,
        }
    }
}

/// Parse `args` (without the program name).
///
/// # Errors
/// A message for the person: an unknown option, a missing value or a bad size.
pub fn parse(args: &[String]) -> Result<Args, String> {
    let mut out = Args::default();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = |name: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "--store" => out.store = Some(PathBuf::from(value("--store")?)),
            "--size" => out.size = parse_size(&value("--size")?)?,
            "-h" | "--help" => out.help = true,
            other => return Err(format!("unknown option {other}")),
        }
    }
    Ok(out)
}

/// The help text.
pub const HELP: &str = "fs-tasks app: the window of the tasks program

  --store <file>     keep the tasks in this store file (default: the data folder, $XDG_DATA_HOME/freesynergy/tasks/app.db)
  --size <WxH>       window size (default 1300x800)

Import and export of iCalendar files: `fs-tasks import <file>` and `fs-tasks export <file>` (the command line).";
