//! The table.

use crate::Finding;

/// The workspace axis.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Workspace {
    /// The live project directory.
    None,
    /// A git worktree on the agent's own branch.
    Worktree,
}

/// Who owns the container runtime's daemon.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ownership {
    Rootless,
    Rootful,
}

/// The runtime axis.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Runtime {
    Host,
    Container(Ownership),
}

/// What the agent binding and the orchestration ask for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Demand {
    pub runtime: Runtime,
    pub workspace: Workspace,
}

/// What this machine and this project offer.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Facts {
    /// Whether the project is a git repository.
    pub git_repository: bool,
    /// The ownership modes a reachable runtime can serve (empty: no runtime).
    pub offers: Vec<Ownership>,
    /// Whether the backend's agent image is present or can be built.
    pub image_available: bool,
}

/// A degrade that was taken; the run goes ahead and says so.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Warning {
    /// A worktree was demanded and none can be had (the project is not a git
    /// repository): the run is in the live directory.
    WorktreeUnavailable,
}

/// What the run gets.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Plan {
    pub runtime: Runtime,
    pub workspace: Workspace,
    pub warnings: Vec<Warning>,
}

/// Decide the run's isolation, or refuse it. `degraded` is `--degraded`: it
/// turns warnings into carrying on and nothing else.
pub fn decide(demand: Demand, facts: &Facts, degraded: bool) -> Result<Plan, Finding> {
    // `--degraded` rescues no container refusal and changes no decision.
    let _ = degraded;
    if let Runtime::Container(demanded) = demand.runtime {
        if facts.offers.is_empty() {
            return Err(Finding::NoRuntime { demanded });
        }
        if !facts.offers.contains(&demanded) {
            return Err(Finding::OwnershipMismatch {
                demanded,
                offered: facts.offers.clone(),
            });
        }
        if !facts.image_available {
            return Err(Finding::ImageUnavailable { demanded });
        }
    }
    let (workspace, warnings) = match demand.workspace {
        Workspace::Worktree if !facts.git_repository => {
            (Workspace::None, vec![Warning::WorktreeUnavailable])
        }
        other => (other, Vec::new()),
    };
    Ok(Plan {
        runtime: demand.runtime,
        workspace,
        warnings,
    })
}
