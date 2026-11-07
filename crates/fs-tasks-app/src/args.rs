//! The command line.
//!
//! Pattern: Builder-like parser — plain `std::env::args`, no extra crate for a few options. The app holds no store: it needs the
//! socket of a server.

use std::path::{Path, PathBuf};

use fs_components_app::parse_size;
use fs_components_workbenches::tasks::TaskView;

/// What was asked for on the command line.
#[derive(Debug, PartialEq, Eq)]
pub struct Args {
    /// Unix socket of a running `fs-tasks-server`; the app holds no store of its own.
    pub socket: Option<PathBuf>,
    /// An iCalendar file to import before anything else.
    pub import: Option<PathBuf>,
    /// Write all tasks to this iCalendar file; no window opens then.
    pub export: Option<PathBuf>,
    /// The `uid`s of tasks marked sensitive that the export may include, each named by the person.
    pub export_sensitive: Vec<String>,
    /// Draw a picture to this file instead of opening a window.
    pub snapshot: Option<PathBuf>,
    /// Size of the window or picture.
    pub size: (u32, u32),
    /// The view to start with.
    pub view: Option<TaskView>,
    /// `--help` was given.
    pub help: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            socket: None,
            import: None,
            export: None,
            export_sensitive: Vec::new(),
            snapshot: None,
            size: (1300, 800),
            view: None,
            help: false,
        }
    }
}

impl Args {
    /// The socket to connect to.
    ///
    /// # Errors
    /// A message for the person if `--socket` was not given, saying where a socket comes from.
    pub fn socket_path(&self) -> Result<&Path, String> {
        self.socket.as_deref().ok_or_else(|| {
            "no --socket given: the app talks to a running tasks server; start one with \
             `fs-tasks-server --socket <path>` and pass the same path here"
                .to_string()
        })
    }
}

/// Parse `args` (without the program name).
///
/// # Errors
/// A message for the person: an unknown option, a missing value or a bad size or view.
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
            "--socket" => out.socket = Some(PathBuf::from(value("--socket")?)),
            "--store" => {
                return Err(
                    "--store is gone: the store belongs to the server, start it with \
                     `fs-tasks-server --data <dir>` and give the app its --socket"
                        .into(),
                );
            }
            "--import" => out.import = Some(PathBuf::from(value("--import")?)),
            "--export" => out.export = Some(PathBuf::from(value("--export")?)),
            "--export-sensitive" => out.export_sensitive.push(value("--export-sensitive")?),
            "--snapshot" => out.snapshot = Some(PathBuf::from(value("--snapshot")?)),
            "--size" => out.size = parse_size(&value("--size")?)?,
            "--view" => out.view = Some(view(&value("--view")?)?),
            "-h" | "--help" => out.help = true,
            other => return Err(format!("unknown option {other}")),
        }
    }
    if !out.export_sensitive.is_empty() && out.export.is_none() {
        return Err("--export-sensitive needs --export".into());
    }
    Ok(out)
}

fn view(text: &str) -> Result<TaskView, String> {
    match text {
        "list" => Ok(TaskView::List),
        "status" => Ok(TaskView::Status),
        other => Err(format!("--view wants list or status, not {other}")),
    }
}

/// The help text.
pub const HELP: &str = "fs-tasks-app: the tasks program

  --socket <path>    Unix socket of a running `fs-tasks-server --socket <path>` (required)
  --import <file>    import an iCalendar file first; tasks already known are skipped, never replaced
  --export <file>    write all tasks to a new iCalendar file and stop; an existing file is not
                     replaced, tasks marked sensitive are left out and named
  --export-sensitive <uid>
                     release this task marked sensitive for --export; repeat for each task
  --snapshot <png>   draw a picture instead of opening a window
  --size <WxH>       window or picture size (default 1300x800)
  --view <list|status>
                     the view to start with (default list)";
