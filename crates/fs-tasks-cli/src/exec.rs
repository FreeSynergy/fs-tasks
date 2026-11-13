//! Running a check command: no shell, a time limit, a limit on what is kept of the output.
//!
//! Pattern: none (one function) — the words are the argument vector as they stand; nothing expands them. The
//! policy decides before whether they may run at all.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// How much of the end of the output is kept, in bytes.
const KEEP: usize = 2048;

/// How long the output is waited for after the child has ended (a grandchild may still hold the pipes).
const GRACE: Duration = Duration::from_millis(500);

/// How a check ended.
#[derive(Debug, PartialEq, Eq)]
pub struct Outcome {
    /// The program exited with 0.
    pub passed: bool,
    /// The exit code, if it exited by itself within the time.
    pub code: Option<i32>,
    /// The time ran out and the program was killed.
    pub timed_out: bool,
    /// The end of what it wrote (stdout then stderr), control characters shown as `?`.
    pub tail: String,
}

/// Read to the end, keep the last [`KEEP`] bytes; a full pipe never blocks the child.
fn drain(mut from: impl Read + Send + 'static) -> Receiver<Vec<u8>> {
    let (send, receive) = channel();
    std::thread::spawn(move || {
        let (mut kept, mut chunk) = (Vec::new(), [0u8; 4096]);
        while let Ok(n) = from.read(&mut chunk) {
            if n == 0 {
                break;
            }
            kept.extend_from_slice(&chunk[..n]);
            if kept.len() > KEEP {
                kept.drain(..kept.len() - KEEP);
            }
        }
        let _ = send.send(kept);
    });
    receive
}

/// The `kill` tool at its fixed places (never looked up through `PATH`).
#[cfg(unix)]
const KILL_TOOLS: [&str; 2] = ["/bin/kill", "/usr/bin/kill"];

/// End the process group `pgid` (Unix: `kill -s KILL -- -<pgid>`, the tool is used so that no library is added).
/// Elsewhere nothing is done. The error says why the group could not be ended.
fn kill_group(pgid: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        let tool = KILL_TOOLS
            .iter()
            .find(|t| std::path::Path::new(t).is_file())
            .ok_or("no kill tool found (/bin/kill, /usr/bin/kill)")?;
        let status = Command::new(tool)
            .args(["-s", "KILL", "--", &format!("-{pgid}")])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| format!("cannot run {tool}: {e}"))?;
        status
            .success()
            .then_some(())
            .ok_or_else(|| format!("{tool} failed: {status}"))
    }
    #[cfg(not(unix))]
    {
        let _ = pgid;
        Ok(())
    }
}

/// End the child and everything it started. Unix: the child leads its own process group (`execute` makes it so).
/// Elsewhere (Windows) only the child itself is ended: a documented limit. A group that could not be ended is an error.
fn stop(child: &mut Child) -> Result<(), String> {
    let group = kill_group(child.id());
    let _ = child.kill();
    group
}

/// The kept output of both pipes, or an error when a process that outlived the child holds them open (it is ended).
fn collect(
    pgid: u32,
    (out, err): (Receiver<Vec<u8>>, Receiver<Vec<u8>>),
) -> Result<String, String> {
    let end = Instant::now() + GRACE;
    let mut bytes = Vec::new();
    for pipe in [out, err] {
        let wait = end.saturating_duration_since(Instant::now());
        if let Ok(part) = pipe.recv_timeout(wait) {
            bytes.extend(part);
        } else {
            let ended = kill_group(pgid).map_or_else(
                |e| format!(" (and it could not be ended: {e})"),
                |()| String::new(),
            );
            return Err(format!(
                "the check ended but a process it started keeps its output open; the group was ended{ended}"
            ));
        }
    }
    let start = bytes.len().saturating_sub(KEEP);
    Ok(String::from_utf8_lossy(&bytes[start..])
        .chars()
        .map(|c| if c.is_control() && c != '\n' { '?' } else { c })
        .collect())
}

/// Run `argv` (program first) for at most `seconds`; when the time runs out the whole process group is ended (Unix).
///
/// # Errors
/// The program could not be started.
pub fn execute(argv: &[String], seconds: u64) -> Result<Outcome, String> {
    let (program, rest) = argv.split_first().ok_or("the check command is empty")?;
    let mut command = Command::new(program);
    command
        .args(rest)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Its own process group, so a timeout can end the grandchildren too.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut child = command
        .spawn()
        .map_err(|e| format!("cannot start {program}: {e}"))?;
    let readers = child
        .stdout
        .take()
        .zip(child.stderr.take())
        .map(|(o, e)| (drain(o), drain(e)));
    let pgid = child.id();
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut stopped = Ok(());
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(s) => break Some(s),
            None if Instant::now() >= deadline => {
                stopped = stop(&mut child);
                let _ = child.wait();
                break None;
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    };
    // After a kill a grandchild may still hold the pipes: the readers are not waited for then.
    let mut tail = match (status.is_some(), readers) {
        (true, Some(pipes)) => collect(pgid, pipes)?,
        _ => String::new(),
    };
    if let Err(why) = stopped {
        tail.push_str("\n(the process group could not be ended: ");
        tail.push_str(&why);
        tail.push(')');
    }
    Ok(Outcome {
        passed: status.is_some_and(|s| s.success()),
        code: status.and_then(|s| s.code()),
        timed_out: status.is_none(),
        tail,
    })
}
