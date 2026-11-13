//! Running a check command: no shell, a time limit, a limit on what is kept of the output.
//!
//! Pattern: none (one function) — the words are the argument vector as they stand; nothing expands them. The
//! policy decides before whether they may run at all.

use std::io::Read;
use std::process::{Command, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How much of the end of the output is kept, in bytes.
const KEEP: usize = 2048;

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
fn drain(mut from: impl Read + Send + 'static) -> JoinHandle<Vec<u8>> {
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
        kept
    })
}

/// Run `argv` (program first) for at most `seconds`.
///
/// # Errors
/// The program could not be started.
pub fn execute(argv: &[String], seconds: u64) -> Result<Outcome, String> {
    let (program, rest) = argv.split_first().ok_or("the check command is empty")?;
    let mut child = Command::new(program)
        .args(rest)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot start {program}: {e}"))?;
    let readers = child
        .stdout
        .take()
        .zip(child.stderr.take())
        .map(|(o, e)| (drain(o), drain(e)));
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(s) => break Some(s),
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    };
    // After a kill a grandchild may still hold the pipes: the readers are not waited for then.
    let tail = match (status.is_some(), readers) {
        (true, Some((o, e))) => {
            let mut bytes = o.join().unwrap_or_default();
            bytes.extend(e.join().unwrap_or_default());
            let start = bytes.len().saturating_sub(KEEP);
            String::from_utf8_lossy(&bytes[start..])
                .chars()
                .map(|c| if c.is_control() && c != '\n' { '?' } else { c })
                .collect()
        }
        _ => String::new(),
    };
    Ok(Outcome {
        passed: status.is_some_and(|s| s.success()),
        code: status.and_then(|s| s.code()),
        timed_out: status.is_none(),
        tail,
    })
}
