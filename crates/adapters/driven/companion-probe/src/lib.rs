//! The `LoadoutProbe` port as a process (Go's `companions.Verify`): the
//! companion run as `<path> loadout --format yaml` with a time limit (Go's is
//! three seconds), its output read, a program that outlives the limit killed.

use std::time::Duration;

use ctxloom_ports::{LoadoutProbe, Unanswered};

/// Probes a program by running it.
pub struct ProcessProbe {
    limit: Duration,
}

impl ProcessProbe {
    /// A probe that gives a program `limit` to answer.
    pub fn new(limit: Duration) -> ProcessProbe {
        ProcessProbe { limit }
    }

    /// Go's three seconds.
    pub fn standard() -> ProcessProbe {
        ProcessProbe::new(Duration::from_secs(3))
    }
}

impl LoadoutProbe for ProcessProbe {
    fn answers(&self, path: &str) -> Result<(), Unanswered> {
        self.loadout(path).map(drop)
    }
}

impl ProcessProbe {
    /// The text `<path> loadout --format yaml` prints, under the same bound
    /// and with the same failures as [`answers`](LoadoutProbe::answers).
    pub fn loadout(&self, path: &str) -> Result<String, Unanswered> {
        use std::io::Read;
        use std::process::{Command, Stdio};
        use std::time::Instant;

        let mut child = Command::new(path)
            .args(["loadout", "--format", "yaml"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| Unanswered::Failed(e.to_string()))?;
        // The output is read as it comes, so a program with much to say is
        // never stuck on a full pipe while the probe waits for it to end.
        let mut stdout = child.stdout.take().map(std::io::BufReader::new);
        let reader = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(out) = stdout.as_mut() {
                let _ = out.read_to_end(&mut bytes);
            }
            bytes
        });
        let deadline = Instant::now() + self.limit;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    // The reader is left to end with whatever still holds the
                    // pipe (a shell's child outlives the shell it was killed with).
                    drop(reader);
                    return Err(Unanswered::Failed(format!(
                        "did not answer within {}ms",
                        self.limit.as_millis()
                    )));
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(5)),
                Err(e) => return Err(Unanswered::Failed(e.to_string())),
            }
        };
        let printed = reader.join().unwrap_or_default();
        if !status.success() {
            return Err(Unanswered::Failed(status.to_string()));
        }
        if printed.is_empty() {
            return Err(Unanswered::Silent);
        }
        Ok(String::from_utf8_lossy(&printed).into_owned())
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
