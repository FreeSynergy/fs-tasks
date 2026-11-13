//! Making sure a server answers: the logic lives in `fs-tasks-server` (`start`), so the command line and the window
//! start the server the same way (one lock file, one server for many clients).

pub(crate) use fs_tasks_server::ensure_server;
