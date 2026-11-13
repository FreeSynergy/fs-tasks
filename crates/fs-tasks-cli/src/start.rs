//! Making sure a server answers, starting the local one if it is not there.
//!
//! Pattern: Double-checked lock — probe the socket; if nobody answers, take a lock file next to the socket, probe
//! again (another client may have started the server meanwhile), and only then start `fs-tasks serve` and wait
//! for the socket. Many clients at once start one server, not many.

use std::fs::OpenOptions;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use fs_interfaces::rpc::connect;
use fs_interfaces::tasks_capnp::tasks;

/// How long to wait for a server that was started, in steps of 50 ms.
const STEPS: u32 = 100;

async fn answers(socket: &Path) -> bool {
    connect::<tasks::Client>(socket).await.is_ok()
}

fn spawn(socket: &Path, data: Option<&Path>) -> Result<Child, String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("cannot find myself to start the server: {e}"))?;
    let mut command = Command::new(exe);
    command.arg("serve").arg("--socket").arg(socket);
    if let Some(data) = data {
        command.arg("--data").arg(data);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // Its own process group: a Ctrl-C meant for the command does not stop the server.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    command
        .spawn()
        .map_err(|e| format!("cannot start the server: {e}"))
}

/// Return once a server answers at `socket`; start one with `data` as its data folder if none does.
///
/// # Errors
/// The server could not be started or did not come up in five seconds.
pub async fn ensure_server(socket: &Path, data: Option<&Path>) -> Result<(), String> {
    if answers(socket).await {
        return Ok(());
    }
    if let Some(dir) = socket.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(socket.with_extension("lock"))
        .map_err(|e| format!("{}: {e}", socket.display()))?;
    // Held until this function returns (the lock goes with the file handle).
    lock.lock()
        .map_err(|e| format!("{}: {e}", socket.display()))?;
    if answers(socket).await {
        return Ok(());
    }
    let mut child = spawn(socket, data)?;
    for _ in 0..STEPS {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if answers(socket).await {
            return Ok(());
        }
        if let Ok(Some(status)) = child.try_wait() {
            return Err(format!(
                "the server ended at once ({status}); run `fs-tasks serve` to see why"
            ));
        }
    }
    Err(format!(
        "the server did not come up at {}",
        socket.display()
    ))
}
