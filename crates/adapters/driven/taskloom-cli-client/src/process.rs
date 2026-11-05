//! Running a program with a deadline: its output read as it comes, a program
//! that outlives its limit killed and reaped, never waited on.

use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// How often a running program is asked whether it has finished.
const POLL: Duration = Duration::from_millis(5);

/// Why a run produced no output.
pub(crate) enum Unfinished {
    /// It could not be started or waited on.
    Failed(std::io::Error),
    /// It did not finish within its limit; it was killed and reaped.
    TimedOut,
}

/// `command` run to its end, or killed and reaped at `timeout`.
pub(crate) fn output_within(
    command: &mut Command,
    timeout: Duration,
) -> Result<Output, Unfinished> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(Unfinished::Failed)?;
    // Read as it comes, so a program with much to say is never stuck on a
    // full pipe while this waits for it to end.
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                // The readers end with whatever still holds the pipes (a
                // helper the program started outlives it).
                return Err(Unfinished::TimedOut);
            }
            Ok(None) => std::thread::sleep(POLL),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Unfinished::Failed(e));
            }
        }
    };
    Ok(Output {
        status,
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    })
}

fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> std::thread::JoinHandle<Vec<u8>> {
    let mut pipe = pipe;
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(pipe) = pipe.as_mut() {
            let _ = pipe.read_to_end(&mut bytes);
        }
        bytes
    })
}
