//! The `CompanionLoadouts` port as processes (Go's `Prober.ProbeCompanionLoadouts`
//! and `companions.BinaryName`): a registered name resolved to its binary on
//! the `PATH`, and that binary asked for its loadout under the probe's bound.

use std::sync::Arc;

use ctxloom_ports::{CompanionLoadout, CompanionLoadouts, Executables, Unanswered};
use process_env::EnvSnapshot;

use crate::ProcessProbe;

/// The shipped companions whose binaries carry their own names; every other
/// name `n` is the binary `ctxloom-companion-n`.
const FIRST_PARTY: [&str; 3] = ["ltk", "taskloom", "reprise"];

const PREFIX: &str = "ctxloom-companion-";

/// The binary a companion name resolves on the `PATH`.
pub fn binary_name(name: &str) -> String {
    if FIRST_PARTY.contains(&name) {
        name.to_string()
    } else {
        format!("{PREFIX}{name}")
    }
}

/// Whether `name` is a plain companion name, `^[A-Za-z0-9][A-Za-z0-9._-]*$`:
/// one path segment that cannot be read as a flag.
pub fn is_plain_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Companions found on the `PATH` and run to be asked.
pub struct ProcessCompanions {
    binaries: Arc<dyn Executables>,
    probe: ProcessProbe,
}

impl ProcessCompanions {
    /// Companions found by `binaries` and asked through `probe`.
    pub fn new(binaries: Arc<dyn Executables>, probe: ProcessProbe) -> ProcessCompanions {
        ProcessCompanions { binaries, probe }
    }

    /// The same under Go's three seconds, a companion running in `env`.
    pub fn standard(binaries: Arc<dyn Executables>, env: Arc<EnvSnapshot>) -> ProcessCompanions {
        ProcessCompanions::new(binaries, ProcessProbe::standard(env))
    }
}

impl CompanionLoadouts for ProcessCompanions {
    fn loadout(&self, name: &str) -> CompanionLoadout {
        if !is_plain_name(name) {
            return CompanionLoadout::ProbeFailed(format!(
                "{name:?} is not a plain companion name"
            ));
        }
        let Some(path) = self.binaries.find(&binary_name(name)) else {
            return CompanionLoadout::Absent;
        };
        match self.probe.loadout(&path) {
            Ok(text) => CompanionLoadout::Loaded(text),
            Err(Unanswered::Silent) => CompanionLoadout::NoLoadout,
            Err(Unanswered::Failed(detail)) => CompanionLoadout::ProbeFailed(detail),
        }
    }
}

#[cfg(test)]
#[path = "loadouts.test.rs"]
mod tests;
