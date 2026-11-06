//! The `CompanionLoadouts` port as processes: a registered companion name
//! mapped to the program that answers for it, and that program asked for its
//! loadout under the probe's bound (Go's `Prober.ProbeCompanionLoadouts`).
//!
//! Which program a name stands for is not decided here: the adapter is given
//! it, already resolved. How a registration names its program is the
//! composition root's to settle.

use std::collections::BTreeMap;

use ctxloom_ports::{CompanionLoadout, CompanionLoadouts, Unanswered};

use crate::ProcessProbe;

/// Companions as programs, by registered name.
pub struct ProcessCompanions {
    /// Each registered name's program path.
    programs: BTreeMap<String, String>,
    probe: ProcessProbe,
}

impl ProcessCompanions {
    /// Companions answering through the programs at the given paths, asked
    /// through `probe`. A name with no program is absent.
    pub fn new(programs: BTreeMap<String, String>, probe: ProcessProbe) -> ProcessCompanions {
        ProcessCompanions { programs, probe }
    }
}

impl CompanionLoadouts for ProcessCompanions {
    fn loadout(&self, name: &str) -> CompanionLoadout {
        let Some(path) = self.programs.get(name) else {
            return CompanionLoadout::Absent;
        };
        match self.probe.loadout(path) {
            Ok(text) => CompanionLoadout::Loaded(text),
            Err(Unanswered::Silent) => CompanionLoadout::NoLoadout,
            Err(Unanswered::Failed(detail)) => CompanionLoadout::ProbeFailed(detail),
        }
    }
}

#[cfg(test)]
#[path = "loadouts.test.rs"]
mod tests;
