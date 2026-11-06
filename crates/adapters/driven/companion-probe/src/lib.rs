//! The `LoadoutProbe` port as a process (Go's `companions.Verify`): the
//! companion run as `<path> loadout --format yaml` with a time limit (Go's is
//! three seconds), its output read, a program that outlives the limit killed
//! with everything it started (`run-within`).

use std::sync::Arc;
use std::time::Duration;

use ctxloom_ports::{LoadoutProbe, Unanswered};

mod loadouts;
pub use loadouts::ProcessCompanions;
use process_env::EnvSnapshot;
use run_within::{run_within, Spec, Within};

/// How long, once the companion is over, its output is waited for: a helper
/// it started that outlives it can hold the pipe open.
const DRAIN_GRACE: Duration = Duration::from_secs(2);

/// Probes a program by running it.
pub struct ProcessProbe {
    limit: Duration,
    drain_grace: Duration,
    /// The environment the companion runs in: exactly this one.
    env: Arc<EnvSnapshot>,
}

impl ProcessProbe {
    /// A probe that gives a program `limit` to answer, running it in `env`.
    pub fn new(limit: Duration, env: Arc<EnvSnapshot>) -> ProcessProbe {
        ProcessProbe {
            limit,
            drain_grace: DRAIN_GRACE,
            env,
        }
    }

    /// The same, waiting `grace` for a finished program's output (two
    /// seconds by default).
    pub fn drain_grace(mut self, grace: Duration) -> ProcessProbe {
        self.drain_grace = grace;
        self
    }

    /// Go's three seconds.
    pub fn standard(env: Arc<EnvSnapshot>) -> ProcessProbe {
        ProcessProbe::new(Duration::from_secs(3), env)
    }
}

impl LoadoutProbe for ProcessProbe {
    fn answers(&self, command: &[String]) -> Result<(), Unanswered> {
        self.loadout(command).map(drop)
    }
}

impl ProcessProbe {
    /// The text `<command...> loadout --format yaml` prints, under the same
    /// bound and with the same failures as [`answers`](LoadoutProbe::answers).
    /// The system finds a bare program name as it does for any command; this
    /// searches for nothing.
    pub fn loadout(&self, command: &[String]) -> Result<String, Unanswered> {
        let Some((program, arguments)) = command.split_first() else {
            return Err(Unanswered::Failed("the command is empty".to_string()));
        };
        let spec = Spec::new(program)
            .args(arguments)
            .args(["loadout", "--format", "yaml"])
            .envs(self.env.vars());
        let outcome = run_within(
            &spec,
            &Within::new(self.limit).drain_grace(self.drain_grace),
        )
        .map_err(|e| match e.io().kind() {
            std::io::ErrorKind::NotFound => Unanswered::Missing(e.io().to_string()),
            _ => Unanswered::Failed(e.io().to_string()),
        })?;
        let Some(status) = outcome.status else {
            return Err(Unanswered::Failed(format!(
                "did not answer within {}ms",
                self.limit.as_millis()
            )));
        };
        let printed = outcome.stdout;
        if !status.success() {
            // An exit with a status answered; a signal did not.
            return Err(if status.code().is_some() {
                Unanswered::Exited(status.to_string())
            } else {
                Unanswered::Failed(status.to_string())
            });
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
