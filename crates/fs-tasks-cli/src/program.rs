//! The command line as a function: what `fs-tasks <command>` runs (ADR 0063).
//!
//! Pattern: none (wiring) — parse, connect to the socket, run one command, print.

use crate::{HELP, parse, run};
use fs_interfaces::rpc::{connect, default_socket};
use fs_interfaces::tasks_capnp::tasks;

/// Run the command line `words` (after the program name); the exit code.
#[must_use]
pub fn cli_command(words: &[String]) -> i32 {
    let args = match parse(words) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("fs-tasks: {e}");
            return 2;
        }
    };
    let Some(command) = args.command else {
        println!("{HELP}");
        return 0;
    };
    let Some(socket) = args.socket.or_else(|| default_socket("tasks")) else {
        eprintln!("fs-tasks: no socket: give --socket or set XDG_RUNTIME_DIR");
        return 2;
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("fs-tasks: {e}");
            return 1;
        }
    };
    let local = tokio::task::LocalSet::new();
    let out = local.block_on(&runtime, async move {
        let connection = connect::<tasks::Client>(&socket)
            .await
            .map_err(|e| format!("{}: {e}", socket.display()))?;
        run(&connection.client, command).await
    });
    match out {
        Ok(text) => {
            println!("{text}");
            0
        }
        Err(e) => {
            eprintln!("fs-tasks: {e}");
            1
        }
    }
}
