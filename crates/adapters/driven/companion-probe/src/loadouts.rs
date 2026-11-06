//! The `CompanionLoadouts` port as processes (Go's
//! `Prober.ProbeCompanionLoadouts`): a registered companion's command run for
//! its loadout under the probe's bound. Nothing resolves a name or searches a
//! path; the command is exactly what the registration holds.
//!
//! The outcomes are Go's: a program that does not exist is absent; one that
//! ran and exited with a status offers no loadout (quietly); one that was
//! killed, timed out, could not be started or printed nothing failed.

use ctxloom_ports::{CompanionLoadout, CompanionLoadouts, RegisteredCompanion, Unanswered};

use crate::ProcessProbe;

/// Companions run as the commands that are registered for them.
pub struct ProcessCompanions {
    probe: ProcessProbe,
}

impl ProcessCompanions {
    /// Companions asked through `probe`.
    pub fn new(probe: ProcessProbe) -> ProcessCompanions {
        ProcessCompanions { probe }
    }
}

impl CompanionLoadouts for ProcessCompanions {
    fn loadout(&self, companion: &RegisteredCompanion) -> CompanionLoadout {
        match self.probe.loadout(&companion.command) {
            Ok(text) => CompanionLoadout::Loaded(text),
            Err(Unanswered::Missing(_)) => CompanionLoadout::Absent,
            Err(Unanswered::Exited(_)) => CompanionLoadout::NoLoadout,
            Err(Unanswered::Silent) => {
                CompanionLoadout::ProbeFailed("it printed no loadout".to_string())
            }
            Err(Unanswered::Failed(detail)) => CompanionLoadout::ProbeFailed(detail),
        }
    }
}

#[cfg(test)]
#[path = "loadouts.test.rs"]
mod tests;
