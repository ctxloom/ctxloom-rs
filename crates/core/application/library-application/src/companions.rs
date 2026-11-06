//! The registered companions' loadouts, joined to the catalog an assembly
//! reads (Go's `companions.Prober.ProbeCompanionLoadouts` and the companion
//! reader): each registered name asked through the `CompanionLoadouts` port,
//! its `run:` bundle seeded under the companion's reference.
//!
//! Never fatal: an absent, failing or unparseable companion costs its own
//! contribution and is said as a warning. `--no-companions` and
//! `CTXLOOM_NO_COMPANIONS` ask none.

use ctxloom_ports::CompanionLoadout;
use library::catalog::Catalog;
use library::loadout::parse_loadout;
use library::model::Bundle;

use crate::error::Notice;
use crate::project::Project;

/// Whether the environment's switch `name` is on: `1`, `t`, `true`, `y`,
/// `yes` or `on`, in any case, around any space (Go's `envswitch.On`). Any
/// other value is off, so a typo never turns a mode on.
pub(crate) fn switch_on(environ: &[(String, String)], name: &str) -> bool {
    environ
        .iter()
        .find(|(variable, _)| variable == name)
        .is_some_and(|(_, value)| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "t" | "true" | "y" | "yes" | "on"
            )
        })
}

impl Project {
    /// The project's catalog with the registered companions' loadouts in it.
    /// Where there are none, it is the catalog as it is.
    pub(crate) fn catalog_with_companions(&mut self) -> Catalog {
        let loadouts = self.companion_loadouts();
        if loadouts.is_empty() {
            return self.catalog.clone();
        }
        self.catalog.clone().with_companions(loadouts)
    }

    /// The `run:` bundles of the registered companions, `(name, bundle)`,
    /// asked for the first time this is called and kept for the command, so a
    /// companion is run once and its failure said once.
    fn companion_loadouts(&mut self) -> Vec<(String, Bundle)> {
        if let Some(asked) = &self.companion_bundles {
            return asked.clone();
        }
        let asked = self.ask_companions();
        self.companion_bundles = Some(asked.clone());
        asked
    }

    fn ask_companions(&mut self) -> Vec<(String, Bundle)> {
        let Some(port) = self.companions.clone().filter(|_| !self.no_companions) else {
            return Vec::new();
        };
        let mut loaded = Vec::new();
        for name in self.config.companions() {
            match port.loadout(&name) {
                CompanionLoadout::Loaded(text) => match parse_loadout(&text) {
                    Ok(loadout) => loaded.push((name, loadout.run)),
                    Err(cause) => self
                        .notices
                        .push(Notice::LoadoutUnparseable { name, cause }),
                },
                CompanionLoadout::Absent => {
                    self.notices.push(Notice::CompanionNotOnPath { name });
                }
                CompanionLoadout::NoLoadout => {}
                CompanionLoadout::ProbeFailed(detail) => {
                    self.notices
                        .push(Notice::LoadoutProbeFailed { name, detail });
                }
            }
        }
        loaded
    }
}
