//! The command line as a function: what `fs-tasks <command>` runs (ADR 0063).
//!
//! Pattern: none (wiring) — parse, make sure the server is there, connect, run one command, print. Exit codes:
//! 0 ok, 1 error, 2 a mistake in the words, 3 conflict (a claim somebody else holds). With `--json` an error is
//! one JSON object on stderr.

use crate::start::ensure_server;
use crate::{Failure, HELP, parse, run};
use fs_interfaces::rpc::{connect, default_socket};
use fs_interfaces::tasks_capnp::tasks;
use serde_json::json;

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
    let json = command.json();
    let local = tokio::task::LocalSet::new();
    let out = local.block_on(&runtime, async move {
        ensure_server(&socket, args.data.as_deref()).await?;
        let connection = connect::<tasks::Client>(&socket)
            .await
            .map_err(|e| format!("{}: {e}", socket.display()))?;
        run(&connection.client, command).await
    });
    match out {
        Ok(text) => {
            if !text.is_empty() {
                println!("{text}");
            }
            0
        }
        Err(e) => report(&e, json),
    }
}

fn report(failure: &Failure, json: bool) -> i32 {
    if json {
        eprintln!(
            "{}",
            json!({"error": failure.text, "conflict": failure.conflict})
        );
    } else {
        eprintln!("fs-tasks: {failure}");
    }
    failure.code()
}
