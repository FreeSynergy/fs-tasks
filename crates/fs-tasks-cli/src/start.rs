//! Making sure a server answers: the logic lives in `fs-tasks-server` (`start`), so the command line and the window
//! start the server the same way (one lock file, one server for many clients).
//!
//! Pattern: Facade — a re-export of the one function the command line needs.

pub(crate) use fs_tasks_server::ensure_server;
