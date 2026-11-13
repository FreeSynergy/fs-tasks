//! The command line.
//!
//! Pattern: Builder-like parser — plain `std::env::args`, no extra crate for a few options.

use std::path::PathBuf;

use fs_components_app::parse_size;
use fs_components_workbenches::tasks::TaskView;

use crate::day::Day;
use crate::frontend::Frontend;

/// What `--plan` can ask for: a frontend of the window, or one of the earlier plan views of the workbench.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    /// The classic window (`--plan classic`).
    Classic,
    /// A, drawn by the plan widgets (`--plan board`, also `board2`).
    Board,
    /// B, drawn by the plan widgets (`--plan next`, also `next2`).
    Next,
    /// The earlier board on the workbench (`--plan board1`): columns by status, cards.
    OldBoard,
    /// The earlier list on the workbench (`--plan next1`): Today, Ready, Blocked.
    OldNext,
}

impl Plan {
    /// The frontend this asks for; `None` for the earlier workbench views.
    #[must_use]
    pub fn frontend(self) -> Option<Frontend> {
        match self {
            Plan::Classic => Some(Frontend::Classic),
            Plan::Board => Some(Frontend::Board),
            Plan::Next => Some(Frontend::Day),
            Plan::OldBoard | Plan::OldNext => None,
        }
    }
}

/// What was asked for on the command line.
#[derive(Debug, PartialEq, Eq)]
pub struct Args {
    /// Store file; `None` = the data folder for a window, memory for a picture or an export.
    pub store: Option<PathBuf>,
    /// Socket of the tasks server to use (started if none answers), also for a picture or an export.
    pub connect: Option<PathBuf>,
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
    /// Show the plan view: the board or "what next".
    pub plan: Option<Plan>,
    /// Put sample tasks in first: `demo` or `chain:N`.
    pub sample: Option<String>,
    /// The day that is "today" for the plan views (default: the system's).
    pub today: Option<Day>,
    /// Choose the task whose title holds this text (a picture of the open panel).
    pub open: Option<String>,
    /// The day list to show first (`--list today|upcoming|...`).
    pub list: Option<String>,
    /// `--help` was given.
    pub help: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            store: None,
            connect: None,
            import: None,
            export: None,
            export_sensitive: Vec::new(),
            snapshot: None,
            size: (1300, 800),
            view: None,
            plan: None,
            sample: None,
            today: None,
            open: None,
            list: None,
            help: false,
        }
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
            "--store" => out.store = Some(PathBuf::from(value("--store")?)),
            "--connect" => out.connect = Some(PathBuf::from(value("--connect")?)),
            "--import" => out.import = Some(PathBuf::from(value("--import")?)),
            "--export" => out.export = Some(PathBuf::from(value("--export")?)),
            "--export-sensitive" => out.export_sensitive.push(value("--export-sensitive")?),
            "--snapshot" => out.snapshot = Some(PathBuf::from(value("--snapshot")?)),
            "--size" => out.size = parse_size(&value("--size")?)?,
            "--view" => out.view = Some(view(&value("--view")?)?),
            "--plan" => out.plan = Some(plan(&value("--plan")?)?),
            "--open" => out.open = Some(value("--open")?),
            "--list" => out.list = Some(value("--list")?),
            "--sample" => out.sample = Some(value("--sample")?),
            "--today" => out.today = Some(Day::parse(&value("--today")?)?),
            "-h" | "--help" => out.help = true,
            other => return Err(format!("unknown option {other}")),
        }
    }
    if !out.export_sensitive.is_empty() && out.export.is_none() {
        return Err("--export-sensitive needs --export".into());
    }
    Ok(out)
}

fn plan(text: &str) -> Result<Plan, String> {
    match text {
        "classic" => Ok(Plan::Classic),
        "board" | "board2" => Ok(Plan::Board),
        "next" | "next2" => Ok(Plan::Next),
        "board1" => Ok(Plan::OldBoard),
        "next1" => Ok(Plan::OldNext),
        other => Err(format!("--plan wants classic, board or next, not {other}")),
    }
}

fn view(text: &str) -> Result<TaskView, String> {
    match text {
        "list" => Ok(TaskView::List),
        "status" => Ok(TaskView::Status),
        other => Err(format!("--view wants list or status, not {other}")),
    }
}

/// The help text.
pub const HELP: &str = "fs-tasks app: the window of the tasks program (options of the window)

  --connect <socket> use the tasks server at this socket (default for a window: $XDG_RUNTIME_DIR/freesynergy/tasks.sock);
                     it is started if none answers and keeps running when the window closes
  --store <file>     keep the tasks in this store file, in this process (then no server is used; a picture or an
                     export without --connect uses memory)
  --import <file>    import an iCalendar file first; tasks already known are skipped, never replaced
  --export <file>    write all tasks to a new iCalendar file and stop; an existing file is not
                     replaced, tasks marked sensitive are left out and named
  --export-sensitive <uid>
                     release this task marked sensitive for --export; repeat for each task
  --snapshot <png>   draw a picture instead of opening a window
  --size <WxH>       window or picture size (default 1300x800)
  --view <list|status>
                     the view to start with (default list)
  --plan <classic|board|next>
                     the frontend to start with, this time only (not remembered): the classic window, the
                     board (columns by status, cards that say what blocks them) or the day list
                     (Today, Upcoming, Anytime, labels). Without it the window starts with the frontend
                     chosen last in its menu \"Frontend\" (a new installation: the board)
  --sample <demo|chain:N>
                     put sample tasks in first (a small believable set, or a chain of N tasks)
  --today <YYYY-MM-DD>
                     the day that is today for --plan (default: the system's, UTC)";
